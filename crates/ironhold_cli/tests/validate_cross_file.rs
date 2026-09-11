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

/// Invokes `ironhold validate .` with the process's own working directory set to the fixture --
/// the single most common real invocation shape (`cd` into a project, then `validate .`), and
/// the one that regressed `find_assets_root`'s first draft (a relative `.` has no resolvable
/// second `.parent()` at all, so the checks silently never ran). See
/// `asset_root_paths_checked_from_relative_dot_invocation_exits_1` below.
fn validate_relative_dot(fixture_name: &str) -> (i32, String) {
    let out = ironhold()
        .args(["validate", "."])
        .current_dir(fixture(fixture_name))
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
fn wrong_case_scene_path_exits_1() {
    // Real file on disk is scenes/main.scene.ron; the LoadScene action authors "Main" (capital M).
    // Path::exists() is case-insensitive on NTFS so this would otherwise validate clean while
    // 404ing over HTTP in the actual WASM/browser build. On a case-sensitive filesystem (Linux/
    // macOS/WSL/a per-directory case-sensitive NTFS dir) `exists()` itself fails for the wrong
    // case, so this falls through to the pre-existing "not found on disk" message instead — still
    // exit 1, just not proof of the new check specifically. Accept either.
    let (code, stdout) = validate("bad_scene_path_case");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        (stdout.contains("resolves on disk to") && stdout.contains("main.scene.ron"))
            || stdout.contains("not found on disk"),
        "expected the case-mismatch or missing-file message in output:\n{stdout}"
    );
}

#[test]
fn backslash_scene_path_exits_1() {
    // Path::join accepts `\` on Windows, so a backslash-separated authored path resolves locally
    // even though the WASM/browser build only understands `/`. On a case-sensitive/Unix-like
    // filesystem `\` is a literal filename character, so this falls through to the pre-existing
    // "not found on disk" message instead — still exit 1, just not proof of the new check
    // specifically. Accept either.
    let (code, stdout) = validate("bad_scene_path_backslash");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        (stdout.contains("backslash") && stdout.contains("forward slashes"))
            || stdout.contains("not found on disk"),
        "expected the backslash-separator or missing-file message in output:\n{stdout}"
    );
}

#[test]
fn matching_case_scene_path_exits_0() {
    let (code, stdout) = validate("valid_scene_path_case_matches");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

#[test]
fn scene_outside_convention_dir_is_parsed_and_cross_checked_exits_1() {
    // A scene path referenced by a LoadScene action but living outside scenes/ used to be
    // existence-checked only -- its contents (here, an entity referencing a nonexistent prefab)
    // were never parsed or cross-checked at all. do_validate now folds any such path into the
    // same scenes list a conventionally-placed scene participates in.
    let (code, stdout) = validate("scene_outside_convention_dir_validated");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("levels/custom.scene.ron")
            && stdout.contains("missing_prefab_outside_convention")
            && stdout.contains("not found in prefabs.ron"),
        "expected a missing_prefab error attributed to the out-of-convention scene:\n{stdout}"
    );
}

#[test]
fn initial_scene_outside_convention_dir_is_parsed_and_cross_checked_exits_1() {
    // Same gap, but via ProjectConfig.initial_scene rather than a LoadScene action.
    let (code, stdout) = validate("initial_scene_outside_convention_dir_validated");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("boot/main.scene.ron")
            && stdout.contains("missing_prefab_via_initial_scene")
            && stdout.contains("not found in prefabs.ron"),
        "expected a missing_prefab error attributed to the out-of-convention initial_scene:\n{stdout}"
    );
}

#[test]
fn valid_scene_outside_convention_dir_exits_0() {
    // Positive control: a well-formed scene living outside scenes/, reached via a LoadScene
    // action, should not trip anything just by being discovered and parsed -- carries a real
    // entity referencing a real prefab so the cross-check actually runs (rather than a fixture
    // with nothing for the parse-and-check step to exercise either way).
    let (code, stdout) = validate("valid_scene_outside_convention_dir");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

#[test]
fn broken_initial_scene_is_not_double_reported() {
    // Regression guard: a malformed scenes/main.scene.ron that's ALSO the project's
    // initial_scene must be reported once, not twice -- discovery must not re-discover (and
    // re-parse-fail) a path the scenes/ glob already attempted, whether or not that attempt
    // succeeded.
    let (code, stdout) = validate("broken_initial_scene_not_double_reported");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    let occurrences = stdout.matches("scenes/main.scene.ron").count();
    assert_eq!(
        occurrences, 1,
        "expected scenes/main.scene.ron to appear exactly once, got {occurrences}:\n{stdout}"
    );
}

#[test]
fn initial_scene_case_variant_is_not_double_parsed() {
    // Regression guard: initial_scene: "Scenes/main.scene.ron" (wrong case) referencing the same
    // real scenes/main.scene.ron the glob already parsed must not ALSO be parsed a second time
    // under the mis-cased spelling -- that would duplicate every one of that scene's cross-file
    // errors. The case mismatch itself is still reported (by the pre-existing path_case_mismatch
    // check), just not compounded by a duplicate parse.
    let (code, stdout) = validate("initial_scene_case_variant_not_double_parsed");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    let occurrences = stdout.matches("missing_prefab_case_variant").count();
    assert_eq!(
        occurrences, 1,
        "expected the missing-prefab error to appear exactly once, got {occurrences}:\n{stdout}"
    );
    assert!(
        stdout.contains("resolves on disk to"),
        "expected the pre-existing path_case_mismatch error to still fire:\n{stdout}"
    );
}

#[test]
fn load_scene_path_traversal_is_rejected() {
    // Regression guard: a LoadScene path containing a `..` segment must never be parsed as a
    // scene of THIS project -- doing so would read and cross-check a file outside project_dir
    // (here, a sibling fixture directory) and attribute its errors to this one.
    let (code, stdout) = validate("load_scene_path_traversal_rejected");
    assert_eq!(code, 0, "expected exit 0 (traversal must not be followed), got {code}:\n{stdout}");
    assert!(
        !stdout.contains("missing_prefab_in_a_different_project_entirely"),
        "expected the sibling fixture's content to never be read or cross-checked:\n{stdout}"
    );
}

#[test]
fn wrong_case_configured_catalog_path_exits_1() {
    // Real file on disk is data/subdir/my_prefabs.ron; prefab_catalog authors "Subdir" (capital
    // S) in .project.ron. Exercises the load_configured_catalog call site specifically, not the
    // action/scene-path sites the other three tests above cover. See the case-sensitive-filesystem
    // caveat on wrong_case_scene_path_exits_1 above — same fallback applies here.
    let (code, stdout) = validate("bad_prefab_catalog_path_case");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        (stdout.contains("resolves on disk to") && stdout.contains("subdir/my_prefabs.ron"))
            || stdout.contains("does not exist on disk"),
        "expected the case-mismatch or missing-path message in output:\n{stdout}"
    );
}

