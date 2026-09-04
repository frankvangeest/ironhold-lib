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
fn new_id_token_outside_spawn_id_exits_1() {
    let (code, stdout) = validate("bad_new_id_placement");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("only resolves inside Action::Spawn"),
        "expected the misplaced-{{new_id}} message in output:\n{stdout}"
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
fn missing_scene_path_in_load_scene_exits_1() {
    let (code, stdout) = validate("bad_scene_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("does_not_exist.scene.ron") && stdout.contains("not found on disk"),
        "expected the missing LoadScene path in output:\n{stdout}"
    );
    assert!(
        stdout.contains("also_does_not_exist.scene.ron"),
        "expected the missing ToggleOverlay path in output too:\n{stdout}"
    );
}

#[test]
fn missing_initial_scene_exits_1() {
    let (code, stdout) = validate("bad_initial_scene");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_boot_scene.scene.ron") && stdout.contains("initial_scene"),
        "expected the missing initial_scene path in output:\n{stdout}"
    );
}

#[test]
fn missing_items_path_target_exits_1() {
    let (code, stdout) = validate("bad_items_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("items_path") && stdout.contains("does not exist on disk"),
        "expected a diagnostic for the missing items_path target in output:\n{stdout}"
    );
}

#[test]
fn missing_merchant_currency_stat_exits_1() {
    let (code, stdout) = validate("bad_merchant_currency_stat");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("silver") && stdout.contains("not found in stats.ron"),
        "expected the missing currency_stat in output:\n{stdout}"
    );
}

#[test]
fn missing_merchant_item_key_exits_1() {
    let (code, stdout) = validate("bad_merchant_item_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("iron_sword") && stdout.contains("not found in items.ron"),
        "expected the missing item_key in output:\n{stdout}"
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
fn dialogue_parse_error_exits_1() {
    // ironhold_cli validate never parsed dialogues/*.dialogue.ron at all before this fix — a
    // typo'd field (here: jump_to0 instead of jump_to) produced zero diagnostic. Confirms the
    // file is now actually deserialized, not just cross-checked.
    let (code, stdout) = validate("bad_dialogue_parse");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("broken.dialogue.ron"),
        "expected 'broken.dialogue.ron' in output:\n{stdout}"
    );
    assert!(
        stdout.contains("jump_to0"),
        "expected the unexpected field name in output:\n{stdout}"
    );
}

#[test]
fn dialogue_do_actions_missing_effect_key_exits_1() {
    // collect_actions previously skipped dialogue do_actions entirely for cross-file checks.
    // Confirms a dialogue choice's action is now walked the same as a rule's, with the source
    // label correctly pointing at the dialogue file (not silently defaulting to some other path).
    let (code, stdout) = validate("bad_dialogue_action_reference");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_effect"),
        "expected 'missing_effect' in output:\n{stdout}"
    );
    assert!(
        stdout.contains("dialogues/npc.dialogue.ron"),
        "expected the error to be attributed to the dialogue file, not some other source:\n{stdout}"
    );
}

#[test]
fn missing_prefab_dialogue_path_exits_1() {
    // PrefabDef.dialogue had no existence check while its structural twin PrefabDef.behavior
    // did — same silent-failure class this whole feature exists to close.
    let (code, stdout) = validate("bad_dialogue_prefab_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("does_not_exist.dialogue.ron"),
        "expected the missing dialogue path in output:\n{stdout}"
    );
}

#[test]
fn missing_start_dialogue_action_path_exits_1() {
    // Action::StartDialogue.dialogue_path had no existence check while LoadScene/
    // LoadSceneOverlay/PreloadScene/ToggleOverlay did.
    let (code, stdout) = validate("bad_dialogue_action_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("does_not_exist.dialogue.ron"),
        "expected the missing StartDialogue path in output:\n{stdout}"
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
fn label_font_size_zero_exits_1() {
    let (code, stdout) = validate("bad_font_size");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("broken_label") && stdout.contains("font_size"),
        "expected an invalid_font_size error naming the label in output:\n{stdout}"
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

/// `flycam_model_never_renders_warning.md`: a `tags: ["flycam"]` prefab's `model:` is silently
/// discarded at scene load (`scene_loader.rs`) — this is the design-time counterpart. Scoped to
/// the prefab catalog, so this reports once per offending *prefab*, not per scene entity.
#[test]
fn flycam_model_never_renders_exits_1() {
    let (code, stdout) = validate("flycam_model_never_renders");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("flycam_with_body") && stdout.contains("model") && stdout.contains("never render"),
        "expected the offending prefab key and a mention of the ignored field in output:\n{stdout}"
    );
}

/// Same check, `children:` half — a composite `kind: Primitive` flycam prefab. Distinct code path
/// from `model:` (`flycam_ignored_fields()`'s `children` branch), previously untested.
#[test]
fn flycam_children_never_render_exits_1() {
    let (code, stdout) = validate("flycam_children_never_render");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("flycam_with_children") && stdout.contains("children") && stdout.contains("never render"),
        "expected the offending prefab key and a mention of the ignored field in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("model") || stdout.contains("children"),
        "remedy must not tell a children-only offender to set model: \"\" as if that were the cause:\n{stdout}"
    );
}

/// Same check, `shape`/`primitive` half — a `kind: Primitive` flycam prefab authored the
/// idiomatic single-shape way (no `model:`, no `children:`), the dominant authoring style for
/// every other visible `Primitive` prefab in this repo's example projects. Previously a total
/// blind spot: neither `model` nor `children` fire for this shape.
#[test]
fn flycam_shape_never_renders_exits_1() {
    let (code, stdout) = validate("flycam_shape_never_renders");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("flycam_with_shape") && stdout.contains("shape/primitive") && stdout.contains("never render"),
        "expected the offending prefab key and a mention of the ignored field in output:\n{stdout}"
    );
}

