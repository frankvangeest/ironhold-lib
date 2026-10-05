//! D2 determinism guard: no std/hash-ordered `HashMap`/`HashSet` in `ironhold_core/src` unless it
//! is annotated as safe.
//!
//! Why: Rust's std `HashMap`/`HashSet` use a seeded hasher (random on native; address-derived and
//! allocation-history-dependent on wasm32), so iterating one gives a different order per instance,
//! per run and per platform. Any gameplay path whose outcome depends on that order is a
//! lockstep/replay divergence source (see `planning/investigations/hashmap_iteration_order_audit.md`
//! and `crates/ironhold_core/src/CLAUDE.md` "Deterministic iteration order on gameplay paths").
//! D1 fixed the sites that decided outcomes; this test stops new ones appearing.
//!
//! The rule, for every non-comment, non-test line that constructs or names a hash container
//! (`HashMap<`, `HashMap::`, `HashMap(`, and the same for `HashSet` and for any identifier ending in
//! `HashMap`/`HashSet`, e.g. `FxHashMap`, `EntityHashSet`, `bevy::platform::collections::HashMap`):
//!
//! - **iterated, with an order-dependent outcome (or user-visible order):** use `BTreeMap`/`BTreeSet`,
//!   an `IndexMap` (insertion order is part of the contract) or a sorted `Vec` instead. User-visible
//!   order includes validation errors, CLI output, event order and `warn!`/log lines a designer reads;
//! - **only keyed lookup / insert / remove / contains:** keep the hash container and put
//!   `// det: lookup-only` on the same line;
//! - **iterated, but the result provably does not depend on the order** (a pure `retain`, a merge
//!   into another unordered collection, work that is sorted before use, log order only):
//!   `// det: order-independent` on the same line, with a few words on why.
//!
//! `use` imports are not flagged (they name no container being built or typed) **except** aliasing
//! imports (`use ...::HashMap as Map;`) and `type X = HashMap<..>` aliases, which are always flagged
//! (a marker cannot hide them: every later use of the alias would be invisible to this scan). Of
//! test code, only the body of a `#[cfg(test)] mod ... { }` block is exempt (brace-matched, so code
//! placed after a test module is still scanned and a doc comment that merely mentions
//! `#[cfg(test)]` does not switch the scan off); an unbalanced test module panics instead of
//! silently exempting the rest of the file.
//!
//! **Known limits — this is a guard rail, not a proof:**
//! - A marker sits on the *declaration*. It is a claim about every use of that container; if you add
//!   iteration (`.iter()`, `.keys()`, `.values()`, `.drain()`, a `for` loop) to a `lookup-only` one,
//!   re-check the claim and relabel or switch the type. Reviewers should grep the uses of annotated names.
//! - Hash containers owned by external types and iterated without ever being named in our source
//!   (e.g. bevy's `Gltf::named_animations`) are invisible.
//! - The scan cuts a line at the first `//`, even inside a string literal, and accepts a marker that
//!   appears inside a string; contrived, but possible.
//! - Only `ironhold_core/src` is scanned. The CLI crate is a tool (it sorts its output explicitly), and
//!   the native/web runners hold no maps.
//! - Query / entity iteration order is not a hash container and is out of scope (D4, `Entity` order
//!   rule in `src/CLAUDE.md`).

use std::fs;
use std::path::{Path, PathBuf};

const MARKERS: [&str; 2] = ["det: lookup-only", "det: order-independent"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn mentions_hash_container(code: &str) -> bool {
    code.contains("HashMap") || code.contains("HashSet")
}

/// Whether `line` constructs or types a hash container without an allowed marker.
fn line_violates(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return false; // comment / doc comment
    }
    // Only the code before any trailing `//` comment counts.
    let code = line.split("//").next().unwrap_or("");
    let code_trim = code.trim_start();

    // Aliases cannot be hidden behind a marker: every later use of the alias would be invisible.
    let is_use = code_trim.starts_with("use ") || code_trim.starts_with("pub use ") || code_trim.starts_with("pub(crate) use ");
    if is_use && (code.contains("HashMap as ") || code.contains("HashSet as ")) {
        return true;
    }
    let is_type_alias = code_trim.starts_with("type ")
        || code_trim.starts_with("pub type ")
        || code_trim.starts_with("pub(crate) type ");
    if is_type_alias && mentions_hash_container(code) {
        return true;
    }

    if MARKERS.iter().any(|m| line.contains(m)) {
        return false;
    }
    for token in ["HashMap", "HashSet"] {
        let mut from = 0;
        while let Some(pos) = code[from..].find(token) {
            let end = from + pos + token.len();
            let rest = code[end..].trim_start();
            if rest.starts_with('<') || rest.starts_with("::") || rest.starts_with('(') {
                return true;
            }
            from = end;
        }
    }
    false
}