#[test]
fn wrong_case_configured_catalog_path_still_parses_and_checks_downstream() {
    // Regression guard: a case-mismatched (but locally readable) configured catalog path must
    // still be parsed, not just reported and dropped — otherwise every check that depends on the
    // catalog having loaded (here, the scene's typo'd prefab key) silently vanishes until the
    // designer fixes the casing, ambushing them with a fresh wave of unrelated errors afterward.
    let (_, stdout) = validate("bad_prefab_catalog_path_case");
    assert!(
        stdout.contains("typo_prefab_key_not_in_catalog")
            && stdout.contains("not found in prefabs.ron"),
        "expected the scene's missing_prefab error to still fire despite the catalog-path case \
         mismatch:\n{stdout}"
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
fn missing_prefab_catalog_path_exits_1() {
    let (code, stdout) = validate("bad_prefab_catalog_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("prefab_catalog") && stdout.contains("does not exist on disk"),
        "expected a diagnostic for the missing prefab_catalog target in output:\n{stdout}"
    );
}

#[test]
fn missing_asset_catalog_path_exits_1() {
    let (code, stdout) = validate("bad_asset_catalog_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("asset_catalog") && stdout.contains("does not exist on disk"),
        "expected a diagnostic for the missing asset_catalog target in output:\n{stdout}"
    );
}

#[test]
fn missing_stats_path_target_exits_1() {
    let (code, stdout) = validate("bad_stats_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("stats_path") && stdout.contains("does not exist on disk"),
        "expected a diagnostic for the missing stats_path target in output:\n{stdout}"
    );
}

#[test]
fn relocated_prefab_catalog_is_actually_checked_exits_1() {
    // Proves prefab_catalog's configured path is what gets read, not the "prefabs/prefabs.ron"
    // convention path (which doesn't exist at all in this fixture) -- if the relocated catalog
    // were silently skipped instead of loaded, this scene's bad prefab reference would go
    // undetected and the run would incorrectly exit 0.
    let (code, stdout) = validate("relocated_prefab_catalog");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("typo_prefab_key"),
        "expected the missing prefab key from the relocated catalog in output:\n{stdout}"
    );
}

#[test]
fn unset_prefab_catalog_with_convention_file_present_is_clean_without_strict_but_warns_with_strict() {
    // A real .project.ron exists but never sets prefab_catalog, while prefabs/prefabs.ron sits on
    // disk anyway -- validate's convention-path fallback checks it clean (matching every other
    // convention-path check in this file), but --strict should flag the divergence from the
    // runtime, which loads nothing at all for an unset field.
    let (code, _stdout) = validate("strict_unset_prefab_catalog");
    assert_eq!(code, 0, "expected exit 0 without --strict, got {code}");

    let (strict_code, strict_stdout) = validate_strict("strict_unset_prefab_catalog");
    assert_eq!(strict_code, 1, "expected exit 1 under --strict, got {strict_code}");
    assert!(
        strict_stdout.contains("prefab_catalog") && strict_stdout.contains("prefabs/prefabs.ron"),
        "expected the unset-catalog-path warning in output:\n{strict_stdout}"
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
fn missing_action_item_key_exits_1() {
    let (code, stdout) = validate("bad_action_item_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    for typo in [
        "typo_add_item",
        "typo_remove_item",
        "typo_transfer_item",
        "typo_buy_item",
    ] {
        assert!(
            stdout.contains(typo) && stdout.contains("not found in items.ron"),
            "expected missing item_key {typo:?} in output:\n{stdout}"
        );
    }
}

#[test]
fn missing_inventory_item_key_exits_1() {
    let (code, stdout) = validate("bad_inventory_item_key");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("typo_chest_item") && stdout.contains("not found in items.ron"),
        "expected the missing inventory item_key in output:\n{stdout}"
    );
}

#[test]
fn missing_item_currency_stat_exits_1() {
    let (code, stdout) = validate("bad_item_currency_stat");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("currency_stat \"silver\"") && stdout.contains("not found in stats.ron"),
        "expected the missing item currency_stat in output:\n{stdout}"
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

/// The prefab-catalog loop (behavior/camera_mode/foliage/stat-widget checks) iterates sorted
/// keys, not the `HashMap`'s arbitrary iteration order — otherwise error output would depend on
/// hash-seed-driven ordering instead of being stable across runs. 3 prefabs authored out of
/// alphabetical order, all with a missing `behavior` file, must always be reported in
/// alphabetical key order (`aaa_ghost`, then `mmm_ghost`, then `zzz_ghost`).
#[test]
fn prefab_catalog_errors_are_sorted_by_key_exits_1() {
    let (code, stdout) = validate("sorted_prefab_catalog_errors");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    let aaa = stdout.find("aaa_ghost").expect("expected 'aaa_ghost' in output");
    let mmm = stdout.find("mmm_ghost").expect("expected 'mmm_ghost' in output");
    let zzz = stdout.find("zzz_ghost").expect("expected 'zzz_ghost' in output");
    assert!(
        aaa < mmm && mmm < zzz,
        "expected errors in alphabetical key order (aaa, mmm, zzz), got positions {aaa}, {mmm}, {zzz}:\n{stdout}"
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

/// Regression test for the keep-first-on-collision fix (`gamepad_action_bar_slots.md` review,
/// system-architect + debug-detective): with 3+ bars sharing a key, every collision must cite the
/// SAME first-seen bar (`bar_p1`) as its partner, matching the runtime `warn!`'s
/// `seen.get()`-then-insert-only-when-absent behavior — not the immediately-preceding bar, which
/// is what an overwrite-on-insert implementation would report for the 3rd collision.
#[test]
fn cross_bar_duplicate_action_bar_key_three_way_keeps_first_exits_1() {
    let (code, stdout) = validate("cross_bar_duplicate_action_bar_key_three_way");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("ActionBar \"bar_p1\" slot \"KeyQ\" and ActionBar \"bar_p2\" slot \"KeyQ\""),
        "expected bar_p2's collision to cite first-seen bar_p1 in output:\n{stdout}"
    );
    assert!(
        stdout.contains("ActionBar \"bar_p1\" slot \"KeyQ\" and ActionBar \"bar_p3\" slot \"KeyQ\""),
        "expected bar_p3's collision to ALSO cite first-seen bar_p1 (not bar_p2) in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("ActionBar \"bar_p2\" slot \"KeyQ\" and ActionBar \"bar_p3\""),
        "bar_p3's collision must not cite bar_p2 -- that would be the old overwrite-on-insert bug:\n{stdout}"
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

#[test]
fn duplicate_gamepad_index_join_prefab_exits_1() {
    // A scene-placed player and a join_prefab_keys hot-join slot sharing a gamepad_index --
    // a hot-joined player's seed is read from its prefab exactly like a scene-placed player's,
    // so this collides too, even though no scene entity directly authors it.
    let (code, stdout) = validate("duplicate_gamepad_index_join_prefab");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("\"player_01\"")
            && stdout.contains("join_prefab_keys[1]")
            && stdout.contains("both use gamepad_index"),
        "expected the specific duplicate_gamepad_index collision between the scene entity and the \
         join_prefab_keys slot in output:\n{stdout}"
    );
}

/// Regression test for the keep-first-on-collision fix, `duplicate_gamepad_index`'s own site: 3
/// players sharing `gamepad_index: 0` must all be cited against the SAME first-seen player
/// (`player_01`), not the immediately-preceding one.
#[test]
fn duplicate_gamepad_index_three_way_keeps_first_exits_1() {
    let (code, stdout) = validate("duplicate_gamepad_index_three_way");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("\"player_01\" and \"player_02\""),
        "expected player_02's collision to cite first-seen player_01 in output:\n{stdout}"
    );
    assert!(
        stdout.contains("\"player_01\" and \"player_03\""),
        "expected player_03's collision to ALSO cite first-seen player_01 (not player_02) in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("\"player_02\" and \"player_03\""),
        "player_03's collision must not cite player_02 -- that would be the old overwrite-on-insert bug:\n{stdout}"
    );
}

/// Two player-tagged prefabs instantiated in the same scene both omit `player_index`, which
/// defaults to `0` — they'd show the identical "P1" HUD label/color (player_index is 0-based
/// internally, but the HUD label itself is 1-based, hence the `+1` in the message) and be
/// indistinguishable in local co-op. Same collision-detection shape as `duplicate_gamepad_index`,
/// but for `player_index` instead of `gamepad_index` — and `--strict`-only, unlike that sibling
/// check: nothing crashes, matching the runtime's own `warn!` severity for this case.
#[test]
fn duplicate_player_index_same_scene_without_strict_exits_0() {
    let (code, _) = validate("duplicate_player_index_same_scene");
    assert_eq!(code, 0, "duplicate player_index without --strict should exit 0");
}

#[test]
fn duplicate_player_index_same_scene_strict_exits_1() {
    let (code, stdout) = validate_strict("duplicate_player_index_same_scene");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("player_01") && stdout.contains("player_02")
            && stdout.contains("player_index") && stdout.contains("\"P1\""),
        "expected both colliding entity ids, a mention of player_index, and the 1-based HUD \
         label in output:\n{stdout}"
    );
}

/// Regression test (debug-detective finding): `player_index: u32::MAX` on both colliding players
/// must not crash the validator when formatting the 1-based HUD label (`index + 1` would panic
/// on overflow in a debug build and silently wrap to a wrong number in a release one) — the fix
/// uses `saturating_add(1)` instead.
#[test]
fn duplicate_player_index_u32_max_does_not_panic_strict_exits_1() {
    let (code, stdout) = validate_strict("duplicate_player_index_u32_max");
    assert_eq!(code, 1, "expected exit 1 (not a panic/exit 2), got {code}:\n{stdout}");
    assert!(
        stdout.contains("player_index: 4294967295"),
        "expected the offending player_index value in output:\n{stdout}"
    );
}

/// Same shape as above but each prefab explicitly sets a distinct `player_index` — must not be
/// flagged, proving the check is genuinely about the collision, not about `player_index` itself.
#[test]
fn distinct_player_index_same_scene_strict_exits_0() {
    let (code, stdout) = validate_strict("distinct_player_index_same_scene");
    assert_eq!(code, 0, "expected exit 0 (distinct indices, no collision), got {code}:\n{stdout}");
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

// ── coyote_time_secs upper bound (soft, --strict only) ─────────────────────────
//
// Unlike the negative case above, this flags a coyote_time_secs value disproportionate to the
// prefab's own resolved jump airtime (`2 * jump_velocity / GRAVITY`) -- large enough to mask the
// entire jump/fall. See `planning/claude_suggestions.md`'s entry on this, verified empirically
// during `uphill_jump_lock.md`'s sensor-veto fix review.
#[test]
fn coyote_time_exceeds_jump_airtime_without_strict_exits_0() {
    let (code, _) = validate("bad_coyote_time_upper_bound");
    assert_eq!(code, 0, "coyote_time_secs exceeding jump airtime without --strict should exit 0");
}

#[test]
fn coyote_time_exceeds_jump_airtime_strict_exits_1() {
    let (code, stdout) = validate_strict("bad_coyote_time_upper_bound");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("coyote_upper_player") && stdout.contains("actually reports \"ungrounded\" during a `jump`"),
        "expected the offending prefab key and the airtime message in output:\n{stdout}"
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

#[test]
fn camera_mode_nested_split_in_prefab_exits_1() {
    // `split:` authored INSIDE `camera_mode: Orbit(...)` (instead of as a sibling of camera_mode
    // under `components:`) parses fine but is never read — only a runtime warn! before this check.
    let (code, stdout) = validate("camera_mode_nested_split_in_prefab");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("never read and have no effect") && stdout.contains("bad_split_player"),
        "expected the nested split/party message in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_nested_party_in_registry_exits_1() {
    // Same authoring mistake as the prefab case above, but inside a scene's `camera_modes:`
    // registry entry -- proves the shared helper fires from both call sites, and that `party:`
    // (not just `split:`) is caught.
    let (code, stdout) = validate("camera_mode_nested_party_in_registry");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("never read and have no effect") && stdout.contains("\"aerial\""),
        "expected the nested split/party message in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_fixed_both_look_at_in_prefab_strict_exits_1() {
    // A Fixed camera with both look_at/look_at_entity set is working (look_at_entity wins when it
    // resolves, look_at otherwise), not broken -- --strict advisory, not a hard error.
    let (code, stdout) = validate("camera_mode_fixed_both_look_at_in_prefab");
    assert_eq!(code, 0, "expected exit 0 without --strict, got {code}:\n{stdout}");
    let (code, stdout) = validate_strict("camera_mode_fixed_both_look_at_in_prefab");
    assert_eq!(code, 1, "expected exit 1 with --strict, got {code}");
    assert!(
        stdout.contains("has both `look_at` and `look_at_entity` set"),
        "expected the ambiguous-look-at message in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_fixed_neither_look_at_in_prefab_strict_exits_1() {
    let (code, stdout) = validate("camera_mode_fixed_neither_look_at_in_prefab");
    assert_eq!(code, 0, "expected exit 0 without --strict, got {code}:\n{stdout}");
    let (code, stdout) = validate_strict("camera_mode_fixed_neither_look_at_in_prefab");
    assert_eq!(code, 1, "expected exit 1 with --strict, got {code}");
    assert!(
        stdout.contains("has neither `look_at` nor `look_at_entity` set"),
        "expected the missing-look-at message in output:\n{stdout}"
    );
}

#[test]
fn camera_mode_valid_prefab_camera_mode_exits_0() {
    // Positive control: split as a proper sibling of camera_mode, and a Fixed camera with exactly
    // one of look_at/look_at_entity set -- neither should trip the new prefab-level checks.
    let (code, stdout) = validate("camera_mode_valid_prefab_camera_mode");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    // --strict on this fixture is expected to warn about unrelated pre-existing checks (both
    // prefabs are unplaced, and the tagging needed to exercise the player-only gate above also
    // pulls in the unrelated jump-apex-vs-ground-sensor check) -- only assert the two new
    // camera_mode_fixed_* checks specifically stay silent, not that --strict is fully clean.
    let (_, stdout) = validate_strict("camera_mode_valid_prefab_camera_mode");
    assert!(
        !stdout.contains("has both `look_at`") && !stdout.contains("has neither `look_at`"),
        "expected no Fixed look_at --strict warning on a well-formed camera_mode:\n{stdout}"
    );
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

// ── UI trigger reachability ───────────────────────────────────────────────────

#[test]
fn bad_ui_trigger_button_exits_1() {
    let (code, stdout) = validate("bad_ui_trigger_button");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("mute_button") && stdout.contains("ui.button_pressed:toggle_mute"),
        "expected the mismatched Button's id and derived event in output:\n{stdout}"
    );
    assert!(
        stdout.contains("settings_icon") && stdout.contains("ui.button_pressed:open_settings"),
        "expected the mismatched IconButton's id and derived event in output:\n{stdout}"
    );
}

#[test]
fn bad_ui_trigger_key_binding_exits_1() {
    let (code, stdout) = validate("bad_ui_trigger_key_binding");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("global_key_bindings") && stdout.contains("ui.button_pressed:menu_toggle"),
        "expected the mismatched global_key_bindings entry and derived event in output:\n{stdout}"
    );
    assert!(
        stdout.contains("scene_key_bindings") && stdout.contains("ui.button_pressed:pause_toggle"),
        "expected the mismatched scene_key_bindings entry and derived event in output:\n{stdout}"
    );
    assert!(
        stdout.contains("global_unclaimed_gamepad_bindings")
            && stdout.contains("ui.button_pressed:join"),
        "expected the mismatched global_unclaimed_gamepad_bindings entry and derived event in output:\n{stdout}"
    );
    assert!(
        stdout.contains("scene_unclaimed_gamepad_bindings")
            && stdout.contains("ui.button_pressed:scene_join"),
        "expected the mismatched scene_unclaimed_gamepad_bindings entry and derived event in output:\n{stdout}"
    );
    // A key binding or gamepad binding is never "clicked" — the message must not claim it is.
    assert!(
        !stdout.contains("on click"),
        "key/gamepad binding mismatches must not use button-click phrasing:\n{stdout}"
    );
}

/// A malformed `logic/rules.ron` must not cascade into a wave of `unreachable_trigger` errors on
/// top of the real parse error — the UI trigger reachability check is skipped entirely whenever
/// any logic file failed to parse, since "nothing parsed" cannot be distinguished from "nothing
/// is handled" without fabricating noise.
#[test]
fn bad_rules_parse_does_not_cascade_into_unreachable_trigger_exits_1() {
    let (code, stdout) = validate("bad_rules_parse_no_cascade");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("Expected comma"),
        "expected the rules.ron parse error reported in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("wired_button") && !stdout.contains("but no rule/transition/binding"),
        "a rules.ron parse error must not also produce unreachable_trigger noise for its \
         (would-be-correctly-wired) button:\n{stdout}"
    );
    assert!(
        stdout.contains("Cross-file checks") && stdout.contains(" OK"),
        "expected cross-file checks to report clean (check skipped, not fabricated) in output:\n{stdout}"
    );
}

/// Every trigger source (`Button`, `IconButton`, `global_key_bindings`, `scene_key_bindings`,
/// `global_unclaimed_gamepad_bindings`, `scene_unclaimed_gamepad_bindings`) and every
/// event-handling source (`rules.ron`, a `state_machine.ron` in-state `on:`, its `transitions`,
/// its `global_on:`, and a `behaviors/*.behavior.ron` file) wired correctly — must not
/// false-positive.
#[test]
fn valid_ui_trigger_exits_0() {
    let (code, stdout) = validate("valid_ui_trigger");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// The reverse direction of `valid_ui_trigger_exits_0`: this fixture's every rule/transition/
/// binding is matched to exactly one button/binding, so `--strict`'s new `orphan_rule` check must
/// not false-positive on any of them either. Doubles as a drift guard for
/// `collect_reachable_ui_triggers` (the orphan check's data source, deliberately a separate
/// function from `check_ui_trigger_reachability` rather than a refactor of it) — the fixture
/// exercises all six trigger-source shapes (global/scene key bindings, global/scene unclaimed
/// gamepad bindings, `Button`, `IconButton`), so losing coverage of any one of them here would
/// fail this test, not just silently diverge from the forward check.
#[test]
fn valid_ui_trigger_strict_exits_0() {
    let (code, stdout) = validate_strict("valid_ui_trigger");
    assert_eq!(code, 0, "expected exit 0 under --strict, got {code}:\n{stdout}");
}

// ── Configurable rules_path/state_machine_path (planning/backlog.md) ───────────

#[test]
fn state_machine_only_project_ignores_dead_rules_ron() {
    // Real bug this closes (found live against 3rd_person_game_demo/terrain_demo): a project
    // setting only state_machine_path must NOT have its unrelated, runtime-dead logic/rules.ron
    // silently counted as live. A Spawn action in that dead file references a nonexistent prefab
    // -- if it were wrongly discovered, this would be a hard missing_prefab error.
    let (code, stdout) = validate("state_machine_only_ignores_dead_rules_ron");
    assert_eq!(code, 0, "expected exit 0 (dead rules.ron must be ignored), got {code}:\n{stdout}");
    assert!(
        !stdout.contains("missing_prefab_in_a_dead_rules_file"),
        "expected the dead rules.ron file to never be parsed or cross-checked:\n{stdout}"
    );
}

#[test]
fn rules_path_custom_filename_is_discovered_exits_1() {
    // The other direction of the same bug: a custom rules_path filename (not the
    // "logic/rules.ron" convention) must still be discovered and cross-checked.
    let (code, stdout) = validate("rules_path_custom_filename_is_discovered");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_prefab_in_custom_named_rules_file"),
        "expected the custom-named rules file's own missing_prefab error in output:\n{stdout}"
    );
}

#[test]
fn inline_rules_are_discovered_exits_1() {
    // Inline V1 ProjectConfig.rules (no rules_path, no logic/ dir at all) must be cross-checked
    // exactly like an external rules.ron would be.
    let (code, stdout) = validate("inline_rules_are_discovered");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("missing_prefab_in_inline_rules"),
        "expected the inline rule's own missing_prefab error in output:\n{stdout}"
    );
}

#[test]
fn missing_configured_rules_path_and_state_machine_path_exits_1() {
    // A configured-but-missing rules_path/state_machine_path is a hard error, matching the
    // existing load_configured_catalog precedent for the four catalog paths.
    let (code, stdout) = validate("bad_rules_path_and_state_machine_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("logic/does_not_exist.ron")
            && stdout.contains("rules_path in .project.ron does not exist on disk"),
        "expected the missing rules_path error in output:\n{stdout}"
    );
    assert!(
        stdout.contains("logic/also_missing.ron")
            && stdout.contains("state_machine_path in .project.ron does not exist on disk"),
        "expected the missing state_machine_path error in output:\n{stdout}"
    );
}

#[test]
fn rules_path_case_mismatch_exits_1() {
    // rules_path is a designer-authored path like any other -- it must get the same
    // path_case_mismatch coverage every other configurable path in this file has.
    let (code, stdout) = validate("rules_path_case_mismatch");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        (stdout.contains("resolves on disk to") && stdout.contains("logic/rules.ron"))
            || stdout.contains("does not exist on disk"),
        "expected the case-mismatch or missing-path message in output:\n{stdout}"
    );
}

