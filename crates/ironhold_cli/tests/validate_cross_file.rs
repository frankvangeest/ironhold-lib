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
