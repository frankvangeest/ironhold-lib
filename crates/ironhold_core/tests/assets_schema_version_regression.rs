use ironhold_core::schema::{GameLevel, ProjectConfig};
use ron::de::from_str;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_ron_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read dir {}: {}", dir.display(), e));

    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("Failed to read dir entry in {}: {}", dir.display(), e));
        let path = entry.path();
        if path.is_dir() {
            collect_ron_files_recursive(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("ron") {
            out.push(path);
        }
    }
}

#[test]
fn regression_schema_version_in_assets() {
    // CARGO_MANIFEST_DIR points to crates/ironhold_core
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| panic!("Expected repo root at two levels above {}", crate_root.display()));

    let assets_dir = repo_root.join("assets");
    let scenes_dir = assets_dir.join("scenes");

    // 1) Project configs: assets/*.ron
    let root_entries = fs::read_dir(&assets_dir)
        .unwrap_or_else(|e| panic!("Failed to read assets dir {}: {}", assets_dir.display(), e));

    let mut project_files = Vec::new();
    for entry in root_entries {
        let path = entry.unwrap().path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("ron") {
            project_files.push(path);
        }
    }

    assert!(!project_files.is_empty(), "No project .ron files found in {}", assets_dir.display());

    for file in project_files {
        let contents = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        let cfg: ProjectConfig = from_str(&contents)
            .unwrap_or_else(|e| panic!("ProjectConfig failed to parse {}: {}", file.display(), e));
        cfg.validate()
            .unwrap_or_else(|e| panic!("ProjectConfig failed validation {}: {}", file.display(), e));
    }

    // 2) Levels: assets/scenes/**/*.ron
    let mut level_files = Vec::new();
    collect_ron_files_recursive(&scenes_dir, &mut level_files);

    assert!(!level_files.is_empty(), "No scene .ron files found under {}", scenes_dir.display());

    for file in level_files {
        let contents = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        let level: GameLevel = from_str(&contents)
            .unwrap_or_else(|e| panic!("GameLevel failed to parse {}: {}", file.display(), e));
        level.validate()
            .unwrap_or_else(|e| panic!("GameLevel failed validation {}: {}", file.display(), e));
    }
}