#[test]
fn rules_path_pointed_at_wrong_file_type_does_not_corrupt_that_files_own_report() {
    // Regression guard: a rules_path typo'd onto an existing, valid, different-type file (here,
    // prefabs/prefabs.ron, itself also configured as prefab_catalog) must not be re-parsed as a
    // LogicRulesAsset and reported as broken -- that file already has its own, correct FileResult
    // from loading it as a catalog. The scene's own unrelated missing_prefab error must still
    // fire, proving the catalog itself loaded fine despite the rules_path collision attempt.
    let (code, stdout) = validate("rules_path_pointed_at_wrong_file_type");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert_eq!(
        stdout.matches("prefabs/prefabs.ron").count(),
        1,
        "expected prefabs/prefabs.ron to appear exactly once, not re-parsed under a second type:\n{stdout}"
    );
    assert!(
        stdout.contains("typo_prefab_key_should_still_be_caught"),
        "expected the scene's own missing_prefab error to still fire:\n{stdout}"
    );
}

#[test]
fn unset_rules_path_with_convention_file_strict_exits_1() {
    // A project setting only state_machine_path, with a logic/rules.ron still sitting on disk,
    // must get a --strict signal that the file is dead and unchecked -- otherwise resolve_logic_
    // files correctly ignoring it (see the dead-rules-ron test above) leaves the file with zero
    // diagnostic coverage from any tool at all.
    let (code, stdout) = validate("unset_rules_path_with_convention_file");
    assert_eq!(code, 0, "expected exit 0 without --strict, got {code}:\n{stdout}");
    let (code, stdout) = validate_strict("unset_rules_path_with_convention_file");
    assert_eq!(code, 1, "expected exit 1 with --strict, got {code}");
    assert!(
        stdout.contains("logic/rules.ron exists but rules_path is not set"),
        "expected the unset_logic_path_with_convention_file warning in output:\n{stdout}"
    );
}

