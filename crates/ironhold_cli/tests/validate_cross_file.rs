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

/// `gamepad_action_bar_slots.md`: an unrecognised `gamepad_key` name is a distinct check from the
/// keyboard `key` check above — same shape, different field.
#[test]
fn unparseable_action_bar_gamepad_key_exits_1() {
    let (code, stdout) = validate("bad_action_bar_gamepad_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("NotAButton"),
        "expected the unrecognised gamepad_key name in output:\n{stdout}"
    );
}

/// `gamepad_action_bar_slots.md`: the same player binding 2+ slots to the same gamepad button is
/// a same-player double-fire risk — a different failure mode than the keyboard cross-bar check
/// (the intent/cooldown pipeline is never keyed by `gamepad_key`), so it gets its own check.
#[test]
fn same_player_gamepad_duplicate_key_exits_1() {
    let (code, stdout) = validate("same_player_gamepad_duplicate_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bar_p0_a") && stdout.contains("bar_p0_b"),
        "expected both colliding bar ids in output:\n{stdout}"
    );
}

/// Two *different* players' bars both binding `gamepad_key: "South"` must NOT be flagged — each
/// player has their own physical pad, so this isn't a real collision (unlike the keyboard case,
/// which is genuinely shared hardware).
#[test]
fn gamepad_action_bar_different_players_share_button_exits_0() {
    let (code, stdout) = validate("gamepad_action_bar_different_players_share_button");
    assert_eq!(code, 0, "expected exit 0 (no false collision), got {code}:\n{stdout}");
}

/// An omitted `owner_player` and an explicit `owner_player: 0` both mean "the primary player" —
/// the same `unwrap_or(0)` normalization the runtime's `owns_slot` uses. Two bars sharing a
/// `gamepad_key` this way must collide exactly like two bars both writing `owner_player: 0`
/// would.
#[test]
fn gamepad_action_bar_omitted_owner_matches_explicit_zero_exits_1() {
    let (code, stdout) = validate("gamepad_action_bar_omitted_owner_matches_explicit_zero");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bar_omitted") && stdout.contains("bar_explicit_zero"),
        "expected both colliding bar ids in output:\n{stdout}"
    );
}

/// A `gamepad_key`-bound slot for a player whose prefab sets no `inputs.gamepad_index` at all is
/// silently inert at runtime (no crash, no console message) — this cross-file check is the only
/// diagnostic for it, mirroring `missing_player_stat_template`'s owner_player -> prefab
/// cross-check shape exactly.
#[test]
fn gamepad_key_without_gamepad_index_exits_1() {
    let (code, stdout) = validate("gamepad_key_without_gamepad_index");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("skill_bar") && stdout.contains("gamepad_index"),
        "expected the bar id and a mention of gamepad_index in output:\n{stdout}"
    );
}

/// Same shape as above but the player prefab DOES set `inputs.gamepad_index` — must not be
/// flagged, proving the check is genuinely about the pairing being present, not about
/// `gamepad_key` itself.
#[test]
fn gamepad_key_with_gamepad_index_exits_0() {
    let (code, stdout) = validate("gamepad_key_with_gamepad_index");
    assert_eq!(code, 0, "expected exit 0 (pairing present, no error), got {code}:\n{stdout}");
}

/// `gamepad_player_binding_hardening.md`: two player-tagged prefabs **instantiated in the same
/// scene** authoring the same non-`None` `gamepad_index` — one physical controller would drive
/// both characters at once. Must be flagged with a hard error, not just a runtime `warn!` (see
/// the matching `scene_loader.rs::warn_duplicate_gamepad_index`, which is scene-load-only and not
/// directly unit-testable from `ironhold_core`'s test harness — this CLI check is the one
/// automated place this scenario is verified).
#[test]
fn duplicate_gamepad_index_same_scene_exits_1() {
    let (code, stdout) = validate("duplicate_gamepad_index_same_scene");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("player_01") && stdout.contains("player_02") && stdout.contains("gamepad_index"),
        "expected both colliding entity ids and a mention of gamepad_index in output:\n{stdout}"
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

/// `flycam_scene_conflicts.md`: 2+ `tags: ["flycam"]` entities in one scene silently keep only
/// the last one at runtime (`scene_loader.rs`) — this is the design-time counterpart.
#[test]
fn duplicate_flycam_entity_exits_1() {
    let (code, stdout) = validate("duplicate_flycam_entity");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("camera_a") && stdout.contains("camera_b") && stdout.contains("flycam"),
        "expected both colliding entity ids and a mention of flycam in output:\n{stdout}"
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

// ── camera_modes registry (camera_modes.md v2) ─────────────────────────────────

#[test]
fn camera_mode_reserved_default_key_exits_1() {
    let (code, stdout) = validate("camera_mode_reserved_default_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reserved key"),
        "expected 'reserved key' in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_party_in_registry_exits_1() {
    let (code, stdout) = validate("camera_mode_party_in_registry");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("cannot be reached via SetCameraMode"),
        "expected 'cannot be reached via SetCameraMode' in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_missing_look_at_entity_exits_1() {
    let (code, stdout) = validate("camera_mode_missing_look_at_entity");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("look_at_entity"),
        "expected 'look_at_entity' in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_unknown_set_camera_mode_reference_exits_1() {
    let (code, stdout) = validate("camera_mode_unknown_set_camera_mode");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("nonexistent_preset"),
        "expected 'nonexistent_preset' in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_valid_registry_and_reference_exits_0() {
    let (code, stdout) = validate("camera_mode_valid_registry_and_reference");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}
