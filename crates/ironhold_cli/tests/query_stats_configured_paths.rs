//! Regression tests for `ironhold query`/`ironhold stats` respecting a relocated catalog path
//! (`ProjectConfig.asset_catalog`/`.prefab_catalog`), the same gap `validate.rs` closed for
//! itself in `feature/configurable_catalog_paths`. Before this fix, both commands hardcoded the
//! convention-path literal ("prefabs/prefabs.ron"/"assets.ron") and either hard-errored or
//! silently reported zero entries on a project that relocated a catalog -- even though `validate`
//! on the same project passed clean. No prior test file exists for `query`/`stats` at all, so
//! this is the first.

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

fn run(args: &[&str], fixture_name: &str) -> (i32, String) {
    let out = ironhold()
        .args(args)
        .arg(fixture(fixture_name))
        .output()
        .unwrap_or_else(|e| panic!("failed to run ironhold: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), stdout)
}

/// Before this fix: hard-errored with "prefabs/prefabs.ron not found or could not be parsed",
/// even though `real_prefab` genuinely exists at the configured `prefab_catalog` path.
#[test]
fn query_prefabs_respects_relocated_prefab_catalog() {
    let (code, stdout) = run(&["query", "prefabs"], "relocated_prefab_catalog");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("real_prefab"),
        "expected the relocated catalog's own prefab in output:\n{stdout}"
    );
}

/// Same gap, `query scenes`' player-prefab detection half (`has_player` is computed by matching
/// scene entities against the PREFAB CATALOG's player-tagged keys -- without this fix, the
/// catalog fails to load at all, so `player_prefab_keys` is empty and `has_player` is always
/// `false` regardless of what the scene actually contains). Uses a dedicated fixture (not
/// `relocated_prefab_catalog`, whose scene deliberately references a nonexistent prefab key for
/// its own `validate` regression test) so the assertion is genuinely tied to the fix rather than
/// satisfied by `glob_dir` alone finding the scene file.
#[test]
fn query_scenes_respects_relocated_prefab_catalog_for_player_detection() {
    let (code, stdout) = run(&["query", "scenes"], "relocated_prefab_catalog_player");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("player:true"),
        "expected the relocated catalog's player-tagged prefab to be detected in output:\n{stdout}"
    );
}

/// Before this fix: hard-errored with "assets.ron not found or could not be parsed", even though
/// `real_effect` genuinely exists at the configured `asset_catalog` path.
#[test]
fn query_effects_respects_relocated_asset_catalog() {
    let (code, stdout) = run(&["query", "effects"], "relocated_asset_catalog");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("real_effect"),
        "expected the relocated catalog's own effect in output:\n{stdout}"
    );
}

/// `ironhold stats`' prefab count -- before this fix, silently reported 0 (no hard error, just
/// wrong data) for a project with a relocated `prefab_catalog`.
#[test]
fn stats_respects_relocated_prefab_catalog() {
    let (code, stdout) = run(&["stats"], "relocated_prefab_catalog");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("Prefabs:   1"),
        "expected the relocated catalog's prefab to be counted in output:\n{stdout}"
    );
}

/// `ironhold stats`' asset-catalog counts -- before this fix, silently reported 0 for a project
/// with a relocated `asset_catalog`.
#[test]
fn stats_respects_relocated_asset_catalog() {
    let (code, stdout) = run(&["stats"], "relocated_asset_catalog");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("Effects:   1"),
        "expected the relocated catalog's effect to be counted in output:\n{stdout}"
    );
}

/// Positive control: the common case (no `.project.ron` at all, or one that doesn't relocate
/// anything) must keep working exactly as before -- convention-path fallback, not a regression.
#[test]
fn query_prefabs_still_works_on_convention_path_project() {
    let (code, stdout) = run(&["query", "prefabs"], "valid_project");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("box"),
        "expected the convention-path project's own prefab in output:\n{stdout}"
    );
}

#[test]
fn stats_still_works_on_convention_path_project() {
    let (code, stdout) = run(&["stats"], "valid_project");
    assert_eq!(code, 0, "expected exit 0, got {code}:\n{stdout}");
    assert!(
        stdout.contains("Prefabs:   1"),
        "expected the convention-path project's prefab to be counted in output:\n{stdout}"
    );
}

/// Pins a known, reviewed, NOT-fixed asymmetry (system-architect + alignment-reviewer,
/// `feature/cli_query_stats_paths`'s review, 2026-09-11): a *configured but missing* catalog
/// path hard-errors under `query prefabs` (naming the actual configured path, not the convention
/// literal) but is untested for `stats`, which silently reports zero entries instead -- see the
/// sibling test below. Documenting both here rather than only fixing `query`'s half, so a future
/// change to either doesn't silently widen the gap between them further.
#[test]
fn query_prefabs_hard_errors_naming_the_configured_missing_path() {
    let out = ironhold()
        .args(["query", "prefabs"])
        .arg(fixture("query_configured_prefab_catalog_missing"))
        .output()
        .unwrap_or_else(|e| panic!("failed to run ironhold: {e}"));
    let code = out.status.code().unwrap_or(-1);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(code, 2, "expected exit 2 (tool-error), got {code}");
    assert!(
        combined.contains("data/does_not_exist.ron"),
        "expected the error to name the actual CONFIGURED path, not the convention-path literal:\n{combined}"
    );
}

/// Debug-detective finding: neither `AssetCatalog` nor `PrefabCatalog` carries
/// `deny_unknown_fields`, and every field but `schema_version` is `#[serde(default)]` -- so an
/// `asset_catalog`/`prefab_catalog` field accidentally pointed at some OTHER parseable RON file
/// (here, the project's own scene) deserializes cleanly into a valid-looking but entirely EMPTY
/// catalog instead of failing to parse. Before `resolve_catalog_paths` existed this was moot (the
/// hardcoded literal was always the real file); wiring `.validate()` into `query_effects` closes
/// it back up -- the mismatched `schema_version` (scene files use 2, `AssetCatalog` expects 1)
/// is what `.validate()` catches here.
#[test]
fn query_effects_errors_on_a_misconfigured_asset_catalog_field() {
    let out = ironhold()
        .args(["query", "effects"])
        .arg(fixture("query_misconfigured_asset_catalog"))
        .output()
        .unwrap_or_else(|e| panic!("failed to run ironhold: {e}"));
    let code = out.status.code().unwrap_or(-1);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(code, 2, "expected exit 2 (tool-error), got {code}");
    assert!(
        combined.contains("schema_version"),
        "expected the AssetCatalog::validate() schema_version mismatch in output:\n{combined}"
    );
}

/// `stats` has no equivalent hard error -- it silently reports `Prefabs:   0` instead. Not fixed
/// here (out of scope for this batch: distinguishing "unset" from "configured but missing" needs
/// its own small design decision, e.g. an explicit warning line), but pinned so this doesn't
/// regress further unnoticed.
#[test]
fn stats_silently_reports_zero_for_a_configured_missing_path() {
    let (code, stdout) = run(&["stats"], "query_configured_prefab_catalog_missing");
    assert_eq!(code, 0, "expected exit 0 (known gap, not a hard error), got {code}:\n{stdout}");
    assert!(
        stdout.contains("Prefabs:   0"),
        "expected the known silent-zero behavior for a configured-but-missing catalog:\n{stdout}"
    );
}
