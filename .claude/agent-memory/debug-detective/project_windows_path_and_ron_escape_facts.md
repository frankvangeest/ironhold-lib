---
name: windows-path-and-ron-escape-facts
description: Empirically-verified Rust/Windows path facts for path-checking code — verbatim \\?\ joins DO normalize forward slashes, and a single backslash in a RON string is a parse error
metadata:
  type: project
---

Two facts that keep getting theorised wrongly in path-validation reviews here. Both verified
empirically on this machine (standalone `rustc -O` probe + the real `ironhold` binary), not from
docs.

**1. `PathBuf::push`/`join` normalises `/` → `\` even onto a `\\?\` verbatim base.**
`std::fs::canonicalize` on Windows returns `\\?\C:\...`, and the folklore is that verbatim paths
treat `/` as a literal filename character, so `verbatim_root.join("shared/models/hero.glb")` would
fail to resolve. It does **not** — Rust's `push` rewrites the separators, and `is_file()` /
`exists()` / `metadata()` / `read_dir()` all returned the correct result. So "canonicalize breaks
forward-slash joins on Windows" is a false alarm. (`std::path::absolute()` is still the better
choice for a path-resolution helper — lexical only, no filesystem hit, no symlink resolution — just
not for that reason.)

**2. A single backslash inside a RON string literal is a hard parse error.**
`(path: "shared\models\hero.glb")` fails with `Unknown escape character` *before* any validator
sees it. A backslash path can only reach a checker as `"shared\\models\\hero.glb"`, which RON
decodes to one backslash. Consequence: Rust-side checks (which see the decoded string) and
`tools/asset_checker/check.py` (whose `extract_refs` regex reads the **raw file text**, so it sees
two backslashes) are looking at different strings for the same line. Any "mirror the Rust check in
Python" claim has to account for that — a `"\\" in ref` test happens to work for both, but a
byte-comparison against the decoded value would not.

**How to apply:** when reviewing a path-existence/case checker, don't accept (or write) a
`\\?\`-verbatim scare story without running the 20-line rustc probe; and don't design a
backslash-detection branch as if designers could author raw Windows paths in RON.

Related: [[windows-case-sensitivity-probing]], [[cli-validate-never-calls-schema-validate]]