/// Yields `(0-based line index, line)` for every line outside `#[cfg(test)] mod ... { ... }` blocks.
/// The attribute must be the whole trimmed line (a doc comment mentioning it does not count); the
/// test module's extent is found by brace matching on the code part of each line, so code placed
/// after a test module is still scanned. An out-of-line `mod tests;` exempts only its own line.
/// `#[cfg(test)]` on a single non-module item exempts just that attribute line and the item's first
/// line. A test module whose braces never balance panics (a literal `"{"`/`'{'` inside it would
/// otherwise silently exempt the rest of the file).
fn non_test_lines(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "#[cfg(test)]" {
            let attr_line = i;
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            let is_mod = lines.get(j).is_some_and(|l| {
                let t = l.trim_start();
                t.starts_with("mod ") || t.starts_with("pub mod ") || t.starts_with("pub(crate) mod ")
            });
            if is_mod {
                let first = lines[j].split("//").next().unwrap_or("").trim_end();
                if first.ends_with(';') && !first.contains('{') {
                    i = j + 1; // out-of-line `mod tests;`
                    continue;
                }
                let mut depth: i32 = 0;
                let mut opened = false;
                let mut closed = false;
                while j < lines.len() {
                    let code = lines[j].split("//").next().unwrap_or("");
                    for ch in code.chars() {
                        if ch == '{' {
                            depth += 1;
                            opened = true;
                        } else if ch == '}' {
                            depth -= 1;
                        }
                    }
                    j += 1;
                    if opened && depth <= 0 {
                        closed = true;
                        break;
                    }
                }
                assert!(
                    closed,
                    "unbalanced braces in the #[cfg(test)] module starting at line {} \
                     (a literal `{{`/`}}` in a string or char inside it?) — the rest of the file would be unscanned",
                    attr_line + 1
                );
                i = j;
            } else {
                i = j + 1; // `#[cfg(test)]` on a single item: skip the attribute and the item's first line
            }
            continue;
        }
        out.push((i, lines[i]));
        i += 1;
    }
    out
}