/// `flycam_model_never_renders_warning.md`: a prefab tagged both `"player"` and `"flycam"` never
/// spawns its player components at all (the flycam branch `continue`s first) — distinct error
/// from the ignored-fields case above since the fix and the failure are different. Asserts the
/// distinguishing phrase, not just "player"/"flycam" (both appear in the *other* message's
/// remedy text too, so a looser assertion couldn't tell the two error types apart).
#[test]
fn flycam_player_tag_conflict_exits_1() {
    let (code, stdout) = validate("flycam_player_tag_conflict");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("confused_flying_player") && stdout.contains("never spawn at all"),
        "expected the offending prefab key and the dual-tag-specific failure text in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("never render"),
        "a dual-tagged prefab must get only the dual-tag error, not also the ignored-fields error:\n{stdout}"
    );
}

/// See `planning/features/done/uphill_jump_lock.md`. `bad_jump_player` sets `jump: Fixed(height: 0.2)`
/// — well under the default ground-check reach (collider_radius 0.4 + ground_cast_length 0.3 =
/// 0.7m) — so its jump can never ballistically clear the sensor, even on flat ground. A
/// `--strict`-only warning, not a hard error: the runtime's `jump_air_grace` fallback keeps this
/// from actually breaking the jump (see `planning/features/done/uphill_jump_lock.md`), matching the
/// scene-load side (`warn_jump_cannot_clear_ground_sensor`, a `warn!`, not a rejected spawn).
#[test]
fn jump_cannot_clear_ground_sensor_without_strict_exits_0() {
    let (code, _) = validate("bad_jump_ground_sensor");
    assert_eq!(code, 0, "jump-sensor-reach misconfiguration without --strict should exit 0");
}

#[test]
fn jump_cannot_clear_ground_sensor_strict_exits_1() {
    let (code, stdout) = validate_strict("bad_jump_ground_sensor");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bad_jump_player")
            && stdout.contains("does not clear this player's ground-check reach"),
        "expected the offending prefab key and the jump-sensor-reach message in output:\n{stdout}"
    );
}

/// See `planning/features/done/uphill_jump_lock.md`. `bad_slope_player` sets
/// `max_walkable_slope_deg: 0.0` — outside the valid `(0, 90]` range — which silently breaks
/// grounding entirely (no surface is ever walkable) rather than just mis-tuning slope behavior.
/// `--strict`-only, matching `jump_cannot_clear_ground_sensor`'s severity.
#[test]
fn invalid_walkable_slope_limit_without_strict_exits_0() {
    let (code, _) = validate("bad_walkable_slope_limit");
    assert_eq!(code, 0, "invalid max_walkable_slope_deg without --strict should exit 0");
}

#[test]
fn invalid_walkable_slope_limit_strict_exits_1() {
    let (code, stdout) = validate_strict("bad_walkable_slope_limit");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bad_slope_player") && stdout.contains("outside the valid"),
        "expected the offending prefab key and the range message in output:\n{stdout}"
    );
}

// ── coyote_time_secs negative (soft, --strict only) ────────────────────────────
//
// Unlike `max_walkable_slope_deg`, a negative `coyote_time_secs` doesn't break grounding at all —
// it silently launders to a zero-tick (disabled) buffer, same as `0.0` — so this is `--strict`-only
// too, but for a different reason: it's flagging a likely typo (a negative value spelling "off"
// when `0.0` already does that unambiguously), not a design-time misconfiguration that breaks a
// feature. See `planning/features/done/uphill_jump_lock.md`'s coyote-time section.
#[test]
fn negative_coyote_time_secs_without_strict_exits_0() {
    let (code, _) = validate("bad_coyote_time");
    assert_eq!(code, 0, "negative coyote_time_secs without --strict should exit 0");
}

