use ironhold_core::schema::{GameLevel, ProjectConfig};
use ironhold_core::schema::scene_v2::GameSceneV2;
use ron::de::from_str;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_ron_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return; };

    for entry in entries.flatten() {
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

    let projects_dir = repo_root.join("assets").join("projects");

    // 1) Project configs: one file per project directory (direct children of assets/projects/)
    let project_dirs = fs::read_dir(&projects_dir)
        .unwrap_or_else(|e| panic!("Failed to read projects dir {}: {}", projects_dir.display(), e));

    let mut project_files: Vec<PathBuf> = Vec::new();
    for entry in project_dirs.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Collect only the top-level .project.ron files in each project directory
            if let Ok(entries) = fs::read_dir(&path) {
                for file_entry in entries.flatten() {
                    let fp = file_entry.path();
                    if fp.is_file() && fp.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.ends_with(".project.ron"))
                        .unwrap_or(false)
                    {
                        project_files.push(fp);
                    }
                }
            }
        }
    }

    assert!(!project_files.is_empty(), "No project .ron files found in {}", projects_dir.display());

    for file in &project_files {
        let contents = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        let cfg: ProjectConfig = from_str(&contents)
            .unwrap_or_else(|e| panic!("ProjectConfig failed to parse {}: {}", file.display(), e));
        cfg.validate()
            .unwrap_or_else(|e| panic!("ProjectConfig failed validation {}: {}", file.display(), e));
    }

    // 2) Levels: assets/projects/*/scenes/**/*.ron
    let mut level_files: Vec<PathBuf> = Vec::new();
    let project_dirs = fs::read_dir(&projects_dir)
        .unwrap_or_else(|e| panic!("Failed to read projects dir {}: {}", projects_dir.display(), e));

    for entry in project_dirs.flatten() {
        let scenes_dir = entry.path().join("scenes");
        if scenes_dir.is_dir() {
            collect_ron_files_recursive(&scenes_dir, &mut level_files);
        }
    }

    assert!(!level_files.is_empty(), "No scene .ron files found under {}", projects_dir.display());

    for file in &level_files {
        let contents = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        let file_name = file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if file_name.ends_with(".scene.ron") {
            // Scene v2 format
            let _scene: GameSceneV2 = from_str(&contents)
                .unwrap_or_else(|e| panic!("GameSceneV2 failed to parse {}: {}", file.display(), e));
        } else {
            // Scene v1 format (GameLevel)
            let level: GameLevel = from_str(&contents)
                .unwrap_or_else(|e| panic!("GameLevel failed to parse {}: {}", file.display(), e));
            level.validate()
                .unwrap_or_else(|e| panic!("GameLevel failed validation {}: {}", file.display(), e));
        }
    }
}