#[test]
fn orphan_ui_rule_strict_exits_1() {
    // A rule handling ui.button_pressed:truly_orphaned with no button/binding anywhere in the
    // project -- dead code, only reported under --strict (same severity class as unused_prefab).
    let (code, stdout) = validate("orphan_ui_rule");
    assert_eq!(code, 0, "expected exit 0 without --strict, got {code}:\n{stdout}");

    let (strict_code, strict_stdout) = validate_strict("orphan_ui_rule");
    assert_eq!(strict_code, 1, "expected exit 1 under --strict, got {strict_code}");
    assert!(
        strict_stdout.contains("ui.button_pressed:truly_orphaned")
            && strict_stdout.contains("no button/key/gamepad binding"),
        "expected the orphan_rule warning for the unreachable rule in output:\n{strict_stdout}"
    );
}

/// The five engine-hardcoded panel triggers (`close_inventory`/`close_shop`/`close_container`/
/// `take_all_from_container`/`buy_item:{item_key}`) are never authored as a scene `Button.action`
/// string -- they're emitted internally by `InventoryPanel`/`ShopPanel`/`ContainerPanel` widgets.
/// A rule correctly handling one of these must NOT be flagged as orphaned just because no scene
/// button authors that exact string.
#[test]
fn valid_panel_triggers_no_orphan_strict_exits_0() {
    let (code, stdout) = validate_strict("valid_panel_triggers_no_orphan");
    assert_eq!(code, 0, "expected exit 0 under --strict, got {code}:\n{stdout}");
}

