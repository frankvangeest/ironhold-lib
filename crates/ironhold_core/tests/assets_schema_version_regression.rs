use ironhold_core::schema::{ProjectConfig, StateMachineAsset};
use ironhold_core::schema::project::LogicRulesAsset;
use ironhold_core::schema::scene_v2::GameSceneV2;
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog};
use ironhold_core::schema::player::AnimationPolicy;
use ron::extensions::Extensions;
use std::fs;
use std::path::{Path, PathBuf};

fn from_str<'de, T: serde::Deserialize<'de>>(s: &'de str) -> Result<T, ron::error::SpannedError> {
    ron::Options::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(s)
}

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
            let scene: GameSceneV2 = from_str(&contents)
                .unwrap_or_else(|e| panic!("GameSceneV2 failed to parse {}: {}", file.display(), e));
            scene.validate()
                .unwrap_or_else(|e| panic!("GameSceneV2 failed validation {}: {}", file.display(), e));
        }
    }

    // 3) Logic files: assets/projects/*/logic/rules.ron and logic/state_machine.ron
    let project_dirs = fs::read_dir(&projects_dir)
        .unwrap_or_else(|e| panic!("Failed to read projects dir {}: {}", projects_dir.display(), e));

    for entry in project_dirs.flatten() {
        let logic_dir = entry.path().join("logic");
        if !logic_dir.is_dir() { continue; }

        let rules_file = logic_dir.join("rules.ron");
        if rules_file.is_file() {
            let contents = fs::read_to_string(&rules_file)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", rules_file.display(), e));
            let _: LogicRulesAsset = from_str(&contents)
                .unwrap_or_else(|e| panic!("LogicRulesAsset failed to parse {}: {}", rules_file.display(), e));
        }

        let sm_file = logic_dir.join("state_machine.ron");
        if sm_file.is_file() {
            let contents = fs::read_to_string(&sm_file)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", sm_file.display(), e));
            let fsm: StateMachineAsset = from_str(&contents)
                .unwrap_or_else(|e| panic!("StateMachineAsset failed to parse {}: {}", sm_file.display(), e));
            fsm.validate()
                .unwrap_or_else(|e| panic!("StateMachineAsset failed validation {}: {}", sm_file.display(), e));
        }
    }

    // 4) Asset catalogs: assets/projects/*/assets.ron
    let project_dirs = fs::read_dir(&projects_dir)
        .unwrap_or_else(|e| panic!("Failed to read projects dir {}: {}", projects_dir.display(), e));

    for entry in project_dirs.flatten() {
        let catalog_file = entry.path().join("assets.ron");
        if !catalog_file.is_file() { continue; }

        let contents = fs::read_to_string(&catalog_file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", catalog_file.display(), e));
        let catalog: AssetCatalog = from_str(&contents)
            .unwrap_or_else(|e| panic!("AssetCatalog failed to parse {}: {}", catalog_file.display(), e));
        catalog.validate()
            .unwrap_or_else(|e| panic!("AssetCatalog failed validation {}: {}", catalog_file.display(), e));
    }

    // 5) Prefab catalogs: assets/projects/*/prefabs/prefabs.ron
    let project_dirs = fs::read_dir(&projects_dir)
        .unwrap_or_else(|e| panic!("Failed to read projects dir {}: {}", projects_dir.display(), e));

    for entry in project_dirs.flatten() {
        let prefab_file = entry.path().join("prefabs").join("prefabs.ron");
        if !prefab_file.is_file() { continue; }

        let contents = fs::read_to_string(&prefab_file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", prefab_file.display(), e));
        let catalog: PrefabCatalog = from_str(&contents)
            .unwrap_or_else(|e| panic!("PrefabCatalog failed to parse {}: {}", prefab_file.display(), e));
        catalog.validate()
            .unwrap_or_else(|e| panic!("PrefabCatalog failed validation {}: {}", prefab_file.display(), e));
    }

    // 6) Animation policies: assets/projects/*/prefabs/animation/*.ron
    let project_dirs = fs::read_dir(&projects_dir)
        .unwrap_or_else(|e| panic!("Failed to read projects dir {}: {}", projects_dir.display(), e));

    for entry in project_dirs.flatten() {
        let anim_dir = entry.path().join("prefabs").join("animation");
        if !anim_dir.is_dir() { continue; }

        let anim_files = fs::read_dir(&anim_dir)
            .unwrap_or_else(|e| panic!("Failed to read animation dir {}: {}", anim_dir.display(), e));

        for anim_entry in anim_files.flatten() {
            let path = anim_entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("ron") { continue; }

            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
            let _policy: AnimationPolicy = from_str(&contents)
                .unwrap_or_else(|e| panic!("AnimationPolicy failed to parse {}: {}", path.display(), e));
        }
    }
}