#[test]
fn negative_coyote_time_secs_strict_exits_1() {
    let (code, stdout) = validate_strict("bad_coyote_time");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("bad_coyote_player") && stdout.contains("negative"),
        "expected the offending prefab key and the negative-value message in output:\n{stdout}"
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

// ── Action::Spawn spawn_point reference (planning/backlog.md) ──────────────────

#[test]
fn bad_spawn_point_exits_1() {
    let (code, stdout) = validate("bad_spawn_point");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("typo_spawn"),
        "expected 'typo_spawn' in output:\n{stdout}"
    );
}

#[test]
fn valid_spawn_point_exits_0() {
    let (code, stdout) = validate("valid_spawn_point");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// `spawn_point` is `{self}`/`{target}`-substituted at interpret time (message_interpreter.rs,
/// dialogue.rs) — a templated value like `"{self}_spawn"`, used to share one behavior rule across
/// several named spawn points, is not the literal runtime key and must not be checked as one.
#[test]
fn spawn_point_self_substitution_no_false_positive_exits_0() {
    let (code, stdout) = validate("spawn_point_self_substitution");
    assert_eq!(code, 0, "expected exit 0 (templated spawn_point), got {code}:\n{stdout}");
}

// ── label_depth_scale (planning/features/label_depth_scale_validation.md) ─────

/// `min_scale` above 1.0 pins every depth-scaled widget in the scene forever — a hard error,
/// not `--strict`-gated (unlike `reference_distance` below, which is a heuristic).
#[test]
fn label_depth_scale_min_scale_too_high_exits_1() {
    let (code, stdout) = validate("label_depth_scale_min_scale_too_high");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("min_scale") && stdout.contains("1.5") && stdout.contains("pin"),
        "expected the min_scale value and the pin consequence in output:\n{stdout}"
    );
}

/// A negative `min_scale` is inert (never binds against an already-non-negative ratio), not a
/// pin — still a hard error, just a different documented consequence in the message.
#[test]
fn label_depth_scale_min_scale_negative_exits_1() {
    let (code, stdout) = validate("label_depth_scale_min_scale_negative");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("min_scale") && stdout.contains("inert"),
        "expected the min_scale field and the inert-no-op consequence in output:\n{stdout}"
    );
}

/// `reference_distance` inside the scene's reachable Orbit camera range must not warn, even
/// under `--strict`.
#[test]
fn label_depth_scale_reference_distance_in_range_exits_0() {
    let (code, stdout) = validate_strict("label_depth_scale_reference_distance_in_range");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// `reference_distance` far outside the scene's reachable Orbit camera range is `--strict`-only
/// (a heuristic band, not a provable misconfiguration) — plain `validate` must stay clean.
#[test]
fn label_depth_scale_reference_distance_out_of_range_without_strict_exits_0() {
    let (code, _) = validate("label_depth_scale_reference_distance_out_of_range");
    assert_eq!(code, 0, "reference_distance misconfiguration without --strict should exit 0");
}

#[test]
fn label_depth_scale_reference_distance_out_of_range_strict_exits_1() {
    let (code, stdout) = validate_strict("label_depth_scale_reference_distance_out_of_range");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reference_distance") && stdout.contains("500"),
        "expected the reference_distance value in output:\n{stdout}"
    );
}

/// `CameraModeDef::Follow` contributes a fixed point (`offset.length()`, both bounds) to the
/// camera-range union rather than being skipped like Fixed/FirstPerson/Flycam — proven here by a
/// scene with ONLY a Follow-mode player, where `reference_distance` is set well below that fixed
/// distance's own band.
#[test]
fn label_depth_scale_follow_camera_narrows_band_strict_exits_1() {
    let (code, stdout) = validate_strict("label_depth_scale_follow_camera_narrows_band");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reference_distance"),
        "expected a reference_distance warning in output:\n{stdout}"
    );
}

/// A player whose only camera is `Fixed` (no radius concept) must not trigger the check, no
/// matter how absurd `reference_distance` is — there's no meaningful range to compare against.
#[test]
fn label_depth_scale_fixed_camera_only_no_warn_exits_0() {
    let (code, stdout) = validate_strict("label_depth_scale_fixed_camera_only_no_warn");
    assert_eq!(code, 0, "expected exit 0 (no radius-bearing camera, skip), got {code}:\n{stdout}");
}

/// A scene with no player prefabs at all must not trigger the check or crash.
#[test]
fn label_depth_scale_no_players_no_warn_exits_0() {
    let (code, stdout) = validate_strict("label_depth_scale_no_players_no_warn");
    assert_eq!(code, 0, "expected exit 0 (no players, skip), got {code}:\n{stdout}");
}