/// The same fixture also proves the *forward* direction: a scene with `InventoryPanel`/
/// `ShopPanel`/`ContainerPanel`, each correctly wired to a matching rule, must not false-positive
/// as `unreachable_trigger` either.
#[test]
fn valid_panel_triggers_no_orphan_exits_0() {
    let (code, stdout) = validate("valid_panel_triggers_no_orphan");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// A scene with `InventoryPanel`/`ShopPanel`/`ContainerPanel` but no matching rule for any of
/// their five built-in triggers -- these panels' own close/buy buttons will silently do nothing.
/// Worse than the authored-button case: there's no `action:` string in the designer's own RON to
/// spot the mistake from, only the panel's mere presence.
#[test]
fn bad_panel_trigger_unreachable_exits_1() {
    let (code, stdout) = validate("bad_panel_trigger_unreachable");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("InventoryPanel") && stdout.contains("ui.button_pressed:close_inventory"),
        "expected the unreachable InventoryPanel close trigger in output:\n{stdout}"
    );
    assert!(
        stdout.contains("ShopPanel") && stdout.contains("ui.button_pressed:close_shop"),
        "expected the unreachable ShopPanel close trigger in output:\n{stdout}"
    );
    assert!(
        stdout.contains("buy_item:sword") && stdout.contains("ui.button_pressed:buy_item:sword"),
        "expected the unreachable ShopPanel buy trigger (derived from the merchant's stock) in \
         output:\n{stdout}"
    );
    assert!(
        stdout.contains("ContainerPanel") && stdout.contains("ui.button_pressed:close_container"),
        "expected the unreachable ContainerPanel close trigger in output:\n{stdout}"
    );
    assert!(
        stdout.contains("ui.button_pressed:take_all_from_container"),
        "expected the unreachable ContainerPanel take-all trigger in output:\n{stdout}"
    );
}

/// A malformed scene must not cascade into a bogus `orphan_rule` warning for the rule that would
/// otherwise be reachable via that scene's own (now-unparseable) button -- the orphan check is
/// skipped entirely whenever any scene failed to parse, mirroring `check_ui_trigger_reachability`'s
/// existing logic-file parse-failure protection but for the opposite data set.
#[test]
fn bad_scene_parse_does_not_cascade_into_orphan_rule_strict_exits_1() {
    let (code, stdout) = validate_strict("bad_scene_parse_no_orphan_cascade");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("Expected comma"),
        "expected the scene parse error reported in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("Strict checks") && !stdout.contains("wired_trigger"),
        "a scene parse error must not also produce a bogus orphan_rule report for the \
         (would-be-correctly-wired) rule:\n{stdout}"
    );
}

// ── Non-ASCII character in `text:` (soft, --strict only) ────────────────────────
//
// The embedded UI font has no glyph for ANY non-ASCII character (its cmap covers only
// U+0020..U+007E, per debug-detective's `cli_validate_small_wins` review) -- it renders as a
// tofu box instead. `--strict`-only: the rest of the text still displays correctly, so this is
// an authoring-hygiene lint, not a load-time regression.
#[test]
fn non_ascii_dash_in_label_without_strict_exits_0() {
    let (code, _) = validate("non_ascii_dash_in_label");
    assert_eq!(code, 0, "em-dash in a Label's text without --strict should exit 0");
}

#[test]
fn non_ascii_dash_in_label_strict_exits_1() {
    let (code, stdout) = validate_strict("non_ascii_dash_in_label");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("tofu_label") && stdout.contains("em dash") && stdout.contains("U+2014"),
        "expected the offending label id and the em-dash message in output:\n{stdout}"
    );
}

/// Regression test (debug-detective finding): the check must not be a dash-only allowlist -- the
/// embedded font has zero non-ASCII glyph coverage at all, so an unrelated non-ASCII character
/// like `×` (multiplication sign, no friendly name in `NAMED_NON_ASCII_CHARS`) must also be
/// flagged, with the generic "non-ASCII character" fallback name.
#[test]
fn non_ascii_non_dash_char_in_label_strict_exits_1() {
    let (code, stdout) = validate_strict("non_ascii_non_dash_char_in_label");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("multiply_label") && stdout.contains("non-ASCII character") && stdout.contains("U+00D7"),
        "expected the offending label id and the generic-fallback message in output:\n{stdout}"
    );
}

/// Regression test (debug-detective finding): the check must also cover dialogue prose
/// (`speaker`/`body`/choice `label`), not just short UI labels -- narrative text is the highest-
/// risk surface for a pasted-in em-dash or curly quote.
#[test]
fn non_ascii_char_in_dialogue_body_strict_exits_1() {
    let (code, stdout) = validate_strict("non_ascii_char_in_dialogue");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("DialogueNode.body") && stdout.contains("em dash") && stdout.contains("U+2014"),
        "expected the dialogue node kind and the em-dash message in output:\n{stdout}"
    );
}

/// An ASCII hyphen must not be flagged — proving the check is genuinely about non-ASCII dash
/// characters, not hyphens/dashes in general.
#[test]
fn ascii_hyphen_in_label_strict_exits_0() {
    let (code, stdout) = validate_strict("ascii_hyphen_in_label");
    assert_eq!(code, 0, "expected exit 0 (ASCII hyphen, no false positive), got {code}:\n{stdout}");
}

// ── AssetCatalog / material / terrain path existence + case checks ─────────────
//
// Previously, every `AssetCatalog` entry (model/texture/audio/decal), `MaterialDef`'s own nested
// texture/shader/splatmap paths, `ProjectConfig.global_environment`'s IBL paths, and
// `GameSceneV2.terrain`'s heightmap/splatmap/material_paths were only ever key-existence checked
// (does an action's `key:` resolve to a catalog entry) — never checked for whether the entry's
// own underlying file path actually exists on disk, let alone with correct case. These fixtures
// deliberately live at `tests/fixtures/assets/projects/{case}` (not a bare
// `tests/fixtures/{case}`) since these checks are resolved against a shared "assets root" —
// the nearest ancestor of `project_dir` literally named `assets` (`find_assets_root`) —
// `tests/fixtures/assets/` plays that role here, exactly matching the real
// `assets/projects/{name}/` + `assets/shared/` convention every shipped project uses.
#[test]
fn valid_asset_paths_exits_0() {
    let (code, stdout) = validate("assets/projects/valid_asset_paths");
    assert_eq!(code, 0, "expected exit 0 (every path resolves), got {code}:\n{stdout}");
}