#[test]
fn no_unannotated_hash_containers_in_ironhold_core_src() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(files.len() > 20, "expected to scan the whole src tree, found {} files", files.len());

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("cannot read {} for the determinism scan: {e}", file.display()));
        for (i, line) in non_test_lines(&text) {
            if line_violates(line) {
                let rel = file.strip_prefix(&src).unwrap_or(file);
                violations.push(format!("src/{}:{}: {}", rel.display(), i + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{} unannotated HashMap/HashSet use(s) in ironhold_core/src.\n\
         Std hash containers iterate in a seeded, non-deterministic order. For each line below either:\n\
         \u{20}\u{20}- switch to BTreeMap/BTreeSet, IndexMap or a sorted Vec if the container is iterated, or\n\
         \u{20}\u{20}- add `// det: lookup-only` (keyed lookup/insert/remove/contains only), or\n\
         \u{20}\u{20}- add `// det: order-independent` (iterated, but the result cannot depend on order).\n\
         Aliases (`use .. HashMap as X`, `type X = HashMap<..>`) are never allowed.\n\
         See crates/ironhold_core/src/CLAUDE.md, \"Deterministic iteration order on gameplay paths\".\n\n{}",
        violations.len(),
        violations.join("\n")
    );
}

// ── The scanner itself ─────────────────────────────────────────────────────────────────────────

#[test]
fn scanner_flags_types_and_constructors() {
    assert!(line_violates("    pub map: HashMap<String, f32>,"));
    assert!(line_violates("let mut s = HashSet::new();"));
    assert!(line_violates("let m: std::collections::HashMap<K, V> = Default::default();"));
    assert!(line_violates("x: bevy::platform::collections::HashMap<K, V>,"));
    assert!(line_violates("x: FxHashMap<K, V>,"));
    assert!(line_violates("x: EntityHashSet::default(),"));
    assert!(line_violates("let m = std::collections::HashMap::with_capacity(4);"));
    assert!(line_violates("HashMap <String, f32>"), "whitespace before `<` must not evade the scan");
}

#[test]
fn scanner_ignores_imports_comments_and_marked_lines() {
    assert!(!line_violates("use std::collections::HashMap;"));
    assert!(!line_violates("use std::collections::{HashMap, HashSet};"));
    assert!(!line_violates("    /// A `HashMap<String, f32>` would be wrong here."));
    assert!(!line_violates("    // HashMap::new() is forbidden"));
    assert!(!line_violates("let x = 1; // mentions HashMap<K, V> only in a comment"));
    assert!(!line_violates("let m: HashMap<String, f32> = HashMap::new(); // det: lookup-only"));
    assert!(!line_violates("    retain: HashMap<K, V>, // det: order-independent (pure retain)"));
    assert!(!line_violates("let ordered: BTreeMap<String, f32> = BTreeMap::new();"));
    assert!(!line_violates("let ordered: IndexMap<String, f32> = IndexMap::new();"));
}

#[test]
fn scanner_does_not_treat_other_identifiers_as_hash_containers() {
    assert!(!line_violates("let hash_map_like = 3;"));
    assert!(!line_violates("fn hashmap_helper() {}"));
}

#[test]
fn aliases_cannot_be_hidden_behind_a_marker() {
    assert!(line_violates("use std::collections::HashMap as Map;"));
    assert!(line_violates("use std::collections::{HashSet as Set, BTreeMap};"));
    assert!(line_violates("pub use std::collections::HashMap as Map; // det: lookup-only"));
    assert!(line_violates("type Lookup = HashMap<String, f32>; // det: lookup-only"));
    assert!(line_violates("pub type Seen = std::collections::HashSet<u32>;"));
    assert!(!line_violates("type Ordered = BTreeMap<String, f32>;"));
}

#[test]
fn test_module_body_is_exempt_but_code_after_it_is_scanned() {
    let text = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let _m: HashMap<u8, u8> = HashMap::new(); }\n}\nfn b() { let _m: HashSet<u8> = HashSet::new(); }\n";
    let kept: Vec<&str> = non_test_lines(text).into_iter().map(|(_, l)| l).collect();
    assert!(!kept.iter().any(|l| l.contains("HashMap")), "test module body must be skipped: {kept:?}");
    assert!(kept.iter().any(|l| l.contains("HashSet")), "code after the test module must be scanned: {kept:?}");
}

#[test]
fn doc_comment_mentioning_cfg_test_does_not_switch_the_scan_off() {
    let text = "/// See the `#[cfg(test)]` module below.\nfn a() { let _m: HashMap<u8, u8> = HashMap::new(); }\n";
    let kept = non_test_lines(text);
    assert_eq!(kept.len(), 2);
    assert!(kept.iter().any(|(_, l)| line_violates(l)));
}

#[test]
fn nested_braces_in_the_test_module_do_not_end_the_skip_early() {
    let text = "#[cfg(test)]\nmod tests {\n    fn t() {\n        if true { let _m = HashMap::<u8, u8>::new(); }\n    }\n}\nfn after() {}\n";
    let kept: Vec<&str> = non_test_lines(text).into_iter().map(|(_, l)| l).collect();
    assert_eq!(kept, vec!["fn after() {}"]);
}

#[test]
fn out_of_line_test_module_exempts_only_its_own_line() {
    let text = "#[cfg(test)]\nmod tests;\nfn real() { let _m: HashMap<u8, u8> = HashMap::new(); }\n";
    let kept: Vec<&str> = non_test_lines(text).into_iter().map(|(_, l)| l).collect();
    assert_eq!(kept, vec!["fn real() { let _m: HashMap<u8, u8> = HashMap::new(); }"]);
}

#[test]
#[should_panic(expected = "unbalanced braces")]
fn unbalanced_test_module_fails_loudly_instead_of_exempting_the_rest_of_the_file() {
    // a literal `{` inside a string leaves the depth above zero until EOF
    let text = "#[cfg(test)]\nmod tests {\n    fn t() { let _s = \"{\"; }\n";
    let _ = non_test_lines(text);
}
