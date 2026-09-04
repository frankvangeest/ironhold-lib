use std::path::Path;
use std::process::Command;

fn ironhold() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ironhold"))
}

fn project(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets/projects")
        .join(name)
}

fn validate(name: &str) {
    let status = ironhold()
        .args(["validate"])
        .arg(project(name))
        .status()
        .unwrap_or_else(|e| panic!("failed to run ironhold: {e}"));
    assert!(
        status.success(),
        "ironhold validate {name} exited with {}",
        status.code().unwrap_or(-1)
    );
}

#[test] fn validate_quick_scene()          { validate("quick_scene"); }
#[test] fn validate_3rd_person_game_demo() { validate("3rd_person_game_demo"); }
#[test] fn validate_terrain_demo()         { validate("terrain_demo"); }
#[test] fn validate_custom_materials()     { validate("custom_materials"); }
#[test] fn validate_primitive_world()      { validate("primitive_world"); }
#[test] fn validate_entity_logic_demo()    { validate("entity_logic_demo"); }
#[test] fn validate_particles_demo()       { validate("particles_demo"); }
#[test] fn validate_effect_mayhem_demo()   { validate("effect_mayhem_demo"); }
/// `gamepad_player_binding_hardening.md`: `local_coop_demo`'s catalog legitimately reuses
/// `gamepad_index` values across different rooms' player-prefab variants (e.g.
/// `player_p1_split`/`player_p1_split_ring` both author `gamepad_index: 0` — never
/// co-instantiated in the same scene) — the explicit negative case proving the duplicate-
/// `gamepad_index` check is scoped per-scene (each scene's own `entities:` list), not
/// per-catalog. Must still validate clean.
#[test] fn validate_local_coop_demo()      { validate("local_coop_demo"); }
#[test] fn validate_camera_modes()             { validate("camera_modes"); }
#[test] fn validate_dynamic_animation_control() { validate("dynamic_animation_control"); }
#[test] fn validate_foliage_demo()             { validate("foliage_demo"); }
#[test] fn validate_stats_demo()               { validate("stats_demo"); }
#[test] fn validate_blank_project()            { validate("blank_project"); }