#[test]
fn missing_asset_catalog_model_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_model_missing");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("model \"hero\"") && stdout.contains("not found on disk"),
        "expected the offending model key and missing-file message in output:\n{stdout}"
    );
}

/// Regression test (system-architect/debug-detective finding): `find_assets_root`'s first draft
/// used `project_dir.parent().parent()` on the raw, uncanonicalized CLI argument -- so the single
/// most common real invocation shape, `cd`-ing into a project and running `ironhold validate .`,
/// had no resolvable second parent at all and silently skipped every asset-path check (a
/// genuinely broken texture path validated clean). Reproduces that exact invocation shape against
/// the same broken fixture the plain (absolute-path) test above uses.
#[test]
fn asset_root_paths_checked_from_relative_dot_invocation_exits_1() {
    let (code, stdout) = validate_relative_dot("assets/projects/bad_model_missing");
    assert_eq!(
        code, 1,
        "expected exit 1 even when invoked as `validate .` from inside the project, got {code}:\n{stdout}"
    );
    assert!(
        stdout.contains("model \"hero\"") && stdout.contains("not found on disk"),
        "expected the offending model key and missing-file message in output:\n{stdout}"
    );
}

/// Also proves the GLB `#Scene0` sub-asset fragment doesn't accidentally mask a real
/// missing-file error — the fixture's `path:` carries one, and the base file genuinely doesn't
/// exist.
#[test]
fn missing_asset_catalog_model_path_reports_full_authored_path_with_fragment() {
    let (code, stdout) = validate("assets/projects/bad_model_missing");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("nonexistent_hero.glb#Scene0"),
        "expected the full authored path, fragment included, in output:\n{stdout}"
    );
}

#[test]
fn wrong_case_asset_catalog_model_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_model_case");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("model \"hero\"") && stdout.contains("resolves on disk to"),
        "expected the offending model key and case-mismatch message in output:\n{stdout}"
    );
}

#[test]
fn missing_asset_catalog_texture_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_texture_missing");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("texture \"hero_diffuse\"") && stdout.contains("not found on disk"),
        "expected the offending texture key and missing-file message in output:\n{stdout}"
    );
}

#[test]
fn missing_asset_catalog_audio_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_audio_missing");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("audio \"click\"") && stdout.contains("not found on disk"),
        "expected the offending audio key and missing-file message in output:\n{stdout}"
    );
}

#[test]
fn missing_asset_catalog_decal_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_decal_missing");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("decal \"ring\"") && stdout.contains("not found on disk"),
        "expected the offending decal key and missing-file message in output:\n{stdout}"
    );
}

/// `MaterialDef`'s `MaterialKind::Standard` texture fields are raw `asset_server.load()` paths
/// (`material_factory.rs`), not `AssetCatalog.textures` keys — unlike e.g.
/// `FoliageMaterialDef.leaf_texture`, which genuinely is a catalog key and is correctly left
/// unchecked by this feature.
#[test]
fn missing_standard_material_texture_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_material_standard");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("material \"mat_standard\".base_color_texture") && stdout.contains("not found on disk"),
        "expected the offending material key/field and missing-file message in output:\n{stdout}"
    );
}

#[test]
fn missing_terrain_material_splatmap_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_material_terrain");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("material \"mat_terrain\".splatmap") && stdout.contains("not found on disk"),
        "expected the offending material key/field and missing-file message in output:\n{stdout}"
    );
}

#[test]
fn missing_custom_material_shader_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_material_custom");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("material \"mat_custom\".shader") && stdout.contains("not found on disk"),
        "expected the offending material key/field and missing-file message in output:\n{stdout}"
    );
}

/// `ProjectConfig.global_environment`'s IBL paths — project-scoped, not catalog-scoped, but the
/// identical raw-path shape.
#[test]
fn missing_global_environment_diffuse_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_global_environment");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("global_environment.diffuse_path") && stdout.contains("not found on disk"),
        "expected the offending field and missing-file message in output:\n{stdout}"
    );
}

/// `GameSceneV2.terrain`'s heightmap/splatmap/material_paths — scene-scoped, same raw-path shape
/// again.
#[test]
fn missing_scene_terrain_heightmap_path_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_scene_terrain");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("terrain.heightmap") && stdout.contains("not found on disk"),
        "expected the offending field and missing-file message in output:\n{stdout}"
    );
}

/// A bare `tests/fixtures/{name}/` fixture (no `assets/projects/{name}/` + `assets/shared/`
/// nesting) must not trip any of the new checks at all — `find_assets_root` correctly returns
/// `None` rather than guessing wrong and resolving paths against some unrelated directory.
/// Reuses an existing fixture with a real `assets.ron` model entry.
#[test]
fn asset_catalog_path_checks_are_skipped_for_bare_fixtures_exits_0() {
    let (code, stdout) = validate("valid_project");
    assert_eq!(code, 0, "expected exit 0 (checks skipped, not fabricated), got {code}:\n{stdout}");
}

/// Regression test (debug-detective finding): `find_assets_root` matching on directory NAME
/// alone ("nearest ancestor literally named `assets`") fabricates a false assets root -- and
/// therefore false `missing_file` errors -- for a project living anywhere under a directory that
/// merely happens to be named `assets` for an unrelated reason. This fixture's `project_dir` is
/// `.../assets_name_without_corroboration/assets/decoy_project` -- its `assets`-named ancestor is
/// real, but has no `projects/`/`shared/` child, so `find_assets_root` must still return `None`
/// (checks skipped) despite the name matching, rather than fabricating an error against the
/// unrelated `shared/audio/click.wav` reference this fixture deliberately can't resolve.
#[test]
fn assets_named_ancestor_without_corroboration_is_not_treated_as_assets_root_exits_0() {
    let (code, stdout) = validate("assets_name_without_corroboration/assets/decoy_project");
    assert_eq!(
        code, 0,
        "an `assets`-named ancestor with no projects/shared child must not be treated as the \
         assets root, got {code}:\n{stdout}"
    );
}

/// Regression test (debug-detective finding): an absolute authored path (e.g. pasted from a
/// file-browser "copy path" on the author's own machine) must be rejected explicitly, not
/// silently joined -- `Path::join` with an absolute RHS discards the LHS entirely, so
/// `assets_root.join(absolute_path)` would resolve to whatever file genuinely exists at that
/// absolute path on the machine running `validate`, producing a perfect false negative: valid
/// only there, never over the actual asset-relative path a real build serves from.
#[test]
fn absolute_asset_catalog_path_is_rejected_exits_1() {
    let (code, stdout) = validate("assets/projects/bad_absolute_path");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("model \"hero\"") && stdout.contains("is an absolute path"),
        "expected the offending model key and absolute-path message in output:\n{stdout}"
    );
}

// ── Batch: 6 more silent-failure gap closures (cli_validate_gap_closures) ─────

/// `PrefabDef.animation_policy` is resolved the same way as `behavior`/`dialogue`
/// (project-relative, `entity_spawner.rs`'s `resolve_project_path`) but was previously checked by
/// nothing at all -- a 404 here leaves the entity permanently `Visibility::Hidden`.
#[test]
fn missing_animation_policy_path_exits_1() {
    let (code, stdout) = validate("bad_animation_policy_missing");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("animation_policy") && stdout.contains("not found on disk"),
        "expected the missing animation_policy path in output:\n{stdout}"
    );
}

/// A `jump_to` naming no node in its own dialogue (and not the reserved `"__end__"`) only ever
/// surfaced at runtime as a `warn!` that silently closes the dialogue mid-conversation.
#[test]
fn dialogue_jump_to_unresolved_target_exits_1() {
    let (code, stdout) = validate("bad_dialogue_jump_to_target");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("does_not_exist") && stdout.contains("jump_to"),
        "expected the unresolved jump_to target in output:\n{stdout}"
    );
}

