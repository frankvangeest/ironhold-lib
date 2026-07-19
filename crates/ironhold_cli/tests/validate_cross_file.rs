use std::path::Path;
use std::process::Command;

fn ironhold() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ironhold"))
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn validate(fixture_name: &str) -> (i32, String) {
    let out = ironhold()
        .args(["validate"])
        .arg(fixture(fixture_name))
        .output()
        .unwrap_or_else(|e| panic!("failed to run ironhold: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), stdout)
}

fn validate_strict(fixture_name: &str) -> (i32, String) {
    let out = ironhold()
        .args(["validate", "--strict"])
        .arg(fixture(fixture_name))
        .output()
        .unwrap_or_else(|e| panic!("failed to run ironhold: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), stdout)
}

// ── Valid project ─────────────────────────────────────────────────────────────

#[test]
fn valid_project_exits_0() {
    let (code, _) = validate("valid_project");
    assert_eq!(code, 0);
}

// ── Cross-file reference errors ───────────────────────────────────────────────

#[test]
fn missing_effect_key_exits_1() {
    let (code, stdout) = validate("bad_effect_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_effect"),
        "expected 'missing_effect' in output:\n{stdout}"
    );
}

#[test]
fn missing_audio_key_exits_1() {
    let (code, stdout) = validate("bad_audio_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_sound"),
        "expected 'missing_sound' in output:\n{stdout}"
    );
}

#[test]
fn missing_prefab_in_scene_exits_1() {
    let (code, stdout) = validate("bad_prefab_in_scene");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_prefab"),
        "expected 'missing_prefab' in output:\n{stdout}"
    );
}

#[test]
fn missing_prefab_in_spawn_action_exits_1() {
    let (code, stdout) = validate("bad_prefab_in_spawn");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_prefab"),
        "expected 'missing_prefab' in output:\n{stdout}"
    );
}

#[test]
fn missing_behavior_file_exits_1() {
    let (code, stdout) = validate("bad_behavior_file");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("ghost.behavior.ron"),
        "expected 'ghost.behavior.ron' in output:\n{stdout}"
    );
}

#[test]
fn missing_foliage_leaf_texture_exits_1() {
    let (code, stdout) = validate("bad_foliage_texture");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("nonexistent_leaf"),
        "expected 'nonexistent_leaf' in output:\n{stdout}"
    );
}

#[test]
fn primitive_player_on_terrain_exits_1() {
    let (code, stdout) = validate("primitive_player_on_terrain");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("primitive_player") && stdout.contains("terrain"),
        "expected a primitive-player-on-terrain error in output:\n{stdout}"
    );
}

#[test]
fn unparseable_action_bar_key_exits_1() {
    let (code, stdout) = validate("bad_action_bar_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("MouseLeft"),
        "expected the unrecognised key name in output:\n{stdout}"
    );
}

#[test]
fn duplicate_resolved_action_bar_key_exits_1() {
    let (code, stdout) = validate("duplicate_action_bar_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("q") && stdout.contains("KeyQ"),
        "expected both colliding slot keys in output:\n{stdout}"
    );
}

/// Per-player action bars (Phase 2, `per_player_split_screen_targeting.md`) are the first
/// feature to author 2+ `ActionBar`s in one scene — a shared slot key across different bars must
/// be caught too, not just within one bar's own slots (the intent/cooldown pipeline is keyed by
/// slot_key alone, scene-wide).
#[test]
fn cross_bar_duplicate_action_bar_key_exits_1() {
    let (code, stdout) = validate("cross_bar_duplicate_action_bar_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bar_p1") && stdout.contains("bar_p2"),
        "expected both colliding bar ids in output:\n{stdout}"
    );
}

/// `player_stat_widgets.md` Part C: a `stat_label`/`world_stat_bar` keyed `"{self}.<stat>"` with
/// no matching `stat_templates` entry on that SAME prefab used to render empty forever with no
/// diagnostic — this cross-file check (and its scene-load `warn!` counterpart) catches it.
/// Generic across every prefab kind, not player-specific — this fixture uses a plain `Primitive`
/// prop precisely to prove that.
#[test]
fn missing_stat_widget_template_exits_1() {
    let (code, stdout) = validate("bad_stat_widget_template");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bad_widget_prop") && stdout.contains("stat_templates has no entry"),
        "expected the offending prefab key and the missing-template message in output:\n{stdout}"
    );
}

// ── Parse error ───────────────────────────────────────────────────────────────

#[test]
fn parse_error_exits_1() {
    let (code, stdout) = validate("parse_error");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("ERROR"),
        "expected 'ERROR' in output:\n{stdout}"
    );
}

// ── Strict mode ───────────────────────────────────────────────────────────────

#[test]
fn valid_project_strict_exits_0() {
    let (code, _) = validate_strict("valid_project");
    assert_eq!(code, 0);
}

#[test]
fn orphan_prefab_without_strict_exits_0() {
    let (code, _) = validate("orphan_prefab");
    assert_eq!(code, 0, "orphan prefab without --strict should exit 0");
}

#[test]
fn orphan_prefab_strict_exits_1() {
    let (code, stdout) = validate_strict("orphan_prefab");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("orphan_prop"),
        "expected 'orphan_prop' in output:\n{stdout}"
    );
}

#[test]
fn orphan_effect_strict_exits_1() {
    let (code, stdout) = validate_strict("orphan_effect");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("orphan_smoke"),
        "expected 'orphan_smoke' in output:\n{stdout}"
    );
}

#[test]
fn orphan_audio_strict_exits_1() {
    let (code, stdout) = validate_strict("orphan_audio");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("orphan_sound"),
        "expected 'orphan_sound' in output:\n{stdout}"
    );
}
