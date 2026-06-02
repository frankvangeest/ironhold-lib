/// RON style invariant tests.
///
/// These tests enforce authoring conventions that hold across all RON files in
/// `assets/projects/`. They catch mistakes that the schema round-trip tests miss
/// (e.g. writing `Some(Cuboid)` instead of `Cuboid` when `implicit_some` is active).
use std::fs;
use std::path::{Path, PathBuf};

fn collect_ron_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ron_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("ron") {
            out.push(path);
        }
    }
}

/// `implicit_some` is active at runtime, so `Some(...)` wrappers are never
/// needed in RON files. This test fails if any RON file under `assets/projects/`
/// contains `Some(`, catching copy-paste from Rust code or confusion about the
/// extension.
#[test]
fn no_explicit_some_in_ron_files() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().and_then(|p| p.parent())
        .expect("repo root two levels above crate");

    let projects_dir = repo_root.join("assets").join("projects");

    let mut ron_files = Vec::new();
    collect_ron_files(&projects_dir, &mut ron_files);
    assert!(!ron_files.is_empty(), "No RON files found under {}", projects_dir.display());

    let mut violations: Vec<String> = Vec::new();

    for path in &ron_files {
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

        for (line_no, line) in contents.lines().enumerate() {
            // Skip comments
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if line.contains("Some(") {
                violations.push(format!(
                    "{}:{}: `Some(` found — use bare value; `implicit_some` is active",
                    path.strip_prefix(repo_root).unwrap_or(path).display(),
                    line_no + 1,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "RON files must not use explicit `Some(...)` wrappers ({} violation{}):\n  {}",
        violations.len(),
        if violations.len() == 1 { "" } else { "s" },
        violations.join("\n  "),
    );
}