/// Two players with very different Orbit ranges: `reference_distance` is far outside the first
/// player's own tight range, but well inside the *union* with the second player's much wider
/// range. Proves the union approach (fewer false positives) rather than a per-player-worst-case
/// check, matching the plan's documented design choice.
#[test]
fn label_depth_scale_split_screen_union_no_false_positive_exits_0() {
    let (code, stdout) = validate_strict("label_depth_scale_split_screen_union");
    assert_eq!(code, 0, "expected exit 0 (in range via union), got {code}:\n{stdout}");
}

/// `3rd_person_game_demo`'s own player is spawned entirely via `Action::Spawn` in
/// `state_machine.ron`'s entry_actions, never appearing in `scene.entities` — this fixture
/// mirrors that exact pattern to prove the check also scans Action::Spawn for player-tagged
/// prefabs, not just scene-placed entities.
#[test]
fn label_depth_scale_dynamic_spawn_reference_distance_strict_exits_1() {
    let (code, stdout) = validate_strict("label_depth_scale_dynamic_spawn_reference_distance");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reference_distance") && stdout.contains("500"),
        "expected the reference_distance value in output:\n{stdout}"
    );
}

/// `min_scale` exactly at the boundary values (`0.0`, `1.0`) is valid, not out-of-range — the
/// hard error check uses an inclusive `0.0..=1.0` range, must not off-by-one.
#[test]
fn label_depth_scale_min_scale_zero_boundary_exits_0() {
    let (code, stdout) = validate("label_depth_scale_min_scale_zero_boundary");
    assert_eq!(code, 0, "expected exit 0 (0.0 is a valid boundary), got {code}:\n{stdout}");
}

#[test]
fn label_depth_scale_min_scale_one_boundary_exits_0() {
    let (code, stdout) = validate("label_depth_scale_min_scale_boundary_values");
    assert_eq!(code, 0, "expected exit 0 (1.0 is a valid boundary), got {code}:\n{stdout}");
}

/// A NaN `reference_distance` must not silently escape the check — `NaN < x` and `NaN > y` are
/// both `false` in Rust, so a naive band comparison would let it through even though
/// `(NaN / dist).min(1.0)` at runtime is `1.0`, meaning scaling silently never engages (exactly
/// the failure mode this feature exists to catch).
#[test]
fn label_depth_scale_reference_distance_nan_strict_exits_1() {
    let (code, stdout) = validate_strict("label_depth_scale_reference_distance_nan");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reference_distance") && stdout.contains("NaN"),
        "expected a NaN reference_distance warning in output:\n{stdout}"
    );
}

/// A `Follow` camera with a degenerate zero-length `offset` must not corrupt the band into
/// `(0.0, 0.0)`, which would make any positive `reference_distance` falsely fail — it must be
/// treated as contributing no radius information at all (same as "no radius-bearing camera").
#[test]
fn label_depth_scale_follow_zero_offset_no_warn_exits_0() {
    let (code, stdout) = validate_strict("label_depth_scale_follow_zero_offset_no_warn");
    assert_eq!(code, 0, "expected exit 0 (degenerate offset contributes nothing), got {code}:\n{stdout}");
}

/// A `tags: ["flycam"]` entity suppresses every player camera in the scene
/// (`SuppressPlayerCameras`) — a player prefab's Orbit range must NOT be unioned in when a
/// flycam is present, since that camera never actually spawns.
#[test]
fn label_depth_scale_flycam_with_player_no_warn_exits_0() {
    let (code, stdout) = validate_strict("label_depth_scale_flycam_with_player_no_warn");
    assert_eq!(code, 0, "expected exit 0 (flycam suppresses player camera), got {code}:\n{stdout}");
}

/// `join_prefab_keys` (local-coop character-select variants) are player-tagged prefabs reachable
/// independently of `scene.entities` — omitting them from the union would narrow the band and
/// risk a false positive.
#[test]
fn label_depth_scale_join_prefab_keys_union_strict_exits_1() {
    let (code, stdout) = validate_strict("label_depth_scale_join_prefab_keys_union");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reference_distance") && stdout.contains("500"),
        "expected the reference_distance value in output:\n{stdout}"
    );
}

/// A player prefab authoring neither `camera` nor `camera_mode` falls back to
/// `default_camera_config()` (min_radius 2.0 / max_radius 20.0) at spawn time — the check must
/// exercise that fallback, not treat a camera-less player prefab as "no radius-bearing camera"
/// and wrongly skip.
#[test]
fn label_depth_scale_default_camera_config_fallback_strict_exits_1() {
    let (code, stdout) = validate_strict("label_depth_scale_default_camera_config_fallback");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("reference_distance") && stdout.contains("500"),
        "expected the reference_distance value in output:\n{stdout}"
    );
}