/// A duplicate `DialogueNode.id` within one file made the second node permanently unreachable by
/// jump (`dialogue.rs`'s `nodes.iter().position(...)` only ever matches the first), with zero
/// diagnostic anywhere before this check existed.
#[test]
fn dialogue_duplicate_node_id_exits_1() {
    let (code, stdout) = validate("bad_dialogue_duplicate_node_id");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("duplicate DialogueNode id") && stdout.contains("\"start\""),
        "expected the duplicate node id in output:\n{stdout}"
    );
}

/// `jump_to: "__end__"` (close the dialogue) and a `jump_to` naming a real sibling node must both
/// be accepted without a false positive -- this is the positive control for the check above.
#[test]
fn valid_dialogue_jump_to_end_and_sibling_exits_0() {
    let (code, stdout) = validate("valid_dialogue_jump_to_end");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// `DialogueCondition::StatAtLeast { stat_key }` is the same unchecked global-stats reference
/// shape as the merchant `currency_stat`/`ApplyModifier`/`RemoveModifier` checks -- a typo
/// silently hides the choice forever with no runtime message at all.
#[test]
fn dialogue_stat_at_least_unresolved_stat_key_exits_1() {
    let (code, stdout) = validate("bad_dialogue_stat_at_least");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("strenght") && stdout.contains("not found in stats.ron"),
        "expected the misspelled stat_key in output:\n{stdout}"
    );
}

/// `Action::JoinPlayer` derives `player_{slot + 1}_start` from the scene's own `join_prefab_keys`
/// slot index and looks it up in the SAME scene's `spawn_points`, with no runtime `warn!` at all
/// on a miss -- it silently falls back to spawning next to the primary player instead.
#[test]
fn join_player_missing_spawn_point_exits_1() {
    let (code, stdout) = validate("bad_join_player_spawn_point");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("player_2_start") && stdout.contains("join_prefab_keys[1]"),
        "expected the missing player_2_start spawn point in output:\n{stdout}"
    );
}

/// Positive control for the check above -- a `join_prefab_keys` slot with a matching
/// `player_{N}_start` spawn point must not false-positive.
#[test]
fn valid_join_player_spawn_point_exits_0() {
    let (code, stdout) = validate("valid_join_player_spawn_point");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// Debug-detective finding: a `join_prefab_keys` slot at or beyond `MAX_SPLIT_PLAYERS` can never
/// actually be hot-joined into (the executor bails on `next_slot >= MAX_SPLIT_PLAYERS` before
/// ever reading the slot), so it must be reported as its own `unreachable_join_slot` mistake --
/// NOT as a missing `player_5_start` spawn point, which would be a false positive telling a
/// designer to add a spawn point that would still do nothing.
#[test]
fn join_prefab_keys_slot_beyond_max_split_players_exits_1() {
    let (code, stdout) = validate("bad_join_prefab_keys_beyond_max_split_players");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("join_prefab_keys[4]") && stdout.contains("beyond MAX_SPLIT_PLAYERS"),
        "expected the unreachable-slot message in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("player_5_start"),
        "must not also demand a spawn point for a slot that can never be reached:\n{stdout}"
    );
}

/// Debug-detective finding: a `join_prefab_keys` entry whose prefab key doesn't exist at all
/// can never be hot-joined into either (the executor `continue`s before ever deriving a spawn
/// point), so the pre-existing `missing_reference` error must be the ONLY error -- not paired
/// with a second, false `no spawn_points entry` complaint about a slot nobody could reach anyway.
#[test]
fn join_prefab_keys_missing_prefab_does_not_also_demand_spawn_point_exits_1() {
    let (code, stdout) = validate("bad_join_prefab_keys_missing_prefab");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("typo_prefab") && stdout.contains("not found in prefabs.ron"),
        "expected the missing-prefab error in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("spawn_points entry"),
        "must not also demand a spawn point for a slot whose prefab doesn't even exist:\n{stdout}"
    );
}

/// `ItemDef.icon_sheet` is an `AssetCatalog.textures` key sitting in the same loop the
/// pre-existing `currency_stat` check already uses, but was never itself cross-checked.
#[test]
fn item_icon_sheet_unresolved_exits_1() {
    let (code, stdout) = validate("bad_item_icon_sheet");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("potion_icons") && stdout.contains("not found in assets.ron's textures"),
        "expected the missing icon_sheet texture key in output:\n{stdout}"
    );
}

/// `AssetCatalog::validate()` was never called by `ironhold_cli` at all -- its own schema-level
/// invariants (e.g. rejecting an empty model path) were runtime-only. This fixture has no
/// `assets`-named ancestor with a `projects`/`shared` child, so `check_asset_root_paths` itself
/// is inert here (deliberately isolating this test to the newly-wired `.validate()` call alone).
#[test]
fn asset_catalog_validate_invariant_exits_1() {
    let (code, stdout) = validate("bad_asset_catalog_invariant");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("broken") && stdout.contains("empty path"),
        "expected the AssetCatalog::validate() invariant message in output:\n{stdout}"
    );
}

/// Same as above for `PrefabCatalog::validate()` -- a `kind: Foliage` prefab missing its
/// `foliage` block is a real schema invariant that was runtime-only before this batch.
#[test]
fn prefab_catalog_validate_invariant_exits_1() {
    let (code, stdout) = validate("bad_prefab_catalog_invariant");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("broken_bush") && stdout.contains("no `foliage` block"),
        "expected the PrefabCatalog::validate() invariant message in output:\n{stdout}"
    );
}

/// System-architect finding: `AssetCatalog`/`PrefabCatalog` were only 2 of the 4 catalog types
/// `project_loader.rs` calls `.validate()` on at runtime -- `StatCatalog` and `ItemCatalog` have
/// real invariants too (`min > max`, `max_stack: 0`) and were left runtime-only. Completing the
/// set here.
#[test]
fn stat_catalog_validate_invariant_exits_1() {
    let (code, stdout) = validate("bad_stat_catalog_invariant");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("broken") && stdout.contains("min (10) > max (5)"),
        "expected the StatCatalog::validate() invariant message in output:\n{stdout}"
    );
}

#[test]
fn item_catalog_validate_invariant_exits_1() {
    let (code, stdout) = validate("bad_item_catalog_invariant");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("broken") && stdout.contains("max_stack must be at least 1"),
        "expected the ItemCatalog::validate() invariant message in output:\n{stdout}"
    );
}

// ── Batch: icon_sheet texture-key family + camera-mode/flycam checks (cli_validate_batch3) ────

#[test]
fn inventory_panel_icon_sheet_unresolved_exits_1() {
    let (code, stdout) = validate("bad_ui_texture_keys");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("InventoryPanel") && stdout.contains("missing_inventory_icons"),
        "expected the missing InventoryPanel icon_sheet in output:\n{stdout}"
    );
}

#[test]
fn container_panel_icon_sheet_unresolved_exits_1() {
    let (code, stdout) = validate("bad_ui_texture_keys");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("ContainerPanel") && stdout.contains("missing_container_icons"),
        "expected the missing ContainerPanel icon_sheet in output:\n{stdout}"
    );
}

#[test]
fn action_bar_icon_sheet_unresolved_exits_1() {
    let (code, stdout) = validate("bad_ui_texture_keys");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("ActionBar") && stdout.contains("missing_bar_icons"),
        "expected the missing ActionBar icon_sheet in output:\n{stdout}"
    );
}

#[test]
fn action_bar_slot_icon_unresolved_exits_1() {
    let (code, stdout) = validate("bad_ui_texture_keys");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("slot[0]") && stdout.contains("missing_slot_icon"),
        "expected the missing ActionBar slot icon in output:\n{stdout}"
    );
}

#[test]
fn world_stat_bar_icon_sheet_unresolved_exits_1() {
    let (code, stdout) = validate("bad_world_stat_bar_icon_sheet");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("world_stat_bar Icon") && stdout.contains("missing_heart_icons"),
        "expected the missing world_stat_bar Icon icon_sheet in output:\n{stdout}"
    );
}

#[test]
fn world_stat_bar_texture_sheet_unresolved_exits_1() {
    let (code, stdout) = validate("bad_world_stat_bar_texture_sheet");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("world_stat_bar Textured") && stdout.contains("missing_bar_sheet"),
        "expected the missing world_stat_bar Textured texture_sheet in output:\n{stdout}"
    );
}

/// `target_indicator.texture` is a `decals` key despite the field name -- confirmed by reading
/// `scene_loader.rs`'s own "unknown decal key" runtime warning before writing this check, since
/// an earlier (uncommitted) claim that this resolves against `.textures` turned out to be wrong.
#[test]
fn target_indicator_texture_unresolved_exits_1() {
    let (code, stdout) = validate("bad_target_indicator_texture");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("target_indicator") && stdout.contains("missing_ring_decal")
            && stdout.contains("decals"),
        "expected the missing target_indicator decal key in output:\n{stdout}"
    );
}

/// `CameraModeDef::Party(_)` authored directly on a player prefab has no meaning for a single
/// player -- `entity_spawner.rs` silently falls back to Orbit at runtime.
#[test]
fn camera_mode_party_on_player_prefab_exits_1() {
    let (code, stdout) = validate("bad_camera_mode_party_on_player");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("solo_player") && stdout.contains("Party(...)"),
        "expected the Party-on-player-prefab error in output:\n{stdout}"
    );
}

/// A flycam-tagged prefab's `camera_mode` is only rejected by `scene_loader.rs`'s flycam-spawn
/// match arm if it isn't `Flycam(...)` -- that arm already `warn!`s and falls back to
/// `FlyCamDef::default()`; this is the design-time counterpart to that existing runtime warn.
#[test]
fn flycam_prefab_wrong_camera_mode_exits_1() {
    let (code, stdout) = validate("bad_flycam_wrong_camera_mode");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("confused_flycam") && stdout.contains("not") && stdout.contains("Flycam"),
        "expected the flycam-wrong-camera_mode error in output:\n{stdout}"
    );
}

#[test]
fn orbit_button_and_character_rotate_button_unrecognized_exits_1() {
    let (code, stdout) = validate("bad_orbit_buttons");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("orbit_button") && stdout.contains("\"Middle\""),
        "expected the unrecognised orbit_button in output:\n{stdout}"
    );
    assert!(
        stdout.contains("character_rotate_button"),
        "expected the unrecognised character_rotate_button in output:\n{stdout}"
    );
}

/// `FlyCamDef.look_button` is an unchecked vocabulary field (same shape as `orbit_button`), but
/// the movement keys are the more severe sibling debug-detective found alongside it: they go
/// through `InputMap::parse_key(..).unwrap_or(KeyCode::KeyW)` with NO warning at all, not even at
/// runtime -- the most silent authoring mistake this whole batch closes.
#[test]
fn flycam_look_button_and_movement_key_unrecognized_exits_1() {
    let (code, stdout) = validate("bad_flycam_vocab");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("look_button") && stdout.contains("\"Middle\""),
        "expected the unrecognised look_button in output:\n{stdout}"
    );
    assert!(
        stdout.contains("forward") && stdout.contains("NotAKey"),
        "expected the unrecognised forward key in output:\n{stdout}"
    );
}

/// Positive control: a valid Orbit config with `orbit_button: "None"` (a real, working opt-out)
/// and a valid Flycam config must not false-positive.
#[test]
fn valid_camera_mode_vocab_exits_0() {
    let (code, stdout) = validate("valid_camera_mode_vocab");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// System-architect finding: the legacy `components.camera` field (superseded by, but still
/// live alongside, `camera_mode: Orbit(...)`) carries the identical `orbit_button` vocabulary and
/// is in practice the DOMINANT authoring surface (`local_coop_demo` alone authors ~14 `camera:`
/// blocks, zero `camera_mode: Orbit(...)` ones) -- a `camera_mode`-only check would have missed
/// almost every real occurrence of this mistake.
#[test]
fn legacy_camera_field_orbit_button_unrecognized_exits_1() {
    let (code, stdout) = validate("bad_legacy_camera_orbit_button");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("\"solo_player\"") && stdout.contains("orbit_button \"Middle\""),
        "expected the unrecognised legacy-camera orbit_button in output:\n{stdout}"
    );
}

/// Debug-detective finding: the legacy `components.flycam` field is resolved by the identical
/// runtime parsers as `camera_mode: Flycam(...)` (`entity_spawner.rs`/`scene_loader.rs` both fall
/// back to it when `camera_mode` is unset) and ships in 2 real projects
/// (`foliage_demo`/`dynamic_animation_control`) that author `flycam:` with no `camera_mode` at
/// all -- a `camera_mode`-only check reaches none of that.
#[test]
fn legacy_flycam_field_vocab_unrecognized_exits_1() {
    let (code, stdout) = validate("bad_legacy_flycam_vocab");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("look_button") && stdout.contains("\"Middle\""),
        "expected the unrecognised legacy-flycam look_button in output:\n{stdout}"
    );
    assert!(
        stdout.contains("forward") && stdout.contains("NotAKey"),
        "expected the unrecognised legacy-flycam forward key in output:\n{stdout}"
    );
}

/// Debug-detective finding: the movement-key message must name the REAL per-field runtime
/// default, not hardcode "KeyW" for all six fields -- `down`'s actual fallback is `KeyQ`.
#[test]
fn flycam_movement_key_message_names_correct_default_exits_1() {
    let (code, stdout) = validate("bad_flycam_movement_key_down");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("down") && stdout.contains("NotAKey") && stdout.contains("KeyQ"),
        "expected the down key's REAL fallback (KeyQ, not KeyW) named in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("falls back to KeyW"),
        "must not claim the down key falls back to KeyW -- its real default is KeyQ:\n{stdout}"
    );
}

/// Debug-detective finding: `ActionBarDef.icon_sheet: Some("")` is a documented, runtime-legitimate
/// way to say "no bar-level default sheet" (`scene_loader.rs` filters it identically to `None`) --
/// must not false-positive as a missing texture key.
#[test]
fn action_bar_icon_sheet_empty_string_is_not_a_missing_key_exits_0() {
    let (code, stdout) = validate("valid_action_bar_icon_sheet_empty");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
}

/// Debug-detective finding: `IconButtonDef.icon_on`/`icon_off` are required, non-`Option` texture
/// keys resolved with zero runtime warning on a miss (`scene_loader.rs`) -- every `IconButton` in
/// every scene authors both, making this the family's highest-density unchecked surface.
#[test]
fn icon_button_icon_on_and_icon_off_unresolved_exits_1() {
    let (code, stdout) = validate("bad_icon_button_icons");
    assert_eq!(code, 1, "expected exit 1, got {code}");
    assert!(
        stdout.contains("icon_on") && stdout.contains("missing_audio_off"),
        "expected the missing icon_on texture key in output:\n{stdout}"
    );
    assert!(
        stdout.contains("icon_off") && stdout.contains("missing_audio_on"),
        "expected the missing icon_off texture key in output:\n{stdout}"
    );
}
