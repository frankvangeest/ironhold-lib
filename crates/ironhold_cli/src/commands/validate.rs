use std::collections::HashSet;
use std::path::Path;

use ironhold_core::capabilities::camera::MAX_SPLIT_PLAYERS;
use ironhold_core::schema::camera::CameraModeDef;
use ironhold_core::schema::catalog::{
    AssetCatalog, FlyCamDef, PrefabCatalog, PrefabDef, PrefabKind, WorldStatBarStyle,
};
use ironhold_core::schema::items::ItemCatalog;
use ironhold_core::schema::project::LogicRulesAsset;
use ironhold_core::schema::scene_v2::{GameSceneV2, UiNodeDef};
use ironhold_core::schema::player::{CameraConfig, InputMap};
use ironhold_core::schema::stats::StatCatalog;
use ironhold_core::schema::dialogue::{DialogueCondition, DialogueDef};
use ironhold_core::schema::material::MaterialKind;
use ironhold_core::schema::{Action, ModelFixesAsset, ProjectConfig, StateMachineAsset};
use ironhold_core::runtime::scene_manager::entity_spawner::default_camera_config;

use super::utils::{glob_dir, rel, ron_from_str};
use crate::output::OutputMode;

// ── Internal data structures ──────────────────────────────────────────────────

struct FileResult {
    rel_path: String,
    errors: Vec<String>,
}

impl FileResult {
    fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

struct CrossFileError {
    source_file: String,
    message: String,
    error_type: &'static str,
}

struct StrictWarning {
    source_file: String,
    message: String,
    kind: &'static str,
}

struct ValidationRun {
    project_name: String,
    file_results: Vec<FileResult>,
    cross_errors: Vec<CrossFileError>,
    strict_warnings: Vec<StrictWarning>,
    all_valid: bool,
}

/// Everything `do_validate` parses for one project, bundled once and passed by (cheap, `Copy`)
/// value to `cross_file_checks`/`check_ui_trigger_reachability`/`strict_checks` instead of each
/// taking its own hand-picked subset of parameters. Grew organically, one field at a time, across
/// three unrelated features (`cli_validate_hardening.md`, `orphan_rule_check`,
/// `unreachable_trigger_panel_buttons`) until `strict_checks` alone reached 10 parameters —
/// this struct is that accumulated signature, named and collected in one place instead of
/// threaded positionally through every call site. `rules`/`state_machine` carry their source path
/// (resolved by `resolve_logic_files` from `ProjectConfig.rules_path`/`state_machine_path` --
/// falling back to the `"logic/rules.ron"`/`"logic/state_machine.ron"` convention paths only when
/// there's no `.project.ron` at all, and to inline `ProjectConfig.rules` for an unset `rules_path`
/// specifically -- see that function's own doc comment) alongside the parsed asset (the richest
/// form any consumer needs — `strict_checks` uses both
/// halves; `check_ui_trigger_reachability` only needs the parsed asset and discards the path
/// itself, right after destructuring). Not every consumer uses every field — that's expected for
/// a shared context struct, not a code smell to fix.
///
/// `Copy` is sound only because every field is a shared borrow or `bool`; if a future field ever
/// needs to accumulate results (e.g. an owned `Vec` or a `&mut`), this derive has to go.
#[derive(Clone, Copy)]
struct LoadedProject<'a> {
    project_dir: &'a Path,
    project_config: Option<&'a ProjectConfig>,
    asset_catalog: Option<&'a AssetCatalog>,
    prefab_catalog: Option<&'a PrefabCatalog>,
    stat_catalog: Option<&'a StatCatalog>,
    item_catalog: Option<&'a ItemCatalog>,
    scenes: &'a [(String, GameSceneV2)],
    dialogues: &'a [(String, DialogueDef)],
    actions: &'a [(String, Action)],
    rules: Option<(&'a str, &'a LogicRulesAsset)>,
    state_machine: Option<(&'a str, &'a StateMachineAsset)>,
    behaviors: &'a [(String, StateMachineAsset)],
    /// `true` iff `logic/rules.ron`/`logic/state_machine.ron`/every `behaviors/*.behavior.ron`
    /// parsed without error. Gates `check_ui_trigger_reachability` and (combined with
    /// `scenes_parsed_cleanly`) `strict_checks`'s `orphan_rule` check — both skip entirely rather
    /// than fabricate a wave of secondary noise once a source file's rules/transitions/bindings
    /// are entirely absent from the data they compare against.
    logic_files_parsed_cleanly: bool,
    /// `true` iff every `scenes/*.scene.ron` parsed without error. Only `strict_checks`'s
    /// `orphan_rule` check needs this half — a malformed scene silently drops its buttons from
    /// the reachable-trigger set, which could make an otherwise-live rule look orphaned; the
    /// forward `unreachable_trigger` check has no equivalent exposure (a dropped scene just
    /// contributes no buttons to check, not a false positive), so it doesn't gate on this.
    scenes_parsed_cleanly: bool,
}

// ── Public result type (used by watch) ───────────────────────────────────────

pub struct ValidateResult {
    pub all_valid: bool,
    pub file_count: usize,
    /// Flat list of error strings in "path: message" format.
    pub errors: Vec<String>,
}

// ── RON parsing ───────────────────────────────────────────────────────────────

fn parse_file<T: serde::de::DeserializeOwned>(
    full_path: &Path,
    rel_path: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    let content = match std::fs::read_to_string(full_path) {
        Ok(c) => c,
        Err(e) => {
            results.push(FileResult {
                rel_path: rel_path.to_string(),
                errors: vec![format!("IO error: {e}")],
            });
            return None;
        }
    };
    match ron_from_str::<T>(&content) {
        Ok(val) => {
            results.push(FileResult { rel_path: rel_path.to_string(), errors: Vec::new() });
            Some(val)
        }
        Err(e) => {
            results.push(FileResult {
                rel_path: rel_path.to_string(),
                errors: vec![format!("line {}, col {}: {}", e.span.start.line, e.span.start.col, e.code)],
            });
            None
        }
    }
}

fn try_parse<T: serde::de::DeserializeOwned>(
    project_dir: &Path,
    rel_path: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    let full = project_dir.join(rel_path);
    if !full.exists() {
        return None;
    }
    parse_file(&full, rel_path, results)
}

/// Resolves a project catalog whose path comes from an optional `ProjectConfig` field --
/// `asset_catalog`, `prefab_catalog`, `stats_path`, `items_path` are all treated identically by
/// the runtime's `project_loader.rs` (each just `.map()`'d into an asset load; no convention-path
/// fallback if unset -- an unset field means the runtime loads nothing at all for that catalog).
///
/// `validate` deliberately does NOT mirror that "nothing at all" runtime behavior when the field
/// is unset: it falls back to `convention_path` instead, via the same tolerant `try_parse` every
/// other convention-path file in this module already uses (silently `None` if that file doesn't
/// exist either -- e.g. every project without a stat system). This keeps every project-config-less
/// or field-less fixture/project validating exactly as before this catalog became configurable
/// (confirmed: no shipped project or existing test fixture has a stray, undeclared catalog file
/// sitting at a convention path it doesn't use, so the fallback is inert for existing content)
/// while still closing the actual reported gap: an explicitly *configured* path is honored exactly,
/// and a configured-but-missing path is a hard error (unlike a merely-absent convention-path file),
/// since the runtime unconditionally tries to load whatever's configured. `try_parse` alone can't
/// express that last distinction -- it silently returns `None` for a missing file with no
/// `FileResult` pushed at all, since it's designed for "this convention path might not apply to
/// this project," not "this configured path should exist."
///
/// A `--strict` warning (`unset_catalog_path_with_convention_file`, in `strict_checks`) reports the
/// one case this fallback deliberately leaves otherwise-silent: a real project with a convention-path
/// file on disk but no matching field set, which validates clean here while the runtime loads
/// nothing for it at all.
fn load_configured_catalog<T: serde::de::DeserializeOwned>(
    project_dir: &Path,
    field: Option<&str>,
    convention_path: &str,
    field_name: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    match field {
        Some(path) => parse_configured_path(project_dir, path, field_name, results),
        None => try_parse(project_dir, convention_path, results),
    }
}

/// Parses an explicitly-configured `.project.ron` path -- the shared "explicit path is
/// authoritative" contract every configurable-path field in this file uses (`asset_catalog`,
/// `prefab_catalog`, `stats_path`, `items_path`, `rules_path`, `state_machine_path`, ...):
/// - A configured-but-missing path is a hard error, unlike a merely-absent convention-path file --
///   the runtime unconditionally tries to load whatever's configured, so `try_parse`'s silent
///   `None`-on-missing (designed for "this convention path might not apply to this project") is
///   the wrong contract here.
/// - A case/separator mismatch is reported but still parsed, not treated as missing -- a mis-cased
///   reference shouldn't also silently disable every downstream check that depends on this file
///   having loaded (a designer who fixes the casing shouldn't then be ambushed by a fresh wave of
///   unrelated errors that were there all along).
/// - A path already attempted by an earlier step this same run (any `rel_path` already in
///   `results`, regardless of what it was parsed as) is not re-parsed under a different type --
///   without this, e.g. a `rules_path` typo'd onto `prefabs/prefabs.ron` would be re-parsed as a
///   `LogicRulesAsset`, fail, and report a perfectly valid file as broken (found live during
///   `configurable_logic_paths.md`'s review).
fn parse_configured_path<T: serde::de::DeserializeOwned>(
    project_dir: &Path,
    path: &str,
    field_name: &str,
    results: &mut Vec<FileResult>,
) -> Option<T> {
    if results.iter().any(|r| r.rel_path == path) {
        return None;
    }
    if !project_dir.join(path).is_file() {
        results.push(FileResult {
            rel_path: path.to_string(),
            errors: vec![format!("{field_name} in .project.ron does not exist on disk")],
        });
        return None;
    }
    if let Some(problem) = path_case_mismatch(project_dir, path) {
        results.push(FileResult {
            rel_path: path.to_string(),
            errors: vec![format!("{field_name} {problem}")],
        });
    }
    try_parse(project_dir, path, results)
}

/// Checks whether `authored_path` (relative to `project_dir`) resolves to the exact same path a
/// browser's case-sensitive HTTP file server would see. `Path::exists()`/`Path::join` are both
/// case-insensitive and separator-tolerant on Windows/NTFS, so an authored path with wrong case
/// or a backslash separator (e.g. `"Scenes\\Main.scene.ron"`) silently validates clean here while
/// 404ing in the actual WASM/browser build. Returns `Some(problem)` describing the mismatch, or
/// `None` if the path matches exactly or doesn't exist at all -- the caller's own
/// `exists()`/`is_file()` check already reports the latter case, so callers should only invoke
/// this after that check has passed.
/// Known non-coverage (deliberately left silent, i.e. treated as "no problem", rather than risking
/// a false positive): a `.`/`..`/empty path segment (e.g. `"scenes/../scenes/main.scene.ron"`,
/// `"scenes//main.scene.ron"`) bails out of the walk below via `?` before ever reaching the
/// mismatch comparison; no shipped project or fixture authors paths this way. Case-folding for the
/// exact-match fallback uses `str::to_lowercase` (full Unicode), but the *authored* string is
/// compared byte-for-byte, so a non-ASCII filename that differs only in Unicode normalization form
/// (NFC vs NFD, e.g. on a macOS volume) could still slip through as a false negative.
/// `tools/asset_checker/check.py`'s `find_case_mismatch` is an independent Python port of this
/// same logic (byte-exact match first, case-insensitive fallback, same `.`/`..`/empty
/// non-coverage) for `assets.ron`-only spot-checks without a Rust build -- keep the two in sync.
fn path_case_mismatch(project_dir: &Path, authored_path: &str) -> Option<String> {
    if authored_path.contains('\\') {
        return Some(
            "uses a backslash (`\\`) path separator — author with forward slashes (`/`) only; \
             Windows resolves either locally, but the web build serves assets over HTTP, which \
             only understands `/`"
                .to_string(),
        );
    }
    let mut current_dir = project_dir.to_path_buf();
    let mut real_components: Vec<String> = Vec::new();
    for component in authored_path.split('/') {
        let entries: Vec<_> = std::fs::read_dir(&current_dir).ok()?.flatten().collect();
        // Prefer a byte-exact match so a project that legitimately has both `Main.scene.ron` and
        // `main.scene.ron` on a case-sensitive filesystem never has its correct reference flagged
        // just because a case-insensitively-equal sibling also exists.
        let real_name = entries
            .iter()
            .find(|entry| entry.file_name().to_string_lossy() == component)
            .or_else(|| {
                entries.iter().find(|entry| {
                    entry.file_name().to_string_lossy().to_lowercase() == component.to_lowercase()
                })
            })
            .map(|entry| entry.file_name().to_string_lossy().into_owned())?;
        current_dir.push(&real_name);
        real_components.push(real_name);
    }
    let real_path = real_components.join("/");
    (real_path != authored_path).then(|| {
        format!(
            "resolves on disk to {real_path:?} instead — the web build serves assets over a \
             case-sensitive HTTP path and will 404 on this casing; either rename the reference to \
             match, or rename the file on disk"
        )
    })
}

/// Finds the shared assets root: the nearest ancestor of `project_dir` literally named
/// `"assets"` AND containing a `projects/` or `shared/` child as corroboration -- the name check
/// alone (an earlier draft of this function) fabricates a false assets root, and therefore false
/// `missing_file` errors, for a project living anywhere under a directory that merely happens to
/// be named `assets` for an unrelated reason (e.g. a repo cloned to `D:\assets\...`) -- debug-
/// detective reproduced this live. `AssetCatalog`/material/terrain paths are authored relative to
/// this directory, not `project_dir` itself, since multiple projects reference the same `shared/`
/// files. Walking ancestors (rather than assuming a fixed "exactly two levels up
/// `assets/projects/{name}/`" distance, an even earlier draft) is what makes the single most
/// common real invocation shape work at all -- `ironhold validate .` (or `watch .`) run from
/// inside a project's own directory: an uncanonicalized relative `.` has no resolvable second
/// parent at all, so a fixed-depth `project_dir.parent().parent()` returned `None` before a
/// resolved path even entered the picture (debug-detective/system-architect finding, verified
/// live: a genuinely broken texture path validated clean when invoked as `ironhold validate .`
/// from inside the project). `std::path::absolute()` first (lexical only, no filesystem access,
/// no symlink resolution) so a relative/`.`/`..`-containing `project_dir` still resolves to
/// something `ancestors()` can walk -- deliberately not `Path::canonicalize()`, which does hit
/// the filesystem and follows symlinks, neither of which this function needs. Returns `None` for
/// anything with no corroborated `assets`-named ancestor at all (e.g. this crate's own bare
/// `tests/fixtures/{name}/` fixtures -- the fixtures for these specific checks deliberately live
/// under `tests/fixtures/assets/projects/{name}/` instead, so they have one) rather than guessing
/// wrong and resolving asset paths against some unrelated directory -- callers must treat `None`
/// as "skip these checks", not as a problem to report.
fn find_assets_root(project_dir: &Path) -> Option<std::path::PathBuf> {
    let project_dir = std::path::absolute(project_dir).ok()?;
    project_dir
        .ancestors()
        .find(|a| {
            a.file_name().is_some_and(|n| n == "assets")
                && (a.join("projects").is_dir() || a.join("shared").is_dir())
        })
        .map(|a| a.to_path_buf())
}

/// Checks a single raw asset-relative file path (e.g. `AssetCatalog.models[key].path`,
/// `TerrainConfigV2.heightmap`) for existence and case/separator correctness, resolved against
/// `assets_root` (see `find_assets_root`). A GLB `#Scene0`-style sub-asset fragment, if any, is
/// stripped before the on-disk check -- it names a scene inside the file, not part of the file
/// path itself, matching the identical `path.split('#').next()` idiom `entity_spawner.rs`/
/// `model_spawner.rs` already use when actually loading these paths.
fn check_asset_catalog_path(
    assets_root: &Path,
    source_file: &str,
    context: &str,
    authored_path: &str,
    errors: &mut Vec<CrossFileError>,
) {
    // An empty (or fragment-only, e.g. "#Scene0") path is NOT special-cased here -- it falls
    // through to the ordinary `is_file()` check below (a directory is never a file, so
    // `assets_root.join("")` correctly fails it) and is reported like any other missing file,
    // rather than silently ignored. The one legitimate empty-path case in this schema
    // (`CustomMaterialDef.shader`'s documented "empty means built-in magenta fallback") is
    // filtered at its own call site before reaching this function, not here.
    let file_part = authored_path.split('#').next().unwrap_or("");
    // An absolute path (e.g. pasted from a file-browser "copy path" on the author's own machine,
    // `C:/Users/.../shared/models/hero.glb`) must be rejected explicitly, not silently joined --
    // `Path::join` with an absolute RHS *discards* the LHS entirely, so `assets_root.join(abs)`
    // would resolve to the author's own local file and report a perfect false negative: valid
    // only on that one machine, and never over the actual asset-relative HTTP path a real build
    // serves from (debug-detective finding).
    if Path::new(file_part).is_absolute() || Path::new(file_part).has_root() {
        errors.push(CrossFileError {
            source_file: source_file.to_string(),
            message: format!(
                "{context}: path {authored_path:?} is an absolute path -- author it relative to \
                 the asset root `assets/` instead (e.g. \"shared/models/character-01.glb\"), or \
                 it will only resolve on this machine, never in the actual build"
            ),
            error_type: "absolute_asset_path",
        });
    } else if !assets_root.join(file_part).is_file() {
        errors.push(CrossFileError {
            source_file: source_file.to_string(),
            message: format!(
                "{context}: path {authored_path:?} not found on disk (paths are relative to \
                 the asset root `assets/`, e.g. \"shared/models/character-01.glb\" or \
                 \"projects/{{name}}/terrain/heightmap.png\")"
            ),
            error_type: "missing_file",
        });
    } else if let Some(problem) = path_case_mismatch(assets_root, file_part) {
        errors.push(CrossFileError {
            source_file: source_file.to_string(),
            message: format!("{context}: path {authored_path:?} {problem}"),
            error_type: "path_case_mismatch",
        });
    }
}

/// Checks a designer-authored `AssetCatalog.textures` key (not a raw file path -- the runtime
/// resolves the key to a path itself, e.g. `asset_catalog.textures.get(key)`). Used for the new
/// icon/texture-sheet checks this pass adds: `InventoryPanelDef`/`ContainerPanelDef.icon_sheet`,
/// `ActionBarDef.icon_sheet`/`ActionSlotDef.icon`, and `WorldStatBarStyle::Icon.icon_sheet`/
/// `::Textured.texture_sheet` -- a miss silently renders a blank/unchanged image node with no
/// runtime warning at all for every one of these EXCEPT `Textured.texture_sheet`, which does
/// `warn!` and skips spawning the bar entirely (`stat_display.rs`) -- still worth a design-time
/// check (a hard error beats discovering it via a runtime log line), just not claimed as silent
/// here. Deliberately NOT used to retrofit the two pre-existing sibling
/// checks of this same shape (`FoliageMaterialDef.leaf_texture`, which has its own empty-string
/// guard whose full reasoning wasn't re-verified here; `ItemDef.icon_sheet`, which already
/// resolves a relocation-aware `assets.ron` name this helper doesn't parametrize) -- left as-is
/// to keep this batch additive rather than a refactor. `error_type: "missing_catalog_key"`
/// matches the pre-existing `leaf_texture` check's own error_type, distinct from
/// `"missing_reference"` (used elsewhere in this file for cross-entity references like
/// `currency_stat`/`item_key`, not a same-catalog key lookup) -- `ItemDef.icon_sheet` uses
/// `"missing_reference"` instead, a pre-existing inconsistency not addressed here.
fn check_texture_key(
    asset_catalog: Option<&AssetCatalog>,
    source_file: &str,
    context: &str,
    key: &str,
    errors: &mut Vec<CrossFileError>,
) {
    let Some(assets) = asset_catalog else { return };
    if !assets.textures.contains_key(key) {
        errors.push(CrossFileError {
            source_file: source_file.to_string(),
            message: format!("{context}: texture key {key:?} not found in assets.ron's textures"),
            error_type: "missing_catalog_key",
        });
    }
}

/// `split`/`party` authored INSIDE a `camera_mode: Orbit(...)` payload (instead of as siblings of
/// `camera_mode` under `components:`) parse fine but are never read (`entity_spawner.rs`'s
/// spawn-time match arm) -- a hard error, since there is no legitimate reason to author them
/// there and no fallback makes it work anyway. Shared between prefab-authored `camera_mode:` and
/// a scene's `camera_modes:` registry entries; `registry` selects which remedy is actually
/// authorable at the call site -- a registry entry has no `components:` block and cannot author
/// `split`/`party` in any form, so its remedy is "delete them," not "move them."
///
/// **Known non-coverage, deliberately not fixed here:** `split`/`party` only exist as fields on
/// `CameraConfig` (the `Orbit` payload); `Follow`/`FirstPerson`/`Fixed`/`Flycam`'s own payload
/// structs have no such fields and none of the five carry `#[serde(deny_unknown_fields)]`, so the
/// identical mistake authored inside any of those four variants is silently dropped by serde
/// *before* this function -- or any other post-parse check -- ever sees it. Closing that requires
/// a schema-level decision (adding `deny_unknown_fields` to four more structs, a breaking change
/// for any project currently relying on the silent drop), not a validate.rs addition; logged to
/// `planning/claude_suggestions.md` rather than attempted here.
fn camera_mode_nested_split_party_problem(
    mode: &CameraModeDef,
    context: &str,
    registry: bool,
) -> Option<(String, &'static str)> {
    let CameraModeDef::Orbit(cfg) = mode else { return None };
    if cfg.split.is_none() && cfg.party.is_none() {
        return None;
    }
    let remedy = if registry {
        "delete them -- `split`/`party` are authored on a player prefab's `components:` block, \
         never inside a `camera_modes:` registry preset, which has no such concept at all"
    } else {
        "they must be siblings of `camera_mode`, e.g. \
         `components: (camera_mode: Orbit(...), split: (...))`"
    };
    Some((
        format!(
            "{context}: `split`/`party` authored INSIDE `camera_mode: Orbit(...)` are never read \
             and have no effect — {remedy}"
        ),
        "camera_mode_nested_split_party",
    ))
}

/// A `Fixed(...)` camera mode with both `look_at`/`look_at_entity` set, or neither, only warned
/// at runtime for prefab-authored `camera_mode:` (`entity_spawner.rs`'s spawn-time match arm) --
/// no `ironhold_cli validate` counterpart at either the prefab or the `camera_modes:` registry
/// level. `--strict`-gated, not a hard error: both shapes are *working*, not broken --
/// `fixed_camera_system` (`capabilities/camera.rs`) resolves `look_at_entity` when it's live and
/// falls back to `look_at` otherwise (a legitimate "track this entity, or this static point if it
/// isn't around" pattern), and a `Fixed` camera with neither field set simply holds whatever
/// rotation it already has -- identity rotation if freshly spawned into this mode, or its
/// previous rotation if reached via `SetCameraMode` (deliberately not claiming a single fixed
/// facing here, since that differs between the two paths and an earlier draft of this message
/// was found to be wrong on the registry path). Shared between both call sites since the
/// condition and message don't otherwise depend on where the mode came from.
fn camera_mode_fixed_look_at_problem(mode: &CameraModeDef, context: &str) -> Option<(String, &'static str)> {
    let CameraModeDef::Fixed(fx) = mode else { return None };
    if fx.look_at.is_some() && fx.look_at_entity.is_some() {
        Some((
            format!(
                "{context}: `Fixed(...)` has both `look_at` and `look_at_entity` set — \
                 `look_at_entity` wins whenever it resolves to a live entity, falling back to \
                 `look_at` otherwise; this is fine if that fallback is intentional, otherwise \
                 remove whichever field you don't want used"
            ),
            "camera_mode_fixed_ambiguous_look_at",
        ))
    } else if fx.look_at.is_none() && fx.look_at_entity.is_none() {
        Some((
            format!(
                "{context}: `Fixed(...)` has neither `look_at` nor `look_at_entity` set — the \
                 camera never turns to look at anything; it just holds whatever rotation it \
                 already has (identity rotation if freshly spawned into this mode, its prior \
                 rotation if switched into it via SetCameraMode)"
            ),
            "camera_mode_fixed_missing_look_at",
        ))
    } else {
        None
    }
}

/// Unrecognized string-vocabulary/key fields inside a `CameraModeDef` payload -- shared between
/// prefab-authored `camera_mode:` and a scene's `camera_modes:` registry entries, same pattern as
/// `camera_mode_nested_split_party_problem`/`camera_mode_fixed_look_at_problem` above. Two
/// payload types have fields whose runtime parser only ever `warn!`s and silently substitutes a
/// default on an unrecognized value, with no `ironhold_cli validate` counterpart until now:
/// `Orbit(CameraConfig)`'s `orbit_button`/`character_rotate_button` (`parse_orbit_button`, valid:
/// `"Left"`/`"Right"`/`"Either"`/`"None"`) and `Flycam(FlyCamDef)`'s `look_button`
/// (`parse_flycam_look_button`, valid: `"Left"`/`"Right"`/`"Either"`, no `"None"`). `Flycam`'s six
/// movement-key fields (`forward`/`backward`/`left`/`right`/`up`/`down`) are a stricter, worse
/// gap: they go through `InputMap::parse_key(..).unwrap_or(KeyCode::KeyW)` with **no warning at
/// all**, not even a runtime one -- a typo'd flycam movement key is completely silent at both
/// design time and runtime before this check existed.
const ORBIT_BUTTON_VALUES: &[&str] = &["Left", "Right", "Either", "None"];
const LOOK_BUTTON_VALUES: &[&str] = &["Left", "Right", "Either"];

/// Checks a `CameraConfig` payload's own vocabulary fields -- shared by every place one can be
/// authored: `camera_mode: Orbit(...)`, and the legacy `components.camera` field it superseded
/// (`entity_spawner.rs`'s `orbit_state_from_config` reads either one identically via the same
/// `parse_orbit_button`, and the legacy field is still the ONLY place these values appear in
/// several shipped projects -- `local_coop_demo` alone authors ~14 `camera:` blocks and zero
/// `camera_mode: Orbit(...)` ones, so a `CameraModeDef`-only check would miss the majority of
/// real authored `orbit_button`/`character_rotate_button` values, system-architect finding).
fn orbit_config_vocab_problems(cfg: &CameraConfig, context: &str) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    if !ORBIT_BUTTON_VALUES.contains(&cfg.orbit_button.as_str()) {
        out.push((
            format!(
                "{context}: orbit_button {:?} is not one of \"Left\"/\"Right\"/\"Either\"/\
                 \"None\" -- the runtime warns and falls back to \"Either\"",
                cfg.orbit_button
            ),
            "invalid_binding",
        ));
    }
    if let Some(rot) = &cfg.character_rotate_button {
        if !ORBIT_BUTTON_VALUES.contains(&rot.as_str()) {
            out.push((
                format!(
                    "{context}: character_rotate_button {:?} is not one of \"Left\"/\"Right\"/\
                     \"Either\"/\"None\" -- the runtime warns and falls back to \"Either\"",
                    rot
                ),
                "invalid_binding",
            ));
        }
    }
    out
}

/// Checks a `FlyCamDef` payload's own vocabulary/key fields -- shared by `camera_mode:
/// Flycam(...)` and the legacy `components.flycam` field it superseded (both resolved by the
/// identical `parse_flycam_look_button`/`InputMap::parse_key` calls at every flycam-camera spawn
/// site; `camera_modes`/`dynamic_animation_control`/`foliage_demo` all still author `flycam:
/// (...)`, never `camera_mode: Flycam(...)`, so this is the dominant real authoring surface, not
/// the `CameraModeDef` variant). The six movement-key fields are the more severe check: they go
/// through `InputMap::parse_key(..).unwrap_or(<per-field default>)` with **no warning at all**,
/// not even a runtime one -- a typo'd flycam movement key was completely silent at both design
/// time and runtime before this check existed. Defaults verified against
/// `scene_loader.rs`'s/`entity_spawner.rs`'s flycam-spawn sites, NOT assumed to all be `KeyW`
/// (an earlier draft of this message wrongly claimed that for every field).
fn flycam_def_vocab_problems(fc: &FlyCamDef, context: &str) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    if !LOOK_BUTTON_VALUES.contains(&fc.look_button.as_str()) {
        out.push((
            format!(
                "{context}: look_button {:?} is not one of \"Left\"/\"Right\"/\"Either\" -- the \
                 runtime warns and falls back to \"Either\"",
                fc.look_button
            ),
            "invalid_binding",
        ));
    }
    for (field_name, key, default_key) in [
        ("forward", &fc.forward, "KeyW"), ("backward", &fc.backward, "KeyS"),
        ("left", &fc.left, "KeyA"), ("right", &fc.right, "KeyD"),
        ("up", &fc.up, "Space"), ("down", &fc.down, "KeyQ"),
    ] {
        if InputMap::parse_key(key).is_none() {
            out.push((
                format!(
                    "{context}: {field_name} {:?} is not a recognised key -- the runtime \
                     silently falls back to {default_key} with NO warning at all, not even at \
                     runtime",
                    key
                ),
                "invalid_key",
            ));
        }
    }
    out
}

/// Delegates to whichever of the two payload-specific checks above applies to this
/// `CameraModeDef` variant -- see their doc comments for the runtime behavior each closes.
fn camera_mode_vocab_problems(mode: &CameraModeDef, context: &str) -> Vec<(String, &'static str)> {
    match mode {
        CameraModeDef::Orbit(cfg) => orbit_config_vocab_problems(cfg, context),
        CameraModeDef::Flycam(fc) => flycam_def_vocab_problems(fc, context),
        _ => Vec::new(),
    }
}

/// A subset of non-ASCII characters common enough in pasted-in text to warrant a specific,
/// actionable name in the diagnostic, rather than the generic "non-ASCII character" fallback --
/// dash-like characters (the originally-reported em-dash tofu-box incident) plus the curly quotes
/// and ellipsis a word processor's "smart punctuation" autocorrect is most likely to introduce.
/// Not an exhaustive allowlist: `find_unrenderable_char` below flags every non-ASCII character,
/// named or not (debug-detective finding, `cli_validate_small_wins` review, 2026-09-07: the
/// embedded UI font, `FiraMono-subset.ttf`, was confirmed via its cmap to cover ONLY
/// U+0020..U+007E -- so a dash-only allowlist missed real shipped tofu strings using `·`/`→`/`×`/
/// `°`, e.g. `particles_demo`'s `"Campfire ×4"` and `effect_mayhem_demo`'s
/// `"Walk in → full-sphere burst"`).
const NAMED_NON_ASCII_CHARS: &[(char, &str)] = &[
    ('\u{2010}', "hyphen"),
    ('\u{2011}', "non-breaking hyphen"),
    ('\u{2013}', "en dash"),
    ('\u{2014}', "em dash"),
    ('\u{2015}', "horizontal bar"),
    ('\u{2212}', "minus sign"),
    ('\u{FF0D}', "fullwidth hyphen-minus"),
    ('\u{2018}', "left single quote"),
    ('\u{2019}', "right single quote"),
    ('\u{201C}', "left double quote"),
    ('\u{201D}', "right double quote"),
    ('\u{2026}', "ellipsis"),
    ('\u{00A0}', "non-breaking space"),
];

/// Returns the first character in `text` the embedded UI font has no glyph for (any non-ASCII
/// character -- the font's cmap covers only U+0020..U+007E, see `NAMED_NON_ASCII_CHARS`'s doc
/// comment), along with a human-readable name: a specific one from `NAMED_NON_ASCII_CHARS` for the
/// common cases, or a generic fallback for anything else.
fn find_unrenderable_char(text: &str) -> Option<(char, &'static str)> {
    let ch = text.chars().find(|c| !c.is_ascii())?;
    let name = NAMED_NON_ASCII_CHARS.iter()
        .find(|(named, _)| *named == ch)
        .map_or("non-ASCII character", |(_, name)| name);
    Some((ch, name))
}

/// Keep-first-on-collision: like `HashMap::insert`, but the FIRST value inserted for a key wins
/// and is what's returned on a later collision, instead of the immediately-preceding one --
/// matching the runtime's own `seen.get()`-then-insert-only-when-absent pattern, so a 3rd+
/// colliding entry cites the SAME first-seen entry as its partner the runtime console does
/// (system-architect + debug-detective finding, `gamepad_action_bar_slots.md` review).
fn first_seen<K: std::hash::Hash + Eq, V: Clone>(
    seen: &mut std::collections::HashMap<K, V>,
    key: K,
    value: V,
) -> Option<V> {
    match seen.entry(key) {
        std::collections::hash_map::Entry::Occupied(e) => Some(e.get().clone()),
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(value);
            None
        }
    }
}

// ── File discovery ────────────────────────────────────────────────────────────

fn find_project_ron(project_dir: &Path) -> Option<String> {
    std::fs::read_dir(project_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|name| name.ends_with(".project.ron"))
}

/// A scene path authored outside the `scenes/` convention (`Action::LoadScene`/
/// `LoadSceneOverlay`/`PreloadScene`/`ToggleOverlay`, or `ProjectConfig.initial_scene`) was
/// previously only existence/case-checked, never parsed -- so its contents (entities, ui,
/// camera_modes, spawn_points, ...) were invisible to every cross-file check that walks `scenes`.
/// Parses and folds in any such path, so it participates exactly like a conventionally-placed
/// scene from this point on. Returns the rel_paths actually attempted (whether or not the parse
/// succeeded), for the caller's `scenes_parsed_cleanly` computation.
///
/// Every candidate is filtered before it ever reaches `try_parse`, since this is the first place
/// in this file that hands a *designer-authored* string (rather than a hardcoded convention path)
/// to a parse-then-record helper, and each of the following was reproduced as a real bug during
/// review before this filtering was added:
/// - empty, or containing a `.`/`..`/empty path segment -- `project_dir.join("")` is the project
///   root itself, and `..` can walk outside it entirely, reading and cross-checking a *different*
///   project's file and attributing its errors to this one. Declining to discover these mirrors
///   `path_case_mismatch`'s own documented non-coverage of the same shapes -- silently missing a
///   mistake here is far safer than acting on an attacker-shaped path.
/// - already present in `file_results` (i.e. *any* file this run already attempted to parse as
///   something else, successfully or not -- a catalog, `logic/rules.ron`, a behavior, whatever
///   the `scenes/` glob already covered) -- otherwise a `LoadScene` typo'd onto an existing
///   non-scene file (e.g. `LoadScene("prefabs/prefabs.ron")`) would be re-parsed as a `GameSceneV2`,
///   fail, and report a perfectly valid file as broken; worse, that spurious `FileResult` would
///   also cascade into `scenes_parsed_cleanly` going false and silently disabling the `orphan_rule`
///   `--strict` check. Comparing against `file_results` (not just `scenes`, which holds only
///   *successful* scene parses) also closes the same duplicate-report risk for a genuinely broken
///   `scenes/*.scene.ron` that's also referenced by an action -- without this it would be parsed
///   and reported twice for one mistake.
/// - flagged by `path_case_mismatch` -- that's already reported as its own error; also attempting
///   to parse under the wrong spelling would additionally duplicate the real file's diagnostics
///   under a second, mis-cased name.
/// - not `is_file()` (covers "doesn't exist" *and* "is a directory") -- `try_parse` only guards
///   `exists()`, which is true for a directory too and previously produced a confusing raw IO
///   error ("Access is denied") instead of a clean skip. A missing path is still reported
///   separately by the existing action/`initial_scene` exists()-check in `cross_file_checks`.
fn discover_extra_scenes(
    project_dir: &Path,
    initial_scene: Option<&str>,
    actions: &[(String, Action)],
    scenes: &mut Vec<(String, GameSceneV2)>,
    file_results: &mut Vec<FileResult>,
) -> Vec<String> {
    let is_unsafe_shape =
        |path: &str| path.is_empty() || path.split('/').any(|c| c.is_empty() || c == "." || c == "..");

    let mut candidates: Vec<&str> = Vec::new();
    if let Some(path) = initial_scene {
        candidates.push(path);
    }
    for (_, action) in actions {
        if let Action::LoadScene(path)
        | Action::LoadSceneOverlay(path)
        | Action::PreloadScene(path)
        | Action::ToggleOverlay(path) = action
        {
            candidates.push(path);
        }
    }

    let already_attempted: HashSet<String> = file_results.iter().map(|r| r.rel_path.clone()).collect();
    let mut attempted_this_pass: HashSet<String> = HashSet::new();
    for path in candidates {
        if is_unsafe_shape(path) {
            continue;
        }
        if already_attempted.contains(path) || !attempted_this_pass.insert(path.to_string()) {
            continue;
        }
        if !project_dir.join(path).is_file() {
            continue;
        }
        if path_case_mismatch(project_dir, path).is_some() {
            continue;
        }
        if let Some(scene) = try_parse::<GameSceneV2>(project_dir, path, file_results) {
            scenes.push((path.to_string(), scene));
        }
    }
    attempted_this_pass.into_iter().collect()
}

/// Resolved `rules.ron`/`state_machine.ron` content plus the rel_path each half should be
/// attributed to (for `source_file`/`error_type` messages and the `logic_files_parsed_cleanly`
/// filter) -- see `resolve_logic_files`'s doc comment for why the source path isn't always a
/// literal `"logic/rules.ron"`/`"logic/state_machine.ron"` anymore. The `*_source` strings are
/// filter keys for `logic_files_parsed_cleanly`, not attributions of a real asset -- both are
/// still populated (to the project.ron's own name) even when the matching `Option` is `None`
/// (an unset field with no inline rules, or an unset `state_machine_path`), so do not "simplify"
/// this into `Option<(String, Asset)>` pairs: the filter specifically needs to see a project.ron
/// whose *configured* path failed to parse, which requires the source to outlive a failed parse.
struct ResolvedLogicFiles {
    rules: Option<LogicRulesAsset>,
    rules_source: String,
    state_machine: Option<StateMachineAsset>,
    state_machine_source: String,
}

/// Mirrors the runtime's actual resolution (`project_loader.rs::check_project_loaded`) instead of
/// the two hardcoded convention-path literals this function used to always parse: once a
/// `.project.ron` exists, `rules_path`/`state_machine_path` are the ONLY source for their half --
/// an unset `rules_path` falls back to the inline V1 `ProjectConfig.rules` (the runtime never
/// looks at `"logic/rules.ron"` on disk in that case, even if the file exists), and an unset
/// `state_machine_path` means no state machine at all, full stop. Getting this wrong is exactly
/// how a project that sets only `state_machine_path` (e.g. `3rd_person_game_demo`, `terrain_demo`)
/// previously had its unrelated, dead-at-runtime `logic/rules.ron` silently counted as live by
/// every cross-file check.
///
/// Two deliberate divergences from the runtime:
/// - When there's no *parseable* `.project.ron` (either none exists, or the one that does failed
///   to parse -- `project_config` is `None` either way), still fall back to checking the
///   convention-path files, matching the existing `load_configured_catalog` precedent -- there's
///   no config to consult, so there's no "wrong field" to get out of sync with, and refusing to
///   look would silently stop checking logic entirely for every fixture/project that predates
///   configurable paths.
/// - A configured path containing a `..` segment IS followed (unlike `discover_extra_scenes`,
///   which refuses to discover one) -- this is intentional, not an oversight: `rules_path`/
///   `state_machine_path` are exactly-one-hardcoded-field values the runtime resolves with the
///   identical plain `format!("{root}/{path}")` (`resolve_project_path`), so following it here
///   is runtime-faithful, whereas `discover_extra_scenes`'s candidates are arbitrary
///   designer-authored strings from inside action bodies, a materially different trust boundary.
///
/// A configured-but-missing path is a hard error (see `parse_configured_path`), not `try_parse`'s
/// silent `None` -- correct for a convention-path guess, but this is a `.project.ron`-authored
/// field the runtime unconditionally tries to load.
fn resolve_logic_files(
    project_dir: &Path,
    project_config: Option<&ProjectConfig>,
    file_results: &mut Vec<FileResult>,
) -> ResolvedLogicFiles {
    let Some(config) = project_config else {
        // No .project.ron at all -- nothing to divide by field, fall back to both convention
        // paths exactly like this function always did (needed for every fixture/project that
        // predates configurable paths).
        return ResolvedLogicFiles {
            rules: try_parse::<LogicRulesAsset>(project_dir, "logic/rules.ron", file_results),
            rules_source: "logic/rules.ron".to_string(),
            state_machine: try_parse::<StateMachineAsset>(
                project_dir, "logic/state_machine.ron", file_results,
            ),
            state_machine_source: "logic/state_machine.ron".to_string(),
        };
    };

    let (rules, rules_source) = match config.rules_path.as_deref() {
        Some(path) => (
            parse_configured_path::<LogicRulesAsset>(project_dir, path, "rules_path", file_results),
            path.to_string(),
        ),
        // No convention-path fallback here on purpose -- the runtime's own fallback for an unset
        // rules_path is the inline V1 field, never a guess at "logic/rules.ron" existing on disk.
        // The source string is a filter key for logic_files_parsed_cleanly, not an attribution --
        // it's populated (to the project.ron's own name) even when `rules` ends up `None`, same as
        // the state_machine arm below, so a project.ron that fails to parse (routing to the
        // no-.project.ron branch above instead) is the only way either half comes up genuinely
        // sourceless.
        None => {
            let rules = (!config.rules.is_empty())
                .then(|| LogicRulesAsset { schema_version: 2, rules: config.rules.clone() });
            (rules, find_project_ron(project_dir).unwrap_or_default())
        }
    };

    let (state_machine, state_machine_source) = match config.state_machine_path.as_deref() {
        Some(path) => (
            parse_configured_path::<StateMachineAsset>(
                project_dir, path, "state_machine_path", file_results,
            ),
            path.to_string(),
        ),
        // No inline V1 equivalent exists for the state machine -- an unset state_machine_path
        // means no state machine at all, matching the runtime exactly.
        None => (None, find_project_ron(project_dir).unwrap_or_default()),
    };

    ResolvedLogicFiles { rules, rules_source, state_machine, state_machine_source }
}

// ── Action collection ─────────────────────────────────────────────────────────

fn collect_actions(
    rules: Option<(&str, &LogicRulesAsset)>,
    state_machine: Option<(&str, &StateMachineAsset)>,
    behaviors: &[(String, StateMachineAsset)],
    dialogues: &[(String, DialogueDef)],
) -> Vec<(String, Action)> {
    let mut out = Vec::new();

    if let Some((src, r)) = rules {
        for rule in &r.rules {
            for action in &rule.do_actions {
                out.push((src.to_string(), action.clone()));
            }
        }
    }
    if let Some((src, fsm)) = state_machine {
        for action in fsm_actions(fsm) {
            out.push((src.to_string(), action));
        }
    }
    for (path, behavior) in behaviors {
        for action in fsm_actions(behavior) {
            out.push((path.clone(), action));
        }
    }
    for (path, dialogue) in dialogues {
        for node in &dialogue.nodes {
            for choice in &node.choices {
                for action in &choice.do_actions {
                    out.push((path.clone(), action.clone()));
                }
            }
        }
    }
    out
}

fn fsm_actions(fsm: &StateMachineAsset) -> Vec<Action> {
    let mut out = Vec::new();
    for state in &fsm.states {
        out.extend(state.entry_actions.iter().cloned());
        out.extend(state.exit_actions.iter().cloned());
        for binding in &state.on {
            out.extend(binding.do_actions.iter().cloned());
        }
    }
    for binding in &fsm.global_on {
        out.extend(binding.do_actions.iter().cloned());
    }
    out
}

// ── Cross-file checks ─────────────────────────────────────────────────────────

fn cross_file_checks(project: LoadedProject) -> Vec<CrossFileError> {
    let LoadedProject {
        project_dir, project_config, asset_catalog, prefab_catalog, stat_catalog, item_catalog,
        scenes, dialogues, actions, ..
    } = project;
    let mut errors = Vec::new();

    // All four catalog types have real schema-level invariants their own `.validate()` enforces
    // (e.g. `AssetCatalog::validate()` rejects an empty `models[]`/`decals[]` path,
    // `PrefabCatalog::validate()` rejects a Foliage prefab missing its `foliage` block,
    // `StatCatalog::validate()` rejects `min > max`/a modifier referencing an undefined stat,
    // `ItemCatalog::validate()` rejects `max_stack: 0`/an empty `display_name`) -- but
    // `do_validate` only ever calls `try_parse`/`load_configured_catalog`, never the parsed
    // catalog's own `.validate()`, so these invariants were runtime-only. Wiring all four in here
    // costs nothing further parse-side; verified against all 15 shipped projects (schema_version
    // 1/2 for asset/prefab catalogs respectively) before adding this. `source_file` resolves each
    // catalog's actually-configured path (mirroring the `items_source` idiom further below),
    // since all four are relocatable via `ProjectConfig` fields -- a hardcoded convention-path
    // literal would misattribute the error on a project that moved one.
    for (catalog_err, convention_path, field_name, error_type) in [
        (
            asset_catalog.and_then(|c| c.validate().err()),
            "assets.ron",
            project_config.and_then(|c| c.asset_catalog.as_deref()),
            "invalid_asset_catalog",
        ),
        (
            prefab_catalog.and_then(|c| c.validate().err()),
            "prefabs/prefabs.ron",
            project_config.and_then(|c| c.prefab_catalog.as_deref()),
            "invalid_prefab_catalog",
        ),
        (
            stat_catalog.and_then(|c| c.validate().err()),
            "stats/stats.ron",
            project_config.and_then(|c| c.stats_path.as_deref()),
            "invalid_stat_catalog",
        ),
        (
            item_catalog.and_then(|c| c.validate().err()),
            "items/items.ron",
            project_config.and_then(|c| c.items_path.as_deref()),
            "invalid_item_catalog",
        ),
    ] {
        let Some(message) = catalog_err else { continue };
        errors.push(CrossFileError {
            source_file: field_name.unwrap_or(convention_path).to_string(),
            message,
            error_type,
        });
    }

    for (source, action) in actions {
        match action {
            Action::SpawnEffect { key, .. } => {
                if let Some(c) = asset_catalog {
                    if !c.effects.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("effect key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::ProjectDecal { key, .. } => {
                if let Some(c) = asset_catalog {
                    if !c.decals.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("decal key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PlaySound { key, .. } | Action::PlayMusicLoop { key, .. } => {
                if let Some(c) = asset_catalog {
                    if !c.audio.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("audio key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            // See also: Action::Spawn's `spawn_point` reference check further below, in its own
            // loop over `actions` (grouped with the other scene-scoped "union across all scenes"
            // checks rather than here, since it needs `scenes`, not a catalog).
            Action::Spawn { prefab, .. } => {
                if let Some(c) = prefab_catalog {
                    if !c.prefabs.contains_key(prefab) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("prefab key {:?} not found in prefabs.ron", prefab),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PreloadPrefab(key) => {
                if let Some(c) = prefab_catalog {
                    if !c.prefabs.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("prefab key {:?} not found in prefabs.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PreloadGlb(key) => {
                if let Some(c) = asset_catalog {
                    if !c.models.contains_key(key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("model key {:?} not found in assets.ron", key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::LoadScene(path)
            | Action::LoadSceneOverlay(path)
            | Action::PreloadScene(path)
            | Action::ToggleOverlay(path) => {
                if !project_dir.join(path).exists() {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!(
                            "scene path {:?} not found on disk (paths are relative to the \
                             project folder, e.g. \"scenes/main.scene.ron\")",
                            path
                        ),
                        error_type: "missing_file",
                    });
                } else if let Some(problem) = path_case_mismatch(project_dir, path) {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!("scene path {:?} {}", path, problem),
                        error_type: "path_case_mismatch",
                    });
                }
            }
            Action::StartDialogue { dialogue_path, .. } => {
                if !project_dir.join(dialogue_path).exists() {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!(
                            "dialogue path {:?} not found on disk (paths are relative to the \
                             project folder, e.g. \"dialogues/npc_intro.dialogue.ron\")",
                            dialogue_path
                        ),
                        error_type: "missing_file",
                    });
                } else if let Some(problem) = path_case_mismatch(project_dir, dialogue_path) {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!("dialogue path {:?} {}", dialogue_path, problem),
                        error_type: "path_case_mismatch",
                    });
                }
            }
            Action::ApplyModifier { modifier_key } | Action::RemoveModifier { modifier_key } => {
                if let Some(c) = stat_catalog {
                    if !c.modifiers.contains_key(modifier_key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!(
                                "modifier key {:?} not found in stats.ron",
                                modifier_key
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::AddItem { item_key, .. }
            | Action::RemoveItem { item_key, .. }
            | Action::TransferItem { item_key, .. }
            | Action::BuyItem(item_key) => {
                if let Some(c) = item_catalog {
                    if !c.items.contains_key(item_key) {
                        errors.push(CrossFileError {
                            source_file: source.clone(),
                            message: format!("item_key {:?} not found in items.ron", item_key),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            Action::PlayAnimationOn { start_at_fraction: Some(fraction), .. } => {
                if !(0.0..=1.0).contains(fraction) {
                    errors.push(CrossFileError {
                        source_file: source.clone(),
                        message: format!(
                            "PlayAnimationOn: start_at_fraction {:?} is outside the valid \
                             [0.0, 1.0] range — it's a fraction of the clip's duration, not seconds",
                            fraction
                        ),
                        error_type: "animation_start_at_fraction_out_of_range",
                    });
                }
            }
            _ => {}
        }

        // `{new_id}` only resolves inside Action::Spawn's `id` field (action_executor.rs) --
        // anywhere else (a typo, or a misunderstanding of the token) it silently bakes a literal
        // "{new_id}" substring into a live runtime string instead of resolving, which then fails
        // to match whatever it was meant to reference.
        let misplaced_new_id = match action {
            Action::Spawn { .. } => false,
            other => format!("{:?}", other).contains("{new_id}"),
        };
        if misplaced_new_id {
            errors.push(CrossFileError {
                source_file: source.clone(),
                message: "{new_id} only resolves inside Action::Spawn's `id` field -- it will not \
                    be substituted here and will appear as a literal string at runtime"
                    .to_string(),
                error_type: "misplaced_new_id_token",
            });
        }
    }

    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for entity in &scene.entities {
                if !catalog.prefabs.contains_key(&entity.prefab) {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "entity {:?}: prefab {:?} not found in prefabs.ron",
                            entity.id, entity.prefab
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
        }
    }

    // A merchant's currency_stat/item_key are only ever read at the moment a player opens the
    // shop (Action::OpenShop) or attempts a purchase (Action::BuyItem) — a typo in either
    // otherwise only surfaces as a runtime no-op the first time someone actually tries to trade.
    // Catches it here instead, mirroring every other key-lookup check in this file.
    if let Some(catalog) = prefab_catalog {
        let mut prefab_keys: Vec<&String> = catalog.prefabs.keys().collect();
        prefab_keys.sort();
        for prefab_key in prefab_keys {
            let prefab = &catalog.prefabs[prefab_key];
            let Some(merchant) = &prefab.merchant else { continue };
            if let Some(stats) = stat_catalog {
                if !stats.stats.contains_key(&merchant.currency_stat) {
                    // currency_stat defaults to "gold" when omitted entirely -- if that's the
                    // value that's missing, the designer may not have authored it at all, so say
                    // so rather than implying they typed a bad stat key.
                    let default_note = if merchant.currency_stat == "gold" {
                        " (this is the schema default used when currency_stat is omitted -- \
                          either define a \"gold\" stat or set currency_stat explicitly)"
                    } else {
                        ""
                    };
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: merchant currency_stat {:?} not found in stats.ron{}",
                            prefab_key, merchant.currency_stat, default_note
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
            if let Some(items) = item_catalog {
                for entry in &merchant.stock {
                    if !items.items.contains_key(&entry.item_key) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message: format!(
                                "prefab {:?}: merchant stock item_key {:?} not found in items.ron",
                                prefab_key, entry.item_key
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
        }
    }

    // A prefab's `inventory.initial_items[].item_key` is only read at spawn time -- a typo there
    // doesn't drop the item, it silently creates a stack with no catalog entry (entity_spawner.rs
    // passes None for the catalog, so add_to_slots falls back to max_stack: 99 and the panel
    // renders it at icon_index 0 of the default sheet): a phantom, wrong-icon slot instead of a
    // design-time error. Same failure shape as the merchant stock check above.
    if let Some(catalog) = prefab_catalog {
        if let Some(items) = item_catalog {
            let mut prefab_keys: Vec<&String> = catalog.prefabs.keys().collect();
            prefab_keys.sort();
            for prefab_key in prefab_keys {
                let prefab = &catalog.prefabs[prefab_key];
                let Some(inventory) = &prefab.inventory else { continue };
                for entry in &inventory.initial_items {
                    if !items.items.contains_key(&entry.item_key) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message: format!(
                                "prefab {:?}: inventory initial_items item_key {:?} not found in \
                                 items.ron",
                                prefab_key, entry.item_key
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
        }
    }

    // `ItemDef.currency_stat`/`.icon_sheet` are both only read at the moment they'd actually
    // matter (looting the item; rendering its inventory slot) -- a typo in either doesn't stop
    // the item from being usable, it just silently loses the currency gain (currency_stat --
    // action_executor.rs does warn! at runtime, but only there, and only after the item is
    // already gone) or falls back to the panel's default icon sheet with no signal at all
    // (icon_sheet). Same failure shape as the merchant currency_stat check above, catching both
    // at design time instead.
    if let Some(items) = item_catalog {
        // items.ron's path is configurable via ProjectConfig.items_path (unlike stats.ron's
        // fixed convention path) -- pointing this diagnostic at a literal "items.ron" would
        // send the designer to a path that doesn't exist in every shipped project (they all
        // use "items/items.ron"). Fall back to the literal only in the unreachable case where
        // an item_catalog exists without items_path having been set.
        let items_source = project_config
            .and_then(|c| c.items_path.clone())
            .unwrap_or_else(|| "items.ron".to_string());
        let mut item_keys: Vec<&String> = items.items.keys().collect();
        item_keys.sort();
        for item_key in item_keys {
            let item = &items.items[item_key];
            if let Some(stats) = stat_catalog {
                if let Some(currency_stat) = &item.currency_stat {
                    if !stats.stats.contains_key(currency_stat) {
                        errors.push(CrossFileError {
                            source_file: items_source.clone(),
                            message: format!(
                                "item {:?}: currency_stat {:?} not found in stats.ron",
                                item_key, currency_stat
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            if let Some(assets) = asset_catalog {
                if let Some(icon_sheet) = &item.icon_sheet {
                    if !assets.textures.contains_key(icon_sheet) {
                        let assets_convention_name = project_config
                            .and_then(|c| c.asset_catalog.as_deref())
                            .unwrap_or("assets.ron");
                        errors.push(CrossFileError {
                            source_file: items_source.clone(),
                            message: format!(
                                "item {:?}: icon_sheet {:?} not found in {}'s textures",
                                item_key, icon_sheet, assets_convention_name
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
        }
    }

    // `join_prefab_keys` (local_coop_hot_join_leave.md) entries are read by Action::JoinPlayer
    // only at the moment a player actually presses the join key — a typo'd or missing entry
    // otherwise only surfaces as a runtime warn!+no-op, never at authoring time. Catch it here,
    // and mirror the same three Action::JoinPlayer executor guards (in-bounds, player-tagged,
    // GLB-only) so a scene author sees the mistake before ever running the project rather than
    // discovering a silent no-op (or, for the primitive case, a spawn-time panic) during a
    // playtest. The `player_{slot + 1}_start` spawn-point check (debug-detective finding) is
    // folded into this same slot-by-slot walk, deliberately gated behind the other three: a slot
    // beyond `MAX_SPLIT_PLAYERS` can never actually be hot-joined into at all (the executor bails
    // on `next_slot >= MAX_SPLIT_PLAYERS` before ever reading `join_prefab_keys`), and neither can
    // one whose prefab is missing/non-player/Primitive -- demanding a spawn point for a slot that
    // can never be reached would be a false positive, not a real authoring mistake.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for (slot, entry) in scene.join_prefab_keys.iter().enumerate() {
                let Some(prefab_key) = entry else { continue };
                if slot >= MAX_SPLIT_PLAYERS as usize {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: slot is beyond MAX_SPLIT_PLAYERS ({}) — a \
                             hot-join can never reach this slot; the entry has no effect",
                            slot, MAX_SPLIT_PLAYERS
                        ),
                        error_type: "unreachable_join_slot",
                    });
                    continue;
                }
                let Some(prefab) = catalog.prefabs.get(prefab_key) else {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: prefab {:?} not found in prefabs.ron",
                            slot, prefab_key
                        ),
                        error_type: "missing_reference",
                    });
                    continue;
                };
                if !prefab.is_player() {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: prefab {:?} has no `tags: [\"player\"]` — \
                             Action::JoinPlayer will refuse to hot-join it at runtime",
                            slot, prefab_key
                        ),
                        error_type: "unsupported_join_prefab",
                    });
                    continue;
                }
                if prefab.kind == PrefabKind::Primitive {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: prefab {:?} is primitive-shaped (kind: \
                             Primitive) — hot-join only supports GLB (Actor-kind) players in v1",
                            slot, prefab_key
                        ),
                        error_type: "unsupported_join_prefab",
                    });
                    continue;
                }
                // `Action::JoinPlayer` derives `player_{slot + 1}_start` (1-based) from this same
                // slot index and looks it up in the SAME scene's `spawn_points`, with no `warn!`
                // at all on a miss — it silently falls back to the primary player's position plus
                // an X offset instead. Scene-scoped (both fields live on one `GameSceneV2`) unlike
                // Action::Spawn's `spawn_point` check further below, which must approximate across
                // every scene in the project. Known remaining gap, not modelled here: an overlay
                // scene loaded over this one can desync which scene's `join_prefab_keys` vs.
                // `spawn_points` the runtime actually reads from — logged separately, not fixed in
                // this diagnostic (see planning/claude_suggestions.md).
                let spawn_point_key = format!("player_{}_start", slot + 1);
                if !scene.spawn_points.contains_key(spawn_point_key.as_str()) {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "join_prefab_keys[{}]: no spawn_points entry named {:?} — a player \
                             hot-joining into this slot will silently spawn next to the primary \
                             player instead of at their own spawn point",
                            slot, spawn_point_key
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
        }
    }

    // A dialogue node `id` must be unique within its own file (the schema doc says so, but
    // nothing enforced it) -- `capabilities/dialogue.rs`'s `nodes.iter().position(...)` matches
    // only the FIRST node with a given id, so a duplicate makes the second one permanently
    // unreachable by any `jump_to` targeting it, with zero diagnostic anywhere. A `jump_to` that
    // names no node at all (and isn't the reserved `"__end__"`) is the milder sibling: the
    // runtime only `warn!`s and closes the dialogue panel mid-conversation, a visible and
    // confusing break with no design-time counterpart until now.
    for (dialogue_path, dialogue) in dialogues {
        let mut seen_ids: HashSet<&str> = HashSet::new();
        for node in &dialogue.nodes {
            if !seen_ids.insert(node.id.as_str()) {
                errors.push(CrossFileError {
                    source_file: dialogue_path.clone(),
                    message: format!(
                        "duplicate DialogueNode id {:?} — jump_to can only ever reach the FIRST \
                         node with this id; every later one is permanently unreachable",
                        node.id
                    ),
                    error_type: "duplicate_node_id",
                });
            }
        }
        let node_ids: HashSet<&str> = dialogue.nodes.iter().map(|n| n.id.as_str()).collect();
        for node in &dialogue.nodes {
            for (i, choice) in node.choices.iter().enumerate() {
                if let Some(jump_to) = &choice.jump_to {
                    if jump_to != "__end__" && !node_ids.contains(jump_to.as_str()) {
                        errors.push(CrossFileError {
                            source_file: dialogue_path.clone(),
                            message: format!(
                                "DialogueNode {:?} choice[{}]: jump_to {:?} names no node in \
                                 this dialogue (and isn't \"__end__\") — the conversation will \
                                 silently close when this choice is picked",
                                node.id, i, jump_to
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
                if let Some(DialogueCondition::StatAtLeast { stat_key, .. }) = &choice.condition {
                    if let Some(stats) = stat_catalog {
                        if !stats.stats.contains_key(stat_key) {
                            errors.push(CrossFileError {
                                source_file: dialogue_path.clone(),
                                message: format!(
                                    "DialogueNode {:?} choice[{}]: condition StatAtLeast \
                                     stat_key {:?} not found in stats.ron — this choice will \
                                     always be hidden",
                                    node.id, i, stat_key
                                ),
                                error_type: "missing_reference",
                            });
                        }
                    }
                }
            }
        }
    }

    // The project's own boot scene -- the highest-consequence scene path of all, since a typo
    // here means the project never gets past a blank/loading screen. Same on-disk-existence
    // check as the LoadScene/LoadSceneOverlay/PreloadScene/ToggleOverlay action arm above,
    // mirroring how the runtime resolves it (project_loader.rs's resolve_project_path).
    if let Some(config) = project_config {
        if !project_dir.join(&config.initial_scene).exists() {
            errors.push(CrossFileError {
                source_file: find_project_ron(project_dir).unwrap_or_default(),
                message: format!(
                    "initial_scene {:?} not found on disk (paths are relative to the project \
                     folder, e.g. \"scenes/main.scene.ron\")",
                    config.initial_scene
                ),
                error_type: "missing_file",
            });
        } else if let Some(problem) = path_case_mismatch(project_dir, &config.initial_scene) {
            errors.push(CrossFileError {
                source_file: find_project_ron(project_dir).unwrap_or_default(),
                message: format!("initial_scene {:?} {}", config.initial_scene, problem),
                error_type: "path_case_mismatch",
            });
        }
    }

    // `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings` (gamepad_hot_join.md)
    // button names are only checked at runtime (a `warn!` in
    // project_loader.rs/scene_loader.rs) — same design-time gap `join_prefab_keys` above closes
    // for its own field. Catch it here too, so a typo'd button name surfaces at validate time
    // instead of only as a silent no-op the first time someone presses it.
    if let Some(config) = project_config {
        for button_name in config.global_unclaimed_gamepad_bindings.keys() {
            if InputMap::parse_gamepad_button(button_name).is_none() {
                errors.push(CrossFileError {
                    source_file: find_project_ron(project_dir).unwrap_or_default(),
                    message: format!(
                        "global_unclaimed_gamepad_bindings: unrecognised button name {:?} — binding will have no effect",
                        button_name
                    ),
                    error_type: "invalid_binding",
                });
            }
        }
    }
    for (scene_path, scene) in scenes {
        for button_name in scene.scene_unclaimed_gamepad_bindings.keys() {
            if InputMap::parse_gamepad_button(button_name).is_none() {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "scene_unclaimed_gamepad_bindings: unrecognised button name {:?} — binding will have no effect",
                        button_name
                    ),
                    error_type: "invalid_binding",
                });
            }
        }
    }

    // A primitive-shaped (`kind: Primitive`) player prefab combined with `scene.terrain:
    // Some(...)` isn't supported yet — v3 of `player_model_source_unification.md`. Mirrors the
    // scene-load-time `warn!` in `scene_loader.rs`; this is the design-time counterpart so a
    // scene author sees it before ever running the project.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            if scene.terrain.is_none() { continue; }
            for entity in &scene.entities {
                let Some(prefab) = catalog.prefabs.get(&entity.prefab) else { continue };
                if prefab.kind == PrefabKind::Primitive
                    && prefab.is_player()
                {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "entity {:?}: primitive-shaped player prefab {:?} combined with \
                             scene.terrain — primitive players aren't supported on terrain-\
                             deferred spawn yet (v3 of player_model_source_unification.md); use a \
                             GLB (Actor-kind) player prefab for terrain scenes, or remove terrain \
                             from this scene",
                            entity.id, entity.prefab
                        ),
                        error_type: "unsupported_primitive_player_on_terrain",
                    });
                }
            }
        }
    }

    // A scene authoring 2+ `tags: ["flycam"]` entities silently keeps only the last one in
    // `entities:` order at runtime (`scene_loader.rs`) — this is the design-time counterpart so a
    // scene author sees it before ever running the project. See
    // `planning/features/flycam_scene_conflicts.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            let mut flycam_ids: Vec<&str> = Vec::new();
            for entity in &scene.entities {
                let Some(prefab) = catalog.prefabs.get(&entity.prefab) else { continue };
                if prefab.is_flycam() {
                    flycam_ids.push(&entity.id);
                }
            }
            if flycam_ids.len() > 1 {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "scene has {} `tags: [\"flycam\"]` entities ({}) — only the last one in \
                         `entities:` order is used at runtime, the rest are silently discarded. \
                         Remove all but one flycam-tagged entity from this scene.",
                        flycam_ids.len(),
                        flycam_ids.join(", ")
                    ),
                    error_type: "duplicate_flycam_entity",
                });
            }
        }
    }

    // A `tags: ["flycam"]` prefab's `model`/`shape`/`primitive`/`children` are silently discarded
    // at scene load (`scene_loader.rs`'s `is_flycam` branch `continue`s before any of them are
    // ever consulted), and a prefab tagged both `"player"` and `"flycam"` never spawns its player
    // components at all — this is the design-time counterpart to both scene-load `warn!`s.
    // Prefab-catalog-scoped (not per-scene, unlike `duplicate_flycam_entity` above): the condition
    // is entirely prefab-local, so one bad prefab would otherwise report once per scene that
    // instantiates it. Both scoped to scene-`entities:`-placed flycams specifically — a flycam
    // prefab dynamically `Action::Spawn`ed at runtime doesn't go through this branch and isn't
    // covered (logged in `planning/claude_suggestions.md`).
    // See `planning/features/flycam_model_never_renders_warning.md`.
    if let Some(catalog) = prefab_catalog {
        let mut prefab_keys: Vec<&String> = catalog.prefabs.keys().collect();
        prefab_keys.sort();
        for prefab_key in prefab_keys {
            let prefab = &catalog.prefabs[prefab_key];
            if !prefab.is_flycam() {
                continue;
            }
            if prefab.is_player() {
                errors.push(CrossFileError {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab '{}' has both \"player\" and \"flycam\" tags — the flycam tag \
                         makes it spawn as a camera-only entity and its player components never \
                         spawn at all. Use camera_mode: Flycam(...) on a \"player\"-only prefab \
                         instead if you want a flying player character, or remove the \"player\" \
                         tag if you wanted a plain camera-only flycam.",
                        prefab_key
                    ),
                    error_type: "flycam_player_tag_conflict",
                });
                continue;
            }
            let ignored_fields = prefab.flycam_ignored_fields();
            if !ignored_fields.is_empty() {
                let remedy = PrefabDef::flycam_ignored_fields_remedy(&ignored_fields);
                errors.push(CrossFileError {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "flycam prefab '{}' sets {} — a flycam is camera-only and never renders \
                         a body, so that body will never appear. To silence this, {}. To give a \
                         flying camera a visible body, use camera_mode: Flycam(...) on a \
                         \"player\" prefab instead, or spawn the body as a separate non-flycam \
                         entity at the same position.",
                        prefab_key, ignored_fields.join(", "), remedy
                    ),
                    error_type: "flycam_model_never_renders",
                });
            }
        }
    }

    for (scene_path, scene) in scenes {
        // Scene-wide (not per-bar) so a slot key shared across two different `ActionBar`s is
        // also caught here, not just within one bar's own slots — per-player action bars
        // (`owner_player`, see `planning/features/per_player_split_screen_targeting.md` Phase 2)
        // are the first feature to author 2+ `ActionBar`s in one scene, and a cross-bar collision
        // is worse than "the wrong slot fires": `CooldownMap`/`PendingIntentActions`/
        // `HandledIntentSlots` are keyed by the literal slot_key string alone, scene-wide, so a
        // `rules.ron` rule handling one bar's intent on a colliding key silently suppresses the
        // other bar's pending slot too.
        // `_` lets the compiler infer bevy's `KeyCode` from `InputMap::parse_key`'s return
        // type without this file needing its own `use`/import to name it (this crate already
        // links bevy transitively via ironhold_core — this only avoids one import line).
        //
        // Keyed by positional ui-node index, not `bar.id` — `id` is documented "Unique
        // identifier" but nothing actually enforces that, and comparing by `id` would
        // misclassify (or silently miss) a real cross-bar collision if two bars happened to
        // share an id (system-architect finding, per_player_split_screen_targeting.md Phase 2).
        let mut seen: std::collections::HashMap<_, (usize, &str, &str)> = std::collections::HashMap::new();
        for (node_index, node) in scene.ui.iter().enumerate() {
            let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
            for slot in &bar.slots {
                match ironhold_core::schema::player::InputMap::parse_key(&slot.key) {
                    None => errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "ActionBar {:?}: slot {:?} has an unrecognised key {:?} — it will never fire",
                            bar.id, slot.key, slot.key
                        ),
                        error_type: "invalid_key",
                    }),
                    Some(kc) => {
                        let prev = first_seen(&mut seen, kc, (node_index, &bar.id, &slot.key));
                        if let Some((prev_node_index, prev_bar, prev_key)) = prev {
                            if prev_node_index == node_index {
                                errors.push(CrossFileError {
                                    source_file: scene_path.clone(),
                                    message: format!(
                                        "ActionBar {:?}: slots {:?} and {:?} both resolve to {:?} — only {:?} will fire on press",
                                        bar.id, prev_key, slot.key, kc, prev_key
                                    ),
                                    error_type: "duplicate_key",
                                });
                            } else {
                                errors.push(CrossFileError {
                                    source_file: scene_path.clone(),
                                    message: format!(
                                        "ActionBar {:?} slot {:?} and ActionBar {:?} slot {:?} both resolve to {:?} — \
                                         the intent/cooldown pipeline is keyed by slot_key alone, scene-wide, so a \
                                         rules.ron rule handling one bar's intent on this key will also silently \
                                         suppress the other bar's pending slot",
                                        prev_bar, prev_key, bar.id, slot.key, kc
                                    ),
                                    error_type: "cross_bar_duplicate_key",
                                });
                            }
                        }
                    }
                }
            }
        }

        // `Label`/`Button.font_size <= 0.0` — Bevy's text pipeline doesn't panic on this (guarded
        // before cosmic-text's own `assert_ne!(font_size, 0.0)`), it just silently renders
        // nothing, and the one `warn!` it does log fires via `once!` — a per-process flag, so
        // only the very first offending entity in the whole session is ever reported; a second
        // mis-authored label anywhere, or the same one on a scene reload, produces no diagnostic
        // at all. That "looks fine but is subtly wrong" failure mode is exactly what this CLI
        // check exists to catch at design time instead.
        for node in &scene.ui {
            let (kind, id, font_size) = match node {
                ironhold_core::schema::scene_v2::UiNodeDef::Label(l) => ("Label", &l.id, l.font_size),
                ironhold_core::schema::scene_v2::UiNodeDef::Button(b) => ("Button", &b.id, b.font_size),
                _ => continue,
            };
            if font_size <= 0.0 {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "{kind} {id:?}: font_size {font_size} must be > 0.0 — the text will silently \
                         render nothing (Bevy only warns once, ever, for the whole process)"
                    ),
                    error_type: "invalid_font_size",
                });
            }
        }

        // `icon_sheet`/`icon` fields on the panel/bar UI nodes are `AssetCatalog.textures` keys
        // resolved at scene-load time (`scene_loader.rs`) -- a miss silently skips loading that
        // atlas (InventoryPanel/ContainerPanel) or leaves a slot's icon unresolved (ActionBar),
        // with no runtime `warn!` anywhere. `Option<String>` fields are only checked when actually
        // set — omitting them is a normal, working authoring choice (e.g. an ActionBar whose every
        // slot sets its own `icon` override, or a panel not showing icons at all).
        for node in &scene.ui {
            match node {
                // `icon_on`/`icon_off` are required (non-`Option`) `String` fields resolved via
                // `asset_catalog.textures.get(...).unwrap_or_default()` with zero runtime warning
                // (`scene_loader.rs`) -- every `IconButton` in every scene authors both, so this
                // is the family's highest-density surface, not an edge case (debug-detective
                // finding).
                UiNodeDef::IconButton(btn) => {
                    check_texture_key(
                        asset_catalog, scene_path,
                        &format!("IconButton {:?}: icon_on", btn.id),
                        &btn.icon_on, &mut errors,
                    );
                    check_texture_key(
                        asset_catalog, scene_path,
                        &format!("IconButton {:?}: icon_off", btn.id),
                        &btn.icon_off, &mut errors,
                    );
                }
                UiNodeDef::InventoryPanel(panel) => {
                    if let Some(icon_sheet) = &panel.icon_sheet {
                        check_texture_key(
                            asset_catalog, scene_path,
                            &format!("InventoryPanel {:?}: icon_sheet", panel.id),
                            icon_sheet, &mut errors,
                        );
                    }
                }
                UiNodeDef::ContainerPanel(panel) => {
                    if let Some(icon_sheet) = &panel.icon_sheet {
                        check_texture_key(
                            asset_catalog, scene_path,
                            &format!("ContainerPanel {:?}: icon_sheet", panel.id),
                            icon_sheet, &mut errors,
                        );
                    }
                }
                UiNodeDef::ActionBar(bar) => {
                    // `scene_loader.rs`'s own slot-sheet resolution does
                    // `bar.icon_sheet.as_deref().filter(|s| !s.is_empty())` -- `Some("")` is
                    // treated identically to `None` (no default sheet for this bar), the same
                    // sentinel `ActionSlotDef.icon`'s own empty-string guard below already
                    // respects. Missing this on `bar.icon_sheet` too would hard-error a working,
                    // documented authoring form (debug-detective finding).
                    if let Some(icon_sheet) = bar.icon_sheet.as_deref().filter(|s| !s.is_empty()) {
                        check_texture_key(
                            asset_catalog, scene_path,
                            &format!("ActionBar {:?}: icon_sheet", bar.id),
                            icon_sheet, &mut errors,
                        );
                    }
                    for (i, slot) in bar.slots.iter().enumerate() {
                        if !slot.icon.is_empty() {
                            check_texture_key(
                                asset_catalog, scene_path,
                                &format!("ActionBar {:?} slot[{i}]: icon", bar.id),
                                &slot.icon, &mut errors,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        // `target_indicator.texture` is an `AssetCatalog.decals` key (NOT `.textures` -- despite
        // the field name, it's a ground-ring decal like `Action::ProjectDecal`), resolved at scene
        // load (`scene_loader.rs`'s `target_indicator` setup). A miss already `warn!`s at runtime
        // ("unknown decal key ... indicator disabled for this scene") but had no design-time
        // counterpart until now.
        if let Some(indicator) = &scene.target_indicator {
            if let Some(c) = asset_catalog {
                if !c.decals.contains_key(&indicator.texture) {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "target_indicator: texture {:?} not found in assets.ron's decals",
                            indicator.texture
                        ),
                        error_type: "missing_reference",
                    });
                }
            }
        }

        // Same-player gamepad-slot collision — a different failure mode than the keyboard check
        // above, so a separate pass: the intent/cooldown pipeline is never keyed by `gamepad_key`,
        // so there's no cross-bar pipeline entanglement risk here. The risk is a same-player
        // double-fire (one physical button press activating 2 slots for the same player). Keyed
        // by `(owner_player.unwrap_or(0), GamepadButton)` — matching the runtime's
        // `owns_slot`/`warn_missing_player_stat_templates` "None/Some(0) both mean the primary
        // player" normalization — so two *different* players' bars sharing a button name (each has
        // their own physical pad) is correctly not flagged. See
        // `planning/features/gamepad_action_bar_slots.md`.
        let mut seen_gamepad: std::collections::HashMap<(u32, _), (&str, &str)> = std::collections::HashMap::new();
        for node in &scene.ui {
            let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
            let owner_player = bar.owner_player.unwrap_or(0);
            for slot in &bar.slots {
                let Some(gk) = &slot.gamepad_key else { continue };
                match ironhold_core::schema::player::InputMap::parse_gamepad_button(gk) {
                    None => errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "ActionBar {:?}: slot {:?} has an unrecognised gamepad_key {:?} — it will never fire from gamepad",
                            bar.id, slot.key, gk
                        ),
                        error_type: "invalid_gamepad_key",
                    }),
                    Some(btn) => {
                        let prev = first_seen(&mut seen_gamepad, (owner_player, btn), (&bar.id, &slot.key));
                        if let Some((prev_bar, prev_key)) = prev {
                            errors.push(CrossFileError {
                                source_file: scene_path.clone(),
                                message: format!(
                                    "Player {} has 2+ ActionBar slots bound to gamepad button {:?}: ActionBar {:?} \
                                     slot {:?} and ActionBar {:?} slot {:?} — one press of this button would \
                                     activate both slots for this player",
                                    owner_player, btn, prev_bar, prev_key, bar.id, slot.key
                                ),
                                error_type: "same_player_gamepad_duplicate_key",
                            });
                        }
                    }
                }
            }
        }
    }

    // Per-player action-bar cost slots whose stat isn't declared on the owning player's own
    // `stat_templates` — the player clearly opted into a per-player pool (declares
    // `stat_templates` at all), so a slot costing an undeclared key would silently fall back to
    // the shared global `LoadedStats` pool for just that one stat. Deliberately does not error
    // when the owning player declares no `stat_templates` at all — that's the ordinary, unchanged
    // shared-pool fallback. `owner_player.unwrap_or(0)`, not an early-continue on `None` — mirrors
    // `owns_slot`'s runtime "None/Some(0) both mean the primary player" resolution, so a default
    // (owner_player omitted) bar gets the same coverage as an explicit `owner_player: 0` one
    // (debug-detective finding). See `planning/features/per_player_stat_pools.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for node in &scene.ui {
                let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
                let owner_player = bar.owner_player.unwrap_or(0);
                let player_prefab = scene.entities.iter()
                    .filter_map(|e| catalog.prefabs.get(&e.prefab))
                    .find(|p| p.player_index == owner_player && p.is_player());
                let Some(prefab) = player_prefab else { continue };
                if prefab.stat_templates.is_empty() { continue; }
                for slot in &bar.slots {
                    let Some(cost) = &slot.cost else { continue };
                    if !prefab.stat_templates.iter().any(|t| t.key == cost.stat) {
                        errors.push(CrossFileError {
                            source_file: scene_path.clone(),
                            message: format!(
                                "ActionBar {:?} slot {:?} costs stat {:?}, but player_index {}'s \
                                 prefab declares stat_templates without that key — this slot's \
                                 cost will silently fall back to the shared global LoadedStats \
                                 pool instead of this player's own pool",
                                bar.id, slot.key, cost.stat, owner_player
                            ),
                            error_type: "missing_player_stat_template",
                        });
                    }
                }
            }
        }
    }

    // A slot's `gamepad_key` resolves against its owning player's own `BoundGamepad` (seeded once
    // from `InputMap.gamepad_index`; `gamepad_bind_system` never falls back to any connected
    // pad), so a slot that declares `gamepad_key` for a player whose prefab sets no
    // `gamepad_index` at all is silently inert: no crash, no runtime signal, the slot simply
    // never fires from gamepad. Mirrors the `missing_player_stat_template` check above exactly,
    // including the `unwrap_or(0)` normalization. See `planning/features/gamepad_action_bar_slots.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            for node in &scene.ui {
                let ironhold_core::schema::scene_v2::UiNodeDef::ActionBar(bar) = node else { continue };
                let owner_player = bar.owner_player.unwrap_or(0);
                let player_prefab = scene.entities.iter()
                    .filter_map(|e| catalog.prefabs.get(&e.prefab))
                    .find(|p| p.player_index == owner_player && p.is_player());
                let Some(prefab) = player_prefab else { continue };
                let has_gamepad_index = prefab.components.inputs.as_ref()
                    .is_some_and(|i| i.gamepad_index.is_some());
                if has_gamepad_index { continue; }
                for slot in &bar.slots {
                    let Some(gamepad_key) = &slot.gamepad_key else { continue };
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "ActionBar {:?} slot {:?} declares gamepad_key {:?}, but player_index \
                             {}'s prefab sets no inputs.gamepad_index — this binding will never \
                             fire from gamepad (the slot's keyboard key, if any, still works)",
                            bar.id, slot.key, gamepad_key, owner_player
                        ),
                        error_type: "gamepad_key_without_gamepad_index",
                    });
                }
            }
        }
    }

    // Two or more player-tagged prefabs **instantiated in the same scene's `entities:` list, or
    // reachable via that scene's `join_prefab_keys` hot-join slots** authoring the same non-`None`
    // `gamepad_index` — one physical controller would drive both characters at once, whether both
    // are scene-placed, both are hot-join slots, or one of each (a hot-joined player's
    // `gamepad_index` seed is read from its prefab exactly like a scene-placed player's, unless a
    // gamepad-triggered join instead captures the triggering pad directly — see "Gamepad-triggered
    // hot join" in `crates/ironhold_core/src/CLAUDE.md` — so a keyboard-triggered join can still
    // collide with an already-bound scene player via this same seed). Deliberately scoped to each
    // scene's instantiated/reachable players, not the raw prefab catalog: `local_coop_demo`'s
    // catalog legitimately reuses `gamepad_index` values across different rooms' player variants
    // (never co-instantiated), which a catalog-wide check would false-positive on. Mirrors the
    // runtime `warn!` in `scene_loader.rs`'s `warn_duplicate_gamepad_index` only for the
    // `entities:` half — that warning only scans players already instantiated at scene-load time,
    // so it cannot see a `join_prefab_keys` collision at all; this check is the only design-time
    // (or any-time) signal for that case until the join spawn path itself. See
    // `planning/features/gamepad_player_binding_hardening.md`.
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            let mut seen: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
            let mut check_seed = |id: String, prefab_key: &str, errors: &mut Vec<CrossFileError>| {
                let Some(prefab) = catalog.prefabs.get(prefab_key) else { return };
                if !prefab.is_player() { return }
                let Some(seed) = prefab.components.inputs.as_ref().and_then(|i| i.gamepad_index)
                else { return };
                let prev = first_seen(&mut seen, seed, id.clone());
                if let Some(other_id) = prev {
                    errors.push(CrossFileError {
                        source_file: scene_path.clone(),
                        message: format!(
                            "players {:?} and {:?} both use gamepad_index: {} — one physical \
                             controller would drive both characters at once. Give each player a \
                             different gamepad_index. Deliberately sharing one controller between \
                             two characters is not supported",
                            other_id, id, seed
                        ),
                        error_type: "duplicate_gamepad_index",
                    });
                }
            };
            for entity_def in &scene.entities {
                check_seed(entity_def.id.clone(), &entity_def.prefab, &mut errors);
            }
            for (slot, entry) in scene.join_prefab_keys.iter().enumerate() {
                let Some(prefab_key) = entry else { continue };
                check_seed(
                    format!("join_prefab_keys[{slot}] (prefab {prefab_key:?})"),
                    prefab_key,
                    &mut errors,
                );
            }
        }
    }

    if let Some(catalog) = prefab_catalog {
        // Sorted, not the HashMap's arbitrary iteration order -- so error output (and any
        // snapshot/regression test asserting on it) is stable across runs instead of depending on
        // hash-seed-driven ordering.
        let mut prefab_keys: Vec<&String> = catalog.prefabs.keys().collect();
        prefab_keys.sort();
        for key in prefab_keys {
            let def = &catalog.prefabs[key];
            if let Some(behavior_path) = &def.behavior {
                if !project_dir.join(behavior_path).exists() {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: behavior {:?} not found on disk",
                            key, behavior_path
                        ),
                        error_type: "missing_file",
                    });
                } else if let Some(problem) = path_case_mismatch(project_dir, behavior_path) {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!("prefab {:?}: behavior {:?} {}", key, behavior_path, problem),
                        error_type: "path_case_mismatch",
                    });
                }
            }

            if let Some(dialogue_path) = &def.dialogue {
                if !project_dir.join(dialogue_path).exists() {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: dialogue {:?} not found on disk",
                            key, dialogue_path
                        ),
                        error_type: "missing_file",
                    });
                } else if let Some(problem) = path_case_mismatch(project_dir, dialogue_path) {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!("prefab {:?}: dialogue {:?} {}", key, dialogue_path, problem),
                        error_type: "path_case_mismatch",
                    });
                }
            }

            // Resolved the same way as `behavior`/`dialogue` just above (project-relative, not
            // assets-root-relative -- `entity_spawner.rs`'s `resolve_project_path`), and a 404 here
            // is worse than either: the entity is spawned `Visibility::Hidden` pending the policy
            // load (`entity_spawner.rs:186`) and never becomes visible again once that load fails.
            if let Some(policy_path) = &def.animation_policy {
                if !project_dir.join(policy_path).exists() {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: animation_policy {:?} not found on disk",
                            key, policy_path
                        ),
                        error_type: "missing_file",
                    });
                } else if let Some(problem) = path_case_mismatch(project_dir, policy_path) {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: animation_policy {:?} {}", key, policy_path, problem
                        ),
                        error_type: "path_case_mismatch",
                    });
                }
            }

            // `components.camera_mode` is only ever read for a player-tagged prefab
            // (`assemble_player_config`, `entity_spawner.rs`) -- gating here avoids a misleading
            // "the camera will..." diagnostic on a prefab whose `camera_mode` the runtime never
            // consumes at all (an untagged prop, or a `tags: ["flycam"]` prefab, whose non-Flycam
            // `camera_mode` gets its own dedicated check further below -- a real, different
            // mistake worded for its own failure mode, not this one).
            if def.is_player() {
                if let Some(mode) = &def.components.camera_mode {
                    if let Some((message, error_type)) = camera_mode_nested_split_party_problem(
                        mode,
                        &format!("prefab {key:?}: camera_mode"),
                        false,
                    ) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message,
                            error_type,
                        });
                    }
                    // `Party(_)` authored directly on a player prefab has no meaning for a single
                    // player -- `entity_spawner.rs`'s `resolve_orbit_config_for_multiplayer` warns
                    // and silently falls back to Orbit at runtime. The `camera_modes:` registry
                    // loop already rejects this identical shape (`unsupported_registry_camera_mode`,
                    // below); the prefab-level loop had no equivalent until now.
                    if matches!(mode, CameraModeDef::Party(_)) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message: format!(
                                "prefab {key:?}: camera_mode: Party(...) has no meaning on a \
                                 single player prefab -- the runtime silently falls back to Orbit \
                                 at spawn time. Use camera_mode: Orbit(...) directly, or author \
                                 Party framing via the scene's own party:/split.dynamic fields \
                                 instead"
                            ),
                            error_type: "unsupported_prefab_camera_mode",
                        });
                    }
                }
            }

            // A `tags: ["flycam"]` prefab's `camera_mode`, UNLIKE `model`/`shape`/`primitive`/
            // `children` (which really are silently discarded by the same branch), is not
            // discarded at all -- `scene_loader.rs` passes it straight through as the flycam's
            // mode. It's only rejected one step later, where the flycam-spawn code itself
            // pattern-matches on `CameraModeDef::Flycam(_)` specifically: anything else already
            // gets its own runtime `warn!` ("has no defined behavior for a standalone flycam...
            // falling back to Flycam defaults") and a `FlyCamDef::default()` fallback -- verified
            // directly against that match arm before writing this check, since an earlier
            // (uncommitted) draft of this comment wrongly assumed a silent discard. This is the
            // design-time counterpart to that existing runtime warn, same twin-check pattern used
            // throughout this file. Deliberately NOT folded into the player-tagged check above:
            // this is a different authoring mistake (wrong mode shape on a flycam prefab, not a
            // nested-split/Party mistake on a player prefab), worded for its own failure mode.
            if def.is_flycam() {
                if let Some(mode) = &def.components.camera_mode {
                    if !matches!(mode, CameraModeDef::Flycam(_)) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message: format!(
                                "prefab {key:?}: has tags: [\"flycam\"] but its camera_mode is not \
                                 Flycam(...) -- the flycam spawn code already warns about this at \
                                 runtime and falls back to FlyCamDef::default(), silently \
                                 discarding every field this camera_mode actually set"
                            ),
                            error_type: "unsupported_prefab_camera_mode",
                        });
                    }
                }
            }

            // `camera_mode_vocab_problems` runs exactly once per prefab, on whichever mode the
            // runtime actually ends up consuming -- computed here rather than duplicated inside
            // both `if def.is_player()`/`if def.is_flycam()` blocks above, since those two tags are
            // NOT mutually exclusive (a real, runtime-warned authoring shape: `scene_loader.rs`'s
            // "has both \"player\" and \"flycam\" tags" warn). Two bugs a naive "call it in both
            // blocks" version would have: (1) a dual-tagged prefab with a correctly-shaped
            // `camera_mode: Flycam(...)` would get every vocab error reported TWICE; (2) a
            // dual-tagged prefab with `camera_mode: Orbit(...)` would get Orbit's fields
            // vocab-checked under the `is_player()` gate even though the flycam tag makes the
            // runtime discard this camera_mode entirely (it never reaches `orbit_state_from_config`
            // at all) -- exactly the "misleading diagnostic on a payload the runtime never
            // consumes" class the `is_player()` gate's own comment above says it exists to prevent.
            // `consumed` mirrors the runtime's own precedence: flycam-tagged wins over player-tagged
            // (`scene_loader.rs`'s `if is_flycam { ... continue; }` runs before player assembly),
            // and even then only a `Flycam(_)`-shaped mode is actually read (system-architect
            // finding).
            if let Some(mode) = &def.components.camera_mode {
                let consumed = if def.is_flycam() {
                    matches!(mode, CameraModeDef::Flycam(_))
                } else {
                    def.is_player()
                };
                if consumed {
                    for (message, error_type) in camera_mode_vocab_problems(
                        mode, &format!("prefab {key:?}: camera_mode"),
                    ) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message,
                            error_type,
                        });
                    }
                }
            }

            // Legacy `components.camera`/`components.flycam` fields carry the identical
            // vocabulary/key fields as their `camera_mode: Orbit(...)`/`Flycam(...)` successors
            // and are resolved through the exact same runtime parsers (`entity_spawner.rs`'s
            // `orbit_state_from_config`/flycam-spawn sites) -- and are, in practice, the DOMINANT
            // authoring surface: `local_coop_demo` alone authors ~14 `camera:` blocks and zero
            // `camera_mode: Orbit(...)` ones; `camera_modes`/`dynamic_animation_control`/
            // `foliage_demo` all still author `flycam: (...)`, never `camera_mode: Flycam(...)`.
            // A `camera_mode`-only check would miss almost every real shipped occurrence of this
            // vocabulary (system-architect finding). Gated the same way as their modern
            // equivalents (`camera`: player-only; `flycam`: flycam-tagged-only) -- the two legacy
            // fields can't collide with each other the way `camera_mode` collides across both
            // tags, since each is read only by its own capability.
            if def.is_player() {
                if let Some(cfg) = &def.components.camera {
                    for (message, error_type) in orbit_config_vocab_problems(
                        cfg, &format!("prefab {key:?}: camera"),
                    ) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message,
                            error_type,
                        });
                    }
                }
            }
            if def.is_flycam() {
                if let Some(fc) = &def.components.flycam {
                    for (message, error_type) in flycam_def_vocab_problems(
                        fc, &format!("prefab {key:?}: flycam"),
                    ) {
                        errors.push(CrossFileError {
                            source_file: "prefabs/prefabs.ron".to_string(),
                            message,
                            error_type,
                        });
                    }
                }
            }

            if def.kind == PrefabKind::Foliage {
                if let Some(foliage) = &def.foliage {
                    if let Some(ac) = asset_catalog {
                        let tex_key = &foliage.material.leaf_texture;
                        if !tex_key.is_empty() && !ac.textures.contains_key(tex_key) {
                            errors.push(CrossFileError {
                                source_file: "prefabs/prefabs.ron".to_string(),
                                message: format!(
                                    "prefab {:?}: foliage leaf_texture key {:?} not found in assets.ron textures",
                                    key, tex_key
                                ),
                                error_type: "missing_catalog_key",
                            });
                        }
                    }
                }
            }

            // stat_label/world_stat_bar authored with an entity-local ("{self}.<stat>") key
            // require a matching entry in this SAME prefab's stat_templates, or the widget
            // silently renders empty forever with no runtime feedback. Generic across every
            // prefab kind (players included, since a player prefab is just a prefab with
            // `tags: ["player"]`) — NPCs/props have had this exact silent-failure mode all
            // along; `player_stat_widgets` just makes it far more likely a designer hits it on
            // a player prefab for the first time (carrying over a `{self}.mana` habit onto a
            // player prefab with no matching `stat_templates` entry).
            // See `planning/features/player_stat_widgets.md` Part C.
            for (widget_kind, stat_key) in [
                ("stat_label", def.stat_label.as_ref().map(|sl| &sl.stat_key)),
                ("world_stat_bar", def.world_stat_bar.as_ref().map(|wb| &wb.stat_key)),
            ] {
                let Some(stat_key) = stat_key else { continue };
                let Some(local_stat) = stat_key.strip_prefix("{self}.") else { continue };
                if !def.stat_templates.iter().any(|t| t.key == local_stat) {
                    errors.push(CrossFileError {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: {} keyed {:?}, but this prefab's stat_templates has no \
                             entry for {:?} — the widget will render empty with no further warning",
                            key, widget_kind, stat_key, local_stat
                        ),
                        error_type: "missing_stat_widget_template",
                    });
                }
            }

            // `WorldStatBarStyle::Icon.icon_sheet`/`::Textured.texture_sheet` are both
            // `AssetCatalog.textures` keys resolved at spawn time (`stat_display.rs`) -- `Icon`
            // silently renders nothing on a miss, no runtime `warn!` at all; `Textured` DOES
            // `warn!` and skips spawning the bar entirely, so it's not silent, just still worth a
            // design-time hard error over discovering it via a runtime log line.
            if let Some(bar) = &def.world_stat_bar {
                match &bar.style {
                    WorldStatBarStyle::Icon { icon_sheet, .. } => check_texture_key(
                        asset_catalog, "prefabs/prefabs.ron",
                        &format!("prefab {key:?}: world_stat_bar Icon"), icon_sheet, &mut errors,
                    ),
                    WorldStatBarStyle::Textured { texture_sheet, .. } => check_texture_key(
                        asset_catalog, "prefabs/prefabs.ron",
                        &format!("prefab {key:?}: world_stat_bar Textured"), texture_sheet, &mut errors,
                    ),
                    WorldStatBarStyle::Ascii { .. } | WorldStatBarStyle::Pixel { .. } => {}
                }
            }
        }
    }

    // `camera_modes:` registry authoring mistakes (planning/features/camera_modes.md v2, "Named
    // mode registry" resolution) — mirrors the load-time `warn!`s in `warn_camera_modes_registry`
    // (scene_loader.rs), as the design-time counterpart.
    for (scene_path, scene) in scenes {
        for (key, mode) in &scene.camera_modes {
            if key == "default" {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "camera_modes: preset {:?} uses the reserved key \"default\" — \
                         SetCameraMode(mode: \"default\") always restores a camera's own \
                         scene-authored starting mode, never a designer-defined preset; rename \
                         this entry",
                        key
                    ),
                    error_type: "reserved_camera_mode_key",
                });
            }
            if matches!(mode, CameraModeDef::Party(_)) {
                errors.push(CrossFileError {
                    source_file: scene_path.clone(),
                    message: format!(
                        "camera_modes: preset {:?} is Party(...), which cannot be reached via \
                         SetCameraMode (no per-camera meaning when targeting a single camera) — \
                         remove it or replace it with a different mode",
                        key
                    ),
                    error_type: "unsupported_registry_camera_mode",
                });
            }
            if let CameraModeDef::Fixed(fx) = mode {
                if let Some(target_id) = &fx.look_at_entity {
                    if !scene.entities.iter().any(|e| &e.id == target_id) {
                        errors.push(CrossFileError {
                            source_file: scene_path.clone(),
                            message: format!(
                                "camera_modes: preset {:?}: Fixed's look_at_entity {:?} does not \
                                 match any entity id in this scene's entities — the camera will \
                                 silently fail to find it and hold its last known rotation \
                                 (or none, if it never resolved) every frame",
                                key, target_id
                            ),
                            error_type: "missing_reference",
                        });
                    }
                }
            }
            if let Some((message, error_type)) = camera_mode_nested_split_party_problem(
                mode,
                &format!("camera_modes: preset {key:?}"),
                true,
            ) {
                errors.push(CrossFileError { source_file: scene_path.clone(), message, error_type });
            }
            for (message, error_type) in camera_mode_vocab_problems(
                mode, &format!("camera_modes: preset {key:?}"),
            ) {
                errors.push(CrossFileError { source_file: scene_path.clone(), message, error_type });
            }
        }
    }

    // `Action::SetCameraMode(mode:)` must be either the reserved "default" or a key present in
    // SOME scene's camera_modes registry. Weaker than the project-scoped catalog checks above
    // (rules.ron/state_machine.ron are project-scoped, but camera_modes is scene-scoped, so
    // "defined in scene A, fired only while scene B is active" isn't caught) — still catches the
    // dominant failure, a typo'd key, which today only surfaces as a silent runtime warn!+no-op.
    for (source, action) in actions {
        let Action::SetCameraMode { mode, .. } = action else { continue };
        if mode == "default" {
            continue;
        }
        let found_in_any_scene = scenes.iter().any(|(_, scene)| scene.camera_modes.contains_key(mode));
        if !found_in_any_scene {
            errors.push(CrossFileError {
                source_file: source.clone(),
                message: format!(
                    "SetCameraMode: mode {:?} is not \"default\" and not found in any scene's \
                     camera_modes registry — this will silently warn+no-op at runtime",
                    mode
                ),
                error_type: "missing_reference",
            });
        }
    }

    // `Action::Spawn`'s `spawn_point` must match a key in SOME scene's `spawn_points` map. Same
    // weaker-than-project-scoped caveat as the `SetCameraMode` check above: rules.ron/
    // state_machine.ron actions are project-scoped but `spawn_points` is scene-scoped, so "defined
    // in scene A, fired only while scene B is active" isn't caught — still catches the dominant
    // failure, a typo'd spawn_point name, which today only warns and falls back to the world
    // origin at runtime (`action_executor.rs`). `at_entity` is deliberately not checked here — it
    // needs harder reachability reasoning about which entities exist when the action fires (and,
    // like `spawn_point` itself, is `{self}`/`{target}`-substituted, so a literal check on it
    // would have the same false-positive problem this check guards against below) and stays
    // deferred; see `planning/backlog.md`'s "CLI validate: no reference check for `Action::Spawn`'s
    // `spawn_point`".
    for (source, action) in actions {
        let Action::Spawn { spawn_point: Some(spawn_point), .. } = action else { continue };
        // `spawn_point` is substituted at interpret time for `{self}`/`{target}` tokens
        // (message_interpreter.rs, dialogue.rs) — see the supported-fields list in
        // `crates/ironhold_core/src/CLAUDE.md`. The authored string here is pre-substitution, so a
        // templated value (e.g. `"{self}_spawn"`, used to share one behavior rule across several
        // named spawn points) is not the literal key that will be looked up at runtime; skip it
        // rather than false-positive on a legal pattern.
        if spawn_point.contains('{') {
            continue;
        }
        let found_in_any_scene =
            scenes.iter().any(|(_, scene)| scene.spawn_points.contains_key(spawn_point));
        if !found_in_any_scene {
            errors.push(CrossFileError {
                source_file: source.clone(),
                message: format!(
                    "Spawn: spawn_point {:?} not found in any scene's spawn_points map — this \
                     will silently warn and fall back to the world origin at runtime",
                    spawn_point
                ),
                error_type: "missing_reference",
            });
        }
    }

    // `label_depth_scale.min_scale` outside the documented [0.0, 1.0] range — design-time
    // counterpart of the scene-load clamp+warn! in `scene_loader.rs::
    // warn_label_depth_scale_min_scale_out_of_range`. Without this fix, a value > 1.0 would pin
    // every depth-scaled widget in this scene at that factor forever, regardless of camera
    // distance (the engine clamps it to 1.0 instead); a negative value is inert (never binds
    // against an already-non-negative ratio) rather than doing anything useful. See
    // `planning/features/label_depth_scale_validation.md`.
    for (scene_path, scene) in scenes {
        let Some(cfg) = &scene.label_depth_scale else { continue };
        let Some(min_scale) = cfg.min_scale else { continue };
        if min_scale.is_finite() && (0.0..=1.0).contains(&min_scale) {
            continue;
        }
        let message = if !min_scale.is_finite() {
            format!(
                "label_depth_scale.min_scale is {} (not a finite number) — must be in [0.0, 1.0]",
                min_scale
            )
        } else if min_scale > 1.0 {
            format!(
                "label_depth_scale.min_scale is {} — outside [0.0, 1.0]. Without this fix, every \
                 nameplate/stat label/bar in this scene would pin at {:.0}% size forever, \
                 regardless of camera distance — the engine now clamps it to 1.0 (100%) instead",
                min_scale, min_scale * 100.0
            )
        } else {
            format!(
                "label_depth_scale.min_scale is {} — outside [0.0, 1.0]. Negative values are \
                 silently inert (no effect on scaling) rather than doing anything useful",
                min_scale
            )
        };
        errors.push(CrossFileError {
            source_file: scene_path.clone(),
            message,
            error_type: "label_depth_scale_min_scale_out_of_range",
        });
    }

    errors
}

// ── UI trigger reachability ───────────────────────────────────────────────────

/// Every event string any rule/transition/binding in the project's logic matches against —
/// `rules.ron`'s `on:`, `state_machine.ron`'s in-state `on:`/`transitions[].on`/`global_on:`,
/// and the same three fields in every behavior file. Takes the same already-parsed
/// `rules`/`state_machine`/`behaviors` `do_validate` builds for `collect_actions` above, rather
/// than re-reading the files from disk — a malformed logic file then degrades exactly like every
/// other check in this module (an incomplete `handled` set alongside the file's own already-
/// reported parse error), instead of `check_ui_trigger_reachability` silently swallowing that
/// same parse error a second time and fabricating an `unreachable_trigger` report against every
/// button in the project on top of the real error.
fn collect_handled_events(
    rules: Option<&LogicRulesAsset>,
    state_machine: Option<&StateMachineAsset>,
    behaviors: &[(String, StateMachineAsset)],
) -> HashSet<String> {
    let mut events = HashSet::new();

    if let Some(rules) = rules {
        for rule in &rules.rules {
            events.insert(rule.on.clone());
        }
    }
    if let Some(fsm) = state_machine {
        collect_fsm_events(fsm, &mut events);
    }
    for (_, fsm) in behaviors {
        collect_fsm_events(fsm, &mut events);
    }

    events
}

fn collect_fsm_events(fsm: &StateMachineAsset, events: &mut HashSet<String>) {
    for state in &fsm.states {
        for binding in &state.on {
            events.insert(binding.event.clone());
        }
    }
    for t in &fsm.transitions {
        events.insert(t.on.clone());
    }
    for binding in &fsm.global_on {
        events.insert(binding.event.clone());
    }
}

/// Existence/case checks for every raw asset-relative file path this schema authors -- see
/// `check_asset_catalog_path`'s doc comment for exactly which fields these are and why (raw
/// `asset_server.load()` paths, not `AssetCatalog` keys -- `AssetCatalog.models[key].path`,
/// `.textures[key]`, `.audio[key].path`, `.decals[key]`; every `MaterialDef` variant's nested
/// texture/shader/splatmap paths; `ProjectConfig.global_environment`'s IBL paths; and a scene's
/// `terrain.heightmap`/`.splatmap`/`.material_paths`). Extracted from `cross_file_checks` (which
/// was pushing 1,200+ lines) into its own function, matching the `check_ui_trigger_reachability`
/// precedent below -- `find_assets_root` is resolved exactly once here, rather than once per
/// path-family the way the original inline version did.
fn check_asset_root_paths(project: LoadedProject) -> Vec<CrossFileError> {
    let LoadedProject { project_dir, project_config, asset_catalog, scenes, .. } = project;
    let mut errors = Vec::new();
    let Some(assets_root) = find_assets_root(project_dir) else { return errors };

    if let Some(catalog) = asset_catalog {
        // Mirrors the `items_source` pattern elsewhere in this file: `asset_catalog` is itself a
        // configurable `ProjectConfig` field, so a project that relocates it must still get
        // diagnostics pointing at the real file, not always the convention-path literal.
        let assets_source = project_config
            .and_then(|c| c.asset_catalog.clone())
            .unwrap_or_else(|| "assets.ron".to_string());
        let mut model_keys: Vec<&String> = catalog.models.keys().collect();
        model_keys.sort();
        for key in model_keys {
            check_asset_catalog_path(
                &assets_root, &assets_source, &format!("model {key:?}"),
                &catalog.models[key].path, &mut errors,
            );
        }

        let mut texture_keys: Vec<&String> = catalog.textures.keys().collect();
        texture_keys.sort();
        for key in texture_keys {
            check_asset_catalog_path(
                &assets_root, &assets_source, &format!("texture {key:?}"),
                &catalog.textures[key], &mut errors,
            );
        }

        let mut audio_keys: Vec<&String> = catalog.audio.keys().collect();
        audio_keys.sort();
        for key in audio_keys {
            check_asset_catalog_path(
                &assets_root, &assets_source, &format!("audio {key:?}"),
                &catalog.audio[key].path, &mut errors,
            );
        }

        let mut decal_keys: Vec<&String> = catalog.decals.keys().collect();
        decal_keys.sort();
        for key in decal_keys {
            check_asset_catalog_path(
                &assets_root, &assets_source, &format!("decal {key:?}"),
                &catalog.decals[key], &mut errors,
            );
        }

        // `MaterialDef`'s own nested path fields -- these are raw `asset_server.load()` paths
        // (`material_factory.rs`), NOT `AssetCatalog.textures` keys, unlike e.g.
        // `FoliageMaterialDef.leaf_texture` or `EffectDef.sprite`, which genuinely are catalog
        // keys and are correctly left alone here.
        let mut material_keys: Vec<&String> = catalog.materials.keys().collect();
        material_keys.sort();
        for key in material_keys {
            match &catalog.materials[key].kind {
                MaterialKind::Standard(std_def) => {
                    let texture_fields = [
                        ("base_color_texture", &std_def.base_color_texture),
                        ("normal_map_texture", &std_def.normal_map_texture),
                        ("metallic_roughness_texture", &std_def.metallic_roughness_texture),
                        ("occlusion_texture", &std_def.occlusion_texture),
                        ("emissive_texture", &std_def.emissive_texture),
                    ];
                    for (field_name, path) in texture_fields {
                        let Some(path) = path else { continue };
                        check_asset_catalog_path(
                            &assets_root, &assets_source, &format!("material {key:?}.{field_name}"),
                            path, &mut errors,
                        );
                    }
                }
                MaterialKind::Terrain(terrain_def) => {
                    check_asset_catalog_path(
                        &assets_root, &assets_source, &format!("material {key:?}.splatmap"),
                        &terrain_def.splatmap, &mut errors,
                    );
                    for (i, layer) in terrain_def.layers.iter().enumerate() {
                        check_asset_catalog_path(
                            &assets_root, &assets_source, &format!("material {key:?}.layers[{i}]"),
                            layer, &mut errors,
                        );
                    }
                }
                MaterialKind::Custom(custom_def) => {
                    // Empty/absent shader path is a deliberate, already-warned-about fallback to
                    // the built-in magenta shader (`material_factory.rs`), not a missing file.
                    if let Some(shader) = &custom_def.shader {
                        if !shader.is_empty() {
                            check_asset_catalog_path(
                                &assets_root, &assets_source, &format!("material {key:?}.shader"),
                                shader, &mut errors,
                            );
                        }
                    }
                    let mut slot_keys: Vec<&String> = custom_def.textures.keys().collect();
                    slot_keys.sort();
                    for slot in slot_keys {
                        check_asset_catalog_path(
                            &assets_root, &assets_source, &format!("material {key:?}.textures[{slot:?}]"),
                            &custom_def.textures[slot], &mut errors,
                        );
                    }
                }
            }
        }
    }

    // `ProjectConfig.global_environment`'s IBL paths -- same raw-`asset_server.load()` shape as
    // the `AssetCatalog` material paths above, just project-scoped instead of catalog-scoped.
    if let Some(config) = project_config {
        if let Some(env) = &config.global_environment {
            let source_file = find_project_ron(project_dir).unwrap_or_default();
            if let Some(path) = &env.diffuse_path {
                check_asset_catalog_path(
                    &assets_root, &source_file, "global_environment.diffuse_path", path, &mut errors,
                );
            }
            if let Some(path) = &env.specular_path {
                check_asset_catalog_path(
                    &assets_root, &source_file, "global_environment.specular_path", path, &mut errors,
                );
            }
        }
    }

    // `TerrainConfigV2`'s heightmap/splatmap/material_paths -- same raw-path shape again,
    // scene-scoped this time (`GameSceneV2.terrain`).
    for (scene_path, scene) in scenes {
        let Some(terrain) = &scene.terrain else { continue };
        check_asset_catalog_path(
            &assets_root, scene_path, "terrain.heightmap", &terrain.heightmap, &mut errors,
        );
        check_asset_catalog_path(
            &assets_root, scene_path, "terrain.splatmap", &terrain.splatmap, &mut errors,
        );
        for (i, path) in terrain.material_paths.iter().enumerate() {
            check_asset_catalog_path(
                &assets_root, scene_path, &format!("terrain.material_paths[{i}]"), path, &mut errors,
            );
        }
    }

    errors
}

/// For every scene `Button`/`IconButton`, every `global_key_bindings`/`scene_key_bindings` entry,
/// and every `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings` entry, derive
/// the `ui.button_pressed:{trigger}` event it fires at runtime
/// (`scene_manager/scene_loader.rs`'s `strip_prefix("ui.")` derivation for buttons; every binding
/// map's value is used as the trigger directly, no `ui.` stripping — see `ProjectConfig.
/// global_key_bindings`'s doc comment) and confirm at least one rule/transition/binding anywhere
/// in the project's logic actually matches it. A mismatch means the button/binding is live and
/// fires a `UiEvent` that is matched against zero rules and silently dropped — "I clicked the
/// button and nothing happened" (or "I pressed the key/gamepad button and nothing happened"),
/// with no other symptom. Not gated behind `--strict`: this is "referenced but never resolves,"
/// the same severity class as every other missing-key check above, not an orphan-detection
/// question.
///
/// Deliberately not extended to dialogue choice buttons (`dialogue_choice:{n}`) — those are
/// spawned dynamically by `dialogue.rs` from `DialogueChoiceDef`, never appear as a `UiNodeDef`
/// in scene RON, and are matched directly by `dialogue_tick_system`, not through
/// `rules.ron`/`state_machine.ron`. Nothing here walks `scene.ui` for them, so there is no
/// false-positive risk from that surface.
///
/// **Known latent gap, no shipped project hits it today:** an entity `.behavior.ron`'s event
/// pattern can contain a `{self}` token, substituted against the owning entity's spawn id at
/// match time (`message_interpreter.rs`) — `collect_handled_events` stores the raw,
/// pre-substitution literal, so a behavior authored as `on: "ui.button_pressed:{self}_open"`
/// would never string-match a button's already-concrete derived event and would be wrongly
/// reported as unreachable. No shipped behavior file currently handles a `ui.button_pressed:*`
/// event, so this is theoretical; if one ever does, this check will need `{self}`-aware matching.
///
/// This function's own site enumeration is mirrored by `collect_reachable_ui_triggers` below (the
/// reverse-direction `orphan_rule` check's data source) — keep both in sync if a new UI trigger
/// site type is ever added here. The `{self}`/`dialogue_choice:` false-positive exclusions above
/// are also handled there, in `check_orphan_event`.
fn check_ui_trigger_reachability(project: LoadedProject) -> Vec<CrossFileError> {
    let LoadedProject {
        project_dir, project_config, scenes, prefab_catalog, actions, rules, state_machine,
        behaviors, logic_files_parsed_cleanly, ..
    } = project;
    // collect_handled_events/the checks below only need the parsed asset, not its source path.
    let rules = rules.map(|(_, r)| r);
    let state_machine = state_machine.map(|(_, s)| s);

    // A malformed rules.ron/state_machine.ron/behavior file already reports its own parse error
    // in the per-file results above. Treating that file's "nothing parsed" state as "this project
    // handles nothing" would flood every button/binding in the project with a derived
    // `unreachable_trigger` report piled on top of the one real root cause. Skip entirely until
    // the parse error is fixed rather than fabricate a wave of secondary noise.
    if !logic_files_parsed_cleanly {
        return Vec::new();
    }

    let mut errors = Vec::new();
    let handled = collect_handled_events(rules, state_machine, behaviors);

    // Which prefab keys are actually placed/spawnable anywhere in this project -- mirrors
    // strict_checks's own `used_prefabs` computation (scene entities, join_prefab_keys hot-join
    // slots, and Action::Spawn). Used below to scope the ShopPanel buy_item derivation to
    // merchants that could actually be opened, not every merchant in the whole catalog: unlike
    // collect_reachable_ui_triggers (where over-approximating only ever suppresses a --strict
    // orphan warning, a false negative), over-approximating here would fabricate a hard
    // unreachable_trigger error for a merchant prefab nobody has placed yet -- a real, disruptive
    // false positive at error severity, not just a suppressed warning.
    let mut used_prefabs: HashSet<&str> = HashSet::new();
    for (_, scene) in scenes {
        for entity in &scene.entities {
            used_prefabs.insert(&entity.prefab);
        }
        for entry in scene.join_prefab_keys.iter().flatten() {
            used_prefabs.insert(entry);
        }
    }
    for (_, action) in actions {
        if let Action::Spawn { prefab, .. } = action {
            used_prefabs.insert(prefab);
        }
    }

    // `verb`/`consequence` phrase the message correctly for each trigger source — a key/gamepad
    // binding is never "clicked," and saying so is actively misleading (a designer debugging a
    // broken Escape binding would go looking for a nonexistent button).
    let check = |errors: &mut Vec<CrossFileError>,
                 source: &str,
                 describe: String,
                 trigger: &str,
                 verb: &str,
                 consequence: &str| {
        if trigger.is_empty() {
            errors.push(CrossFileError {
                source_file: source.to_string(),
                message: format!("{describe} has no action configured — {consequence}"),
                error_type: "unreachable_trigger",
            });
            return;
        }
        let event = format!("ui.button_pressed:{trigger}");
        if !handled.contains(&event) {
            errors.push(CrossFileError {
                source_file: source.to_string(),
                message: format!(
                    "{describe} fires {event:?} {verb}, but no rule/transition/binding in \
                     rules.ron, state_machine.ron, or a behavior file handles it — {consequence}"
                ),
                error_type: "unreachable_trigger",
            });
        }
    };

    if let Some(config) = project_config {
        let source = find_project_ron(project_dir).unwrap_or_default();
        for (key, trigger) in &config.global_key_bindings {
            check(
                &mut errors, &source, format!("global_key_bindings[{key:?}]"), trigger,
                "when the key is pressed", "pressing this key will do nothing",
            );
        }
        for (button, trigger) in &config.global_unclaimed_gamepad_bindings {
            check(
                &mut errors, &source, format!("global_unclaimed_gamepad_bindings[{button:?}]"), trigger,
                "when the gamepad button is pressed", "pressing it will do nothing",
            );
        }
    }

    for (scene_path, scene) in scenes {
        for (key, trigger) in &scene.scene_key_bindings {
            check(
                &mut errors, scene_path, format!("scene_key_bindings[{key:?}]"), trigger,
                "when the key is pressed", "pressing this key will do nothing",
            );
        }
        for (button, trigger) in &scene.scene_unclaimed_gamepad_bindings {
            check(
                &mut errors, scene_path, format!("scene_unclaimed_gamepad_bindings[{button:?}]"), trigger,
                "when the gamepad button is pressed", "pressing it will do nothing",
            );
        }
        for node in &scene.ui {
            match node {
                UiNodeDef::Button(btn) => {
                    let trigger = btn.action.strip_prefix("ui.").unwrap_or(&btn.action);
                    check(
                        &mut errors, scene_path, format!("Button {:?}", btn.id), trigger,
                        "when clicked", "clicking it will do nothing",
                    );
                }
                UiNodeDef::IconButton(btn) => {
                    let trigger = btn.action.strip_prefix("ui.").unwrap_or(&btn.action);
                    check(
                        &mut errors, scene_path, format!("IconButton {:?}", btn.id), trigger,
                        "when clicked", "clicking it will do nothing",
                    );
                }
                // The five engine-hardcoded panel triggers (scene_loader.rs's panel-spawn sites,
                // action_executor.rs's per-MerchantDef.stock[] entry) are UiAction::Trigger
                // strings a designer never types as a Button.action -- they're emitted internally
                // by the panel's own built-in close/buy button. A project that adds one of these
                // panels and forgets the matching rule ships a panel whose close/buy button is
                // silently dead -- worse than the authored-button case above, since there's no
                // action: string in the designer's own RON to spot the mistake from. Mirrors
                // collect_reachable_ui_triggers's identical panel-presence-gated derivation
                // (the reverse check's data source) -- keep both in sync.
                UiNodeDef::InventoryPanel(panel) => {
                    check(
                        &mut errors, scene_path, format!("InventoryPanel {:?}", panel.id), "close_inventory",
                        "when its close button is clicked", "clicking it will do nothing",
                    );
                }
                UiNodeDef::ShopPanel(panel) => {
                    check(
                        &mut errors, scene_path, format!("ShopPanel {:?}", panel.id), "close_shop",
                        "when its close button is clicked", "clicking it will do nothing",
                    );
                    // Scoped to every PLACED merchant in the project (any scene entity,
                    // join_prefab_keys slot, or Action::Spawn), not the whole prefab catalog:
                    // ShopPanel is a single generic widget populated at runtime by whichever
                    // entity's Action::OpenShop names it, so which merchant's stock actually shows
                    // here isn't statically knowable, and unioning across every placed merchant is
                    // the safe over-approximation for a hard error (unlike
                    // collect_reachable_ui_triggers's whole-catalog union, which only ever
                    // suppresses a --strict warning -- reusing that same width here would
                    // fabricate a hard error for a merchant prefab nobody has placed yet).
                    // Deduplicated via a sorted set (not the raw per-stock-entry loop) so two
                    // merchants stocking the same item_key -- or the same merchant listing it
                    // twice -- produce one error, not a byte-identical repeat, and so the report
                    // order is deterministic rather than following HashMap iteration order.
                    if let Some(catalog) = prefab_catalog {
                        let item_keys: std::collections::BTreeSet<&str> = catalog.prefabs.iter()
                            .filter(|(key, _)| used_prefabs.contains(key.as_str()))
                            .filter_map(|(_, p)| p.merchant.as_ref())
                            .flat_map(|m| m.stock.iter().map(|e| e.item_key.as_str()))
                            .collect();
                        for item_key in item_keys {
                            check(
                                &mut errors, scene_path,
                                format!("ShopPanel {:?}'s buy button for item_key {:?}", panel.id, item_key),
                                &format!("buy_item:{item_key}"),
                                "when clicked", "buying it will do nothing",
                            );
                        }
                    }
                }
                UiNodeDef::ContainerPanel(panel) => {
                    check(
                        &mut errors, scene_path, format!("ContainerPanel {:?}", panel.id), "close_container",
                        "when its close button is clicked", "clicking it will do nothing",
                    );
                    check(
                        &mut errors, scene_path, format!("ContainerPanel {:?}", panel.id), "take_all_from_container",
                        "when its take-all button is clicked", "clicking it will do nothing",
                    );
                }
                _ => {}
            }
        }
    }

    errors
}

/// Every `ui.button_pressed:{trigger}` event derivable from this project's buttons/key/gamepad
/// bindings, unioned across all scenes — mirrors `check_ui_trigger_reachability`'s own site
/// enumeration exactly (global/scene key bindings, global/scene unclaimed gamepad bindings,
/// scene `Button`/`IconButton` nodes, and its `InventoryPanel`/`ShopPanel`/`ContainerPanel` match
/// arms below); keep both in sync if a new UI trigger site type is ever added. Feeds
/// `check_orphan_ui_rules` below — the same "same two data sets" backing both directions of the
/// reachability question (forward: does a button/binding's fire resolve to a handled rule;
/// reverse: does a rule's `on:` resolve to some button/binding that can fire it).
///
/// Also includes the five engine-hardcoded panel triggers (`close_inventory`/`close_shop`/
/// `close_container`/`take_all_from_container`/`buy_item:{item_key}`, `scene_loader.rs`'s
/// panel-spawn sites and `action_executor.rs`'s per-`MerchantDef.stock[]` entry) — a designer
/// never authors these as a `Button.action` string, they're emitted internally whenever a
/// panel's own built-in close/buy button is clicked, so they'd otherwise false-positive as
/// orphaned every time `check_orphan_ui_rules` sees the (correct, live) state-machine rule that
/// handles one — confirmed against `3rd_person_game_demo`'s real `ShopPanel`/`InventoryPanel`/
/// `ContainerPanel` usage before this was added. The *forward* direction
/// (`check_ui_trigger_reachability`'s own panel match arms, above) now also covers these five,
/// checking the opposite question: does a scene with one of these panels have a matching rule?
fn collect_reachable_ui_triggers(
    project_config: Option<&ProjectConfig>,
    scenes: &[(String, GameSceneV2)],
    prefab_catalog: Option<&PrefabCatalog>,
) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let mut insert = |trigger: &str| {
        if !trigger.is_empty() {
            reachable.insert(format!("ui.button_pressed:{trigger}"));
        }
    };

    if let Some(config) = project_config {
        for trigger in config.global_key_bindings.values() {
            insert(trigger);
        }
        for trigger in config.global_unclaimed_gamepad_bindings.values() {
            insert(trigger);
        }
    }
    for (_, scene) in scenes {
        for trigger in scene.scene_key_bindings.values() {
            insert(trigger);
        }
        for trigger in scene.scene_unclaimed_gamepad_bindings.values() {
            insert(trigger);
        }
        for node in &scene.ui {
            match node {
                UiNodeDef::Button(btn) => insert(btn.action.strip_prefix("ui.").unwrap_or(&btn.action)),
                UiNodeDef::IconButton(btn) => insert(btn.action.strip_prefix("ui.").unwrap_or(&btn.action)),
                UiNodeDef::InventoryPanel(_) => insert("close_inventory"),
                UiNodeDef::ShopPanel(_) => {
                    insert("close_shop");
                    if let Some(catalog) = prefab_catalog {
                        for prefab in catalog.prefabs.values() {
                            if let Some(merchant) = &prefab.merchant {
                                for entry in &merchant.stock {
                                    insert(&format!("buy_item:{}", entry.item_key));
                                }
                            }
                        }
                    }
                }
                UiNodeDef::ContainerPanel(_) => {
                    insert("close_container");
                    insert("take_all_from_container");
                }
                _ => {}
            }
        }
    }
    reachable
}

/// `--strict` reverse of `check_ui_trigger_reachability` above: a rule/transition/binding whose
/// `on:`/`event:` matches the `ui.button_pressed:{trigger}` shape but no button/key/gamepad
/// binding anywhere in the project can ever produce that exact trigger — dead code left over from
/// a scene rewrite (a renamed/removed button, a rule nobody wired up). Only ever inspects
/// `ui.button_pressed:*`-shaped strings; every other event shape (`scene.ready:*`,
/// `entity.entered:*`, a custom `EmitEvent` name, dialogue events, etc.) has no button/binding
/// origin at all and is out of scope for this check, not merely unreachable-by-this-analysis.
/// Same "union across all scenes, not project-scoped" approximation as `collect_reachable_ui_triggers`,
/// and the same scene-parse-failure blind spot already accepted for `SetCameraMode`/`spawn_point`
/// elsewhere in this file (a scene that fails to parse silently drops its buttons from the
/// reachable set, which could make an otherwise-live rule look orphaned) — not fixed here, see
/// `planning/claude_suggestions.md`.
fn check_orphan_ui_rules(
    reachable: &HashSet<String>,
    rules: Option<(&str, &LogicRulesAsset)>,
    state_machine: Option<(&str, &StateMachineAsset)>,
    behaviors: &[(String, StateMachineAsset)],
) -> Vec<StrictWarning> {
    let mut warnings = Vec::new();
    if let Some((src, r)) = rules {
        for rule in &r.rules {
            check_orphan_event(&mut warnings, reachable, src, format!("rule handling {:?}", rule.on), &rule.on);
        }
    }
    if let Some((src, fsm)) = state_machine {
        check_fsm_orphans(&mut warnings, reachable, src, fsm);
    }
    for (path, fsm) in behaviors {
        check_fsm_orphans(&mut warnings, reachable, path, fsm);
    }
    warnings
}

fn check_fsm_orphans(
    warnings: &mut Vec<StrictWarning>,
    reachable: &HashSet<String>,
    source: &str,
    fsm: &StateMachineAsset,
) {
    for state in &fsm.states {
        for binding in &state.on {
            check_orphan_event(
                warnings, reachable, source,
                format!("state {:?}'s on: binding for {:?}", state.name, binding.event),
                &binding.event,
            );
        }
    }
    for t in &fsm.transitions {
        check_orphan_event(
            warnings, reachable, source,
            format!("transition to {:?} on {:?}", t.to, t.on),
            &t.on,
        );
    }
    for binding in &fsm.global_on {
        check_orphan_event(
            warnings, reachable, source,
            format!("global_on binding for {:?}", binding.event),
            &binding.event,
        );
    }
}

fn check_orphan_event(
    warnings: &mut Vec<StrictWarning>,
    reachable: &HashSet<String>,
    source: &str,
    describe: String,
    event: &str,
) {
    if !event.starts_with("ui.button_pressed:") {
        return;
    }
    // `dialogue_choice:{n}` is a sixth engine-emitted UiAction::Trigger source (dialogue.rs's
    // choice buttons), spawned dynamically and never appearing as a UiNodeDef in scene RON --
    // `collect_reachable_ui_triggers` has no way to enumerate it (there's no fixed set of choice
    // indices; a dialogue can have any number of nodes/choices), so a rule correctly handling one
    // would otherwise always look orphaned. Skip rather than false-positive on a working project.
    if event.starts_with("ui.button_pressed:dialogue_choice:") {
        return;
    }
    // Same false-positive class every other string-key check in this file already guards against
    // (e.g. stat_label's "{self}." skip): a behavior file's on:/event: pattern can contain a
    // `{self}` token, substituted against the owning entity's spawn id at match time
    // (message_interpreter.rs) -- the raw, pre-substitution literal stored here would never
    // string-match the button's already-concrete derived event.
    if event.contains('{') {
        return;
    }
    if reachable.contains(event) {
        return;
    }
    warnings.push(StrictWarning {
        source_file: source.to_string(),
        message: format!(
            "{describe} — no button/key/gamepad binding anywhere in the project can ever fire \
             {event:?}. Dead code, or a stale event name left over from a rename/removal."
        ),
        kind: "orphan_rule",
    });
}

// ── Strict (orphan) checks ────────────────────────────────────────────────────

fn strict_checks(project: LoadedProject) -> Vec<StrictWarning> {
    let LoadedProject {
        project_dir, project_config, asset_catalog, prefab_catalog, scenes, dialogues, actions,
        rules, state_machine, behaviors, logic_files_parsed_cleanly, scenes_parsed_cleanly,
        ..
    } = project;
    let orphan_rule_prereqs_clean = logic_files_parsed_cleanly && scenes_parsed_cleanly;
    let mut warnings: Vec<StrictWarning> = Vec::new();

    // `load_configured_catalog` falls back to checking a catalog's convention-path file whenever
    // its ProjectConfig field is unset (see that function's doc comment for why) -- deliberately
    // diverging from the runtime, which loads nothing at all for an unset field. That divergence
    // is silent by design at the always-on error level (it exists specifically so a project with
    // no config, or a fixture, still gets checked), but it's a real, reportable authoring mistake
    // when it happens in a REAL project: a convention-path file left on disk without its matching
    // field set validates clean here while the runtime silently loads an empty/absent catalog.
    // Only fires when a .project.ron actually exists — a config-less project (every check above
    // this comment already treats that as the normal, expected fixture/bootstrap shape, not a
    // mistake) would otherwise light this up on every one of its convention-path files.
    if let Some(config) = project_config {
        for (field, convention_path, field_name) in [
            (config.asset_catalog.as_deref(), "assets.ron", "asset_catalog"),
            (config.prefab_catalog.as_deref(), "prefabs/prefabs.ron", "prefab_catalog"),
            (config.stats_path.as_deref(), "stats/stats.ron", "stats_path"),
            (config.items_path.as_deref(), "items/items.ron", "items_path"),
            (config.model_fixes_path.as_deref(), "overrides/model_fixes.ron", "model_fixes_path"),
        ] {
            if field.is_none() && project_dir.join(convention_path).is_file() {
                warnings.push(StrictWarning {
                    source_file: find_project_ron(project_dir).unwrap_or_default(),
                    message: format!(
                        "{convention_path} exists but {field_name} is not set in .project.ron — \
                         the runtime will not load it, even though this validate run just checked \
                         it via the convention-path fallback"
                    ),
                    kind: "unset_catalog_path_with_convention_file",
                });
            }
        }
    }

    // Unlike the catalog paths above, `rules_path`/`state_machine_path` have NO convention-path
    // fallback at all once a `.project.ron` exists (`resolve_logic_files`) -- a
    // `logic/rules.ron`/`logic/state_machine.ron` left on disk with its matching field unset is
    // not just unloaded by the runtime, it's not even parsed or cross-checked by THIS validate
    // run either (unlike the catalog case, whose message can truthfully say "checked via the
    // convention-path fallback"). Without this warning such a file gets zero signal from any
    // tool: this is exactly what happened to `3rd_person_game_demo`/`terrain_demo`'s dead
    // `logic/rules.ron` files when `resolve_logic_files` closed the false-orphan-warning bug they
    // used to (incorrectly) surface via `orphan_rule` -- that was a real bug fix, but it also
    // silently deleted the only signal pointing at those files, which this restores correctly.
    if let Some(config) = project_config {
        for (field, convention_path, field_name) in [
            (config.rules_path.as_deref(), "logic/rules.ron", "rules_path"),
            (config.state_machine_path.as_deref(), "logic/state_machine.ron", "state_machine_path"),
        ] {
            if field.is_none() && project_dir.join(convention_path).is_file() {
                warnings.push(StrictWarning {
                    source_file: find_project_ron(project_dir).unwrap_or_default(),
                    message: format!(
                        "{convention_path} exists but {field_name} is not set in .project.ron — \
                         the runtime never loads it, and this validate run never parsed or \
                         cross-checked it either; delete the file if it's leftover, or set \
                         {field_name} if it's meant to be live"
                    ),
                    kind: "unset_logic_path_with_convention_file",
                });
            }
        }
    }

    // A `Fixed(...)` camera_mode with both look_at/look_at_entity set, or neither, is working
    // (not broken) either way -- see `camera_mode_fixed_look_at_problem`'s doc comment -- so this
    // is `--strict` advisory, unlike the always-on `camera_mode_nested_split_party` check in
    // `cross_file_checks`. Same player-tagged gate as that check, for the same reason (the
    // runtime never reads `camera_mode` on an untagged or flycam-tagged prefab).
    if let Some(catalog) = prefab_catalog {
        let mut keys: Vec<&String> = catalog.prefabs.keys().collect();
        keys.sort();
        for key in keys {
            let def = &catalog.prefabs[key];
            if !def.is_player() {
                continue;
            }
            let Some(mode) = &def.components.camera_mode else { continue };
            if let Some((message, kind)) =
                camera_mode_fixed_look_at_problem(mode, &format!("prefab {key:?}: camera_mode"))
            {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message,
                    kind,
                });
            }
        }
    }
    for (scene_path, scene) in scenes {
        let mut keys: Vec<&String> = scene.camera_modes.keys().collect();
        keys.sort();
        for key in keys {
            let mode = &scene.camera_modes[key];
            if let Some((message, kind)) =
                camera_mode_fixed_look_at_problem(mode, &format!("camera_modes: preset {key:?}"))
            {
                warnings.push(StrictWarning { source_file: scene_path.clone(), message, kind });
            }
        }
    }

    // Collect every key that appears on the "usage" side.
    let mut used_prefabs: HashSet<&str> = HashSet::new();
    let mut used_effects: HashSet<&str> = HashSet::new();
    let mut used_audio: HashSet<&str> = HashSet::new();
    let mut used_decals: HashSet<&str> = HashSet::new();

    for (_, scene) in scenes {
        for entity in &scene.entities {
            used_prefabs.insert(&entity.prefab);
        }
        for entry in scene.join_prefab_keys.iter().flatten() {
            used_prefabs.insert(entry);
        }
        // `target_indicator.texture` is a decal reference just like `Action::ProjectDecal`'s
        // `key` -- omitting it here would make a project using the indicator get a false
        // "decal defined but never used" warning below (debug-detective finding, reproduced
        // live against `local_coop_demo` and `3rd_person_game_demo`, both of which reference
        // `target_ring` this way).
        if let Some(indicator) = &scene.target_indicator {
            used_decals.insert(&indicator.texture);
        }
    }
    for (_, action) in actions {
        match action {
            Action::Spawn { prefab, .. } => { used_prefabs.insert(prefab); }
            Action::PreloadPrefab(key) => { used_prefabs.insert(key); }
            Action::SpawnEffect { key, .. } => { used_effects.insert(key); }
            Action::PlaySound { key, .. } | Action::PlayMusicLoop { key, .. } => {
                used_audio.insert(key);
            }
            Action::ProjectDecal { key, .. } => { used_decals.insert(key); }
            _ => {}
        }
    }

    // Report defined-but-never-used keys.
    if let Some(catalog) = prefab_catalog {
        let mut keys: Vec<&String> = catalog.prefabs.keys().collect();
        keys.sort();
        for key in keys {
            if !used_prefabs.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab {:?} is defined but never referenced in any scene or action",
                        key
                    ),
                    kind: "unused_prefab",
                });
            }
        }

        // A player prefab's jump can never ballistically clear its own ground-detection
        // sensor's combined reach (collider_radius + ground_cast_length) — the design-time
        // counterpart of the scene-load `warn!` in `scene_loader.rs::
        // warn_jump_cannot_clear_ground_sensor`. Not slope-specific: even on flat ground the
        // ground-check can then never truthfully report "ungrounded", so the jump only re-arms
        // via the bounded jump_air_grace fallback rather than a real landing — a real, working
        // fallback, not a broken feature, which is why this is a `--strict`-only warning rather
        // than a hard error (matching the runtime side, which is also a `warn!`, not a panic or
        // rejected spawn). See `planning/features/uphill_jump_lock.md`. Resolves the jump-height
        // target directly (mirroring `resolve_jump_velocity`'s height resolution) rather than
        // round-tripping through velocity — `apex == height` by construction when the runtime
        // derives velocity from a target height via `v = sqrt(2*g*h)`.
        let mut keys: Vec<&String> = catalog.prefabs.keys().collect();
        keys.sort();
        for key in keys {
            let def = &catalog.prefabs[key];
            if !def.is_player() { continue }
            let player_height = if def.kind == PrefabKind::Primitive {
                def.primitive.as_ref().and_then(|p| p.height).unwrap_or(1.8)
            } else {
                def.components.movement.collider_height.unwrap_or(1.8)
            };
            let collider_radius = if def.kind == PrefabKind::Primitive {
                def.primitive.as_ref().and_then(|p| p.radius).unwrap_or(0.4)
            } else {
                def.components.movement.collider_radius.unwrap_or(0.4)
            };
            let reach = collider_radius + def.components.movement.ground_cast_length;
            let resolve_height = |config: Option<&ironhold_core::schema::catalog::JumpConfig>| -> f32 {
                use ironhold_core::schema::catalog::JumpConfig;
                match config {
                    None => player_height,
                    Some(JumpConfig::Fixed { height }) => *height,
                    Some(JumpConfig::RelativeToHeight { percent }) => player_height * percent / 100.0,
                }
            };
            let jump_apex = resolve_height(def.components.movement.jump.as_ref());
            let mut checks = vec![("jump", jump_apex)];
            if def.components.movement.double_jump {
                checks.push(("double_jump_height", resolve_height(def.components.movement.double_jump_height.as_ref())));
            }
            for (field_name, apex) in checks {
                // `!(apex > reach)`, not `apex <= reach`: a negative/zero authored height (or a
                // negative `RelativeToHeight` percent) makes the resolved velocity NaN at
                // runtime, and `NaN <= reach` is false — silently missing the exact
                // misconfiguration this check exists to catch.
                if !(apex > reach) {
                    warnings.push(StrictWarning {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: `{}` gives a jump apex of {:.2}m, which does not \
                             clear this player's ground-check reach of {:.2}m (collider_radius \
                             {:.2}m + ground_cast_length {:.2}m) — the ground sensor may never \
                             report \"ungrounded\" even on flat ground. Raise `{}` (or \
                             `double_jump_height`) or lower `ground_cast_length`",
                            key, field_name, apex, reach, collider_radius,
                            def.components.movement.ground_cast_length, field_name
                        ),
                        kind: "jump_cannot_clear_ground_sensor",
                    });
                }
            }

            // `max_walkable_slope_deg` outside a sane range silently breaks grounding entirely —
            // a value at or below 0 means no surface is ever walkable, and a player can then only
            // jump if `double_jump` is enabled (the grounded branch of `can_jump` never applies).
            // A value above 90 makes every surface (however overhanging) count as walkable, which
            // is likely not intended either. `90.0` itself is meaningful and valid — it's the
            // "disable this check, fall back to proximity-only grounding" escape hatch, matching
            // this project's pre-fix behavior — so the valid range is `(0.0, 90.0]`, not `(0.0,
            // 90.0)`. See `MovementConfig.max_walkable_slope_deg`'s doc comment.
            let slope_limit = def.components.movement.max_walkable_slope_deg;
            if !(slope_limit > 0.0 && slope_limit <= 90.0) {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab {:?}: `max_walkable_slope_deg` is {:.2}, outside the valid \
                         (0, 90] range — a value at or below 0 means no surface is ever walkable \
                         (jump only works via double_jump, if enabled at all); above 90 makes \
                         every surface, however overhanging, count as walkable",
                        key, slope_limit
                    ),
                    kind: "invalid_walkable_slope_limit",
                });
            }

            // Unlike `max_walkable_slope_deg`, `coyote_time_secs` has no invalid range that breaks
            // grounding outright — any non-negative value just makes the debounce buffer bigger or
            // smaller. It does have a practical, jump-height-dependent upper bound where a large
            // enough value can mask an entire jump's animation — checked separately below. A
            // negative value (or NaN, which `coyote_ticks()`'s `f32::max` clamp also silently
            // launders to `0.0`) is the one case worth flagging unconditionally here: it silently
            // launders to a zero-tick buffer (same as `0.0`) rather than doing anything with the
            // value, which is far more likely a sign-flip typo than an intentional way to spell
            // "disabled". `!(coyote_time_secs >= 0.0)`, not `coyote_time_secs < 0.0`: the latter is
            // false for NaN, silently missing it (debug-detective finding).
            let coyote_time_secs = def.components.movement.coyote_time_secs;
            if !(coyote_time_secs >= 0.0) {
                warnings.push(StrictWarning {
                    source_file: "prefabs/prefabs.ron".to_string(),
                    message: format!(
                        "prefab {:?}: `coyote_time_secs` is {:.3}, which is negative (or NaN) — \
                         this silently disables the coyote-time buffer entirely (same as `0.0`) \
                         rather than doing anything with the value. If you meant to disable it, \
                         use `0.0` instead",
                        key, coyote_time_secs
                    ),
                    kind: "negative_coyote_time_secs",
                });
            }

            // `coyote_time_secs` also has a real, jump-height-dependent upper bound: once it's
            // large enough relative to the time the ground sensor actually reports "ungrounded"
            // during the jump, the buffer can mask that whole window, suppressing the airborne
            // animation and `jump_exit` clip entirely. That window is NOT simply the jump's own
            // airtime (`2 * jump_velocity / GRAVITY`, the ballistic time from launch to landing at
            // ground level) -- the sensor keeps reporting "grounded" (per the sibling
            // `jump_cannot_clear_ground_sensor` check above) for as long as the player's height is
            // at or below `reach`, both on the way up and the way down, which shrinks the real
            // ungrounded window relative to the naive airtime formula by up to ~35% at realistic
            // apex/reach ratios (debug-detective finding, `cli_validate_small_wins` review, using
            // the same kinematic derivation with `(jump_apex - reach)` as the effective height
            // instead of `jump_apex`). `GRAVITY` itself is `pub(crate)` inside `ironhold_core`
            // (deliberately, so `capabilities/player.rs`'s jump-grace window and
            // `scene_loader.rs`'s velocity resolution can't drift apart) and not reachable from
            // this crate, so it's mirrored here as a plain constant — keep this in sync with
            // `scene_manager/scene_loader.rs::GRAVITY` if that ever changes. Guarded on
            // `jump_apex > reach`, not just `jump_apex > 0.0`: a non-positive/NaN apex, or one
            // that doesn't clear `reach` at all, is already flagged by the
            // `jump_cannot_clear_ground_sensor` check above on its own (clearer) terms — this
            // check would otherwise compare against a meaningless or double-reported window.
            const GRAVITY: f32 = 9.81;
            if jump_apex > reach {
                let effective_height = jump_apex - reach;
                let velocity_above_reach = (2.0 * GRAVITY * effective_height).sqrt();
                let airtime = 2.0 * velocity_above_reach / GRAVITY;
                // `!(coyote_time_secs <= airtime)`, not `coyote_time_secs > airtime`: the latter
                // is false for a NaN `coyote_time_secs`, silently missing it (debug-detective
                // finding, same reasoning as the negative-value check above).
                if !(coyote_time_secs <= airtime) {
                    warnings.push(StrictWarning {
                        source_file: "prefabs/prefabs.ron".to_string(),
                        message: format!(
                            "prefab {:?}: `coyote_time_secs` is {:.3}s, longer than the {:.3}s \
                             this player's ground sensor actually reports \"ungrounded\" during a \
                             `jump` (jump apex {:.2}m, ground-check reach {:.2}m) — the \
                             coyote-time buffer can mask that whole window, suppressing the \
                             airborne animation and jump_exit clip. Lower `coyote_time_secs` \
                             below that, or raise `jump` (this check compares against `jump` \
                             specifically, not `double_jump_height`)",
                            key, coyote_time_secs, airtime, jump_apex, reach
                        ),
                        kind: "coyote_time_exceeds_jump_airtime",
                    });
                }
            }
        }
    }

    // The embedded UI font has no glyph for any non-ASCII character at all (its cmap covers only
    // U+0020..U+007E) -- such a character renders as a tofu box rather than the intended
    // punctuation/letter. `--strict`-only: the rest of the text otherwise displays correctly,
    // just missing one glyph, so this is an authoring-hygiene lint rather than a load-time
    // regression. See `find_unrenderable_char`'s doc comment for the originating incident and the
    // cmap-enumeration finding behind this check's actual (non-dash-only) scope.
    for (scene_path, scene) in scenes {
        let check_text = |kind: &str, id: &str, text: &str, warnings: &mut Vec<StrictWarning>| {
            let Some((ch, name)) = find_unrenderable_char(text) else { return };
            let article = if matches!(name.chars().next(), Some('a' | 'e' | 'i' | 'o' | 'u')) {
                "an"
            } else {
                "a"
            };
            warnings.push(StrictWarning {
                source_file: scene_path.clone(),
                message: format!(
                    "{kind} {id:?}: `text` contains {article} {name} ({ch:?}, U+{:04X}) — the \
                     embedded UI font has no glyph for it and renders a tofu box instead. \
                     Replace it with plain ASCII",
                    ch as u32
                ),
                kind: "non_ascii_char_in_text",
            });
        };
        for node in &scene.ui {
            let (kind, id, text) = match node {
                UiNodeDef::Label(l) => ("Label", l.id.as_str(), l.text.as_str()),
                UiNodeDef::Button(b) => ("Button", b.id.as_str(), b.text.as_str()),
                _ => continue,
            };
            check_text(kind, id, text, &mut warnings);
        }
        for entity in &scene.entities {
            let Some(label) = &entity.label else { continue };
            check_text("EntityLabel", &entity.id, &label.text, &mut warnings);
        }
        for wl in &scene.world_labels {
            check_text("WorldLabel", &wl.id, &wl.text, &mut warnings);
        }
    }

    // Same non-ASCII-glyph-coverage check as above, extended to dialogue prose -- the
    // highest-risk text surface for this lint, since `speaker`/`body`/choice `label` are free-form
    // narrative text a designer is far more likely to paste from a word processor (curly quotes,
    // em-dashes) than a short UI label (debug-detective finding, `cli_validate_small_wins` review).
    for (dialogue_path, dialogue) in dialogues {
        let check_dialogue_text = |kind: &str, id: &str, text: &str, warnings: &mut Vec<StrictWarning>| {
            let Some((ch, name)) = find_unrenderable_char(text) else { return };
            let article = if matches!(name.chars().next(), Some('a' | 'e' | 'i' | 'o' | 'u')) {
                "an"
            } else {
                "a"
            };
            warnings.push(StrictWarning {
                source_file: dialogue_path.clone(),
                message: format!(
                    "{kind} {id:?}: `text` contains {article} {name} ({ch:?}, U+{:04X}) — the \
                     embedded UI font has no glyph for it and renders a tofu box instead. \
                     Replace it with plain ASCII",
                    ch as u32
                ),
                kind: "non_ascii_char_in_text",
            });
        };
        for node in &dialogue.nodes {
            check_dialogue_text("DialogueNode.speaker", &node.id, &node.speaker, &mut warnings);
            check_dialogue_text("DialogueNode.body", &node.id, &node.body, &mut warnings);
            for (i, choice) in node.choices.iter().enumerate() {
                check_dialogue_text(
                    "DialogueChoice.label",
                    &format!("{}[{i}]", node.id),
                    &choice.label,
                    &mut warnings,
                );
            }
        }
    }

    // Two or more player-tagged prefabs instantiated in the same scene's `entities:` list sharing
    // the same `player_index` -- including the common case where both simply omit it, since it
    // defaults to `0` -- will show the identical "P{n}" HUD label/color, and collide on the same
    // reserved target-indicator ring `RenderLayers` slot under `own_viewport_only`
    // (`ring_layer_for_player`, `capabilities/camera.rs`). Two runtime `warn!`s already exist for
    // parts of this (`entity_spawner.rs::spawn_players_and_camera`: one for 2+ players sharing
    // `player_index: 0` specifically, one for the `own_viewport_only` ring-layer collision), but
    // this is the first design-time signal, and the only one covering a non-zero duplicate pair.
    // `--strict`-only: nothing crashes, matching this project's severity convention for a
    // cosmetic-but-confusing HUD/ring collision rather than a load failure (and matching those
    // runtime `warn!`s' own severity).
    //
    // Deliberately `entities:`-only, unlike the sibling `duplicate_gamepad_index` check above:
    // `Action::JoinPlayer` (`action_executor.rs`) unconditionally overwrites a hot-joined player's
    // `player_index` with the runtime-computed join slot (`player_config.player_index =
    // next_slot`) -- the join prefab's own authored `player_index` is never actually used, unlike
    // `gamepad_index`, which the join path genuinely does read from the prefab (except when a
    // gamepad-triggered join captures the pad directly). Checking `join_prefab_keys` here would
    // false-positive on the natural, working authoring pattern of reusing one prefab (or two
    // prefabs sharing a `player_index`) across multiple join slots (alignment-reviewer finding).
    if let Some(catalog) = prefab_catalog {
        for (scene_path, scene) in scenes {
            let mut seen: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            for entity_def in &scene.entities {
                let Some(prefab) = catalog.prefabs.get(&entity_def.prefab) else { continue };
                if !prefab.is_player() { continue }
                let index = prefab.player_index;
                let id = entity_def.id.clone();
                let prev = first_seen(&mut seen, index, id.clone());
                if let Some(other_id) = prev {
                    warnings.push(StrictWarning {
                        source_file: scene_path.clone(),
                        message: format!(
                            "players {:?} and {:?} both have player_index: {} — they will show \
                             the identical \"P{}\" HUD label/color, and (under \
                             `SplitScreenDef.own_viewport_only`) collide on the same \
                             target-indicator ring layer. Assign each player a unique \
                             player_index (it defaults to 0 when omitted)",
                            // `saturating_add`, not `+1`: player_index is designer-authored and
                            // unbounded (u32::MAX is a valid, if absurd, RON value) -- a plain
                            // `+1` panics on overflow in a debug build and wraps to a wrong
                            // number in a release one (debug-detective finding).
                            other_id, id, index, index.saturating_add(1)
                        ),
                        kind: "duplicate_player_index",
                    });
                }
            }
        }
    }

    // `label_depth_scale.reference_distance` far outside the scene's reachable player-camera
    // radius range — design-time counterpart of the scene-load `warn!` in `scene_loader.rs::
    // warn_label_depth_scale_reference_distance`. `--strict`-only (not a hard error, unlike
    // `min_scale` above): this is a heuristic band, not a provable misconfiguration — the CLI
    // can't prove scaling never engages, only that it's unlikely to at any camera distance this
    // scene's cameras can actually reach. See `planning/features/label_depth_scale_validation.md`.
    for (scene_path, scene) in scenes {
        let Some(cfg) = &scene.label_depth_scale else { continue };

        let mut overall_min = f32::INFINITY;
        let mut overall_max = f32::NEG_INFINITY;
        let mut widen = |range: Option<(f32, f32)>| {
            if let Some((min_r, max_r)) = range {
                overall_min = overall_min.min(min_r);
                overall_max = overall_max.max(max_r);
            }
        };

        // A `tags: ["flycam"]` entity suppresses every player camera entirely
        // (`SuppressPlayerCameras`, spectator mode) — a scene combining a flycam with
        // `label_depth_scale` (e.g. `custom_materials`) has no player camera to compare
        // `reference_distance` against, so player/join_prefab_keys collection is skipped when one
        // is present. Mirrors the runtime's identical `has_flycam` guard in `scene_loader.rs`.
        let has_flycam = prefab_catalog.is_some_and(|catalog| {
            scene.entities.iter().any(|e| {
                catalog.prefabs.get(&e.prefab).is_some_and(|p| p.is_flycam())
            })
        });
        if let (Some(catalog), false) = (prefab_catalog, has_flycam) {
            let mut widen_prefab_if_player = |prefab_key: &str| {
                let Some(prefab) = catalog.prefabs.get(prefab_key) else { return };
                if !prefab.is_player() { return }
                match &prefab.components.camera_mode {
                    Some(mode) => widen(mode.radius_range()),
                    None => {
                        let c = prefab.components.camera.clone().unwrap_or_else(default_camera_config);
                        widen(Some((c.min_radius, c.max_radius)));
                    }
                }
            };
            for entity_def in &scene.entities {
                widen_prefab_if_player(&entity_def.prefab);
            }
            // Local-coop character-select variants (`join_prefab_keys`) are player-tagged
            // prefabs too, reachable independently of `scene.entities` — omitting them would
            // narrow the band and risk a false positive, the opposite direction of every other
            // tradeoff this check makes.
            for key in scene.join_prefab_keys.iter().flatten() {
                widen_prefab_if_player(key);
            }
            // A player is frequently *not* scene-placed — `3rd_person_game_demo`'s own player is
            // spawned entirely via `Action::Spawn` in `state_machine.ron`'s entry_actions, never
            // appearing in `scene.entities` at all. Without this, the flagship scene this feature
            // was written to protect would silently never trigger the check. Project-wide, not
            // scene-scoped (an action isn't reliably attributable to one scene file) — same
            // documented tradeoff as the `SetCameraMode` registry check above: weaker than a
            // scene-scoped check, but catches the dominant case, and erring toward more reachable
            // cameras only ever widens the acceptable band (fewer false positives), never narrows it.
            for (_, action) in actions {
                if let Action::Spawn { prefab, .. } = action {
                    widen_prefab_if_player(prefab);
                }
            }
        }
        for mode in scene.camera_modes.values() {
            widen(mode.radius_range());
        }

        if !overall_min.is_finite() || !overall_max.is_finite() {
            // No radius-bearing camera reachable from this scene (every camera is
            // Fixed/FirstPerson/Flycam, there's a flycam suppressing player cameras, or there
            // are no player prefabs at all) — no meaningful range to compare against; a false
            // warning here would be worse than no check.
            continue;
        }
        let rd = cfg.reference_distance;
        if !rd.is_finite() {
            warnings.push(StrictWarning {
                source_file: scene_path.clone(),
                message: format!(
                    "label_depth_scale.reference_distance is {} (not a finite number) — depth \
                     scaling will never engage",
                    rd
                ),
                kind: "label_depth_scale_reference_distance_outside_camera_range",
            });
        } else if rd < overall_min * 0.5 || rd > overall_max * 2.0 {
            let suggested = (overall_min + overall_max) / 2.0;
            warnings.push(StrictWarning {
                source_file: scene_path.clone(),
                message: format!(
                    "label_depth_scale.reference_distance is {:.1}, outside this scene's typical \
                     camera zoom range ({:.1}-{:.1}) — depth scaling may never visibly engage, or \
                     may engage immediately at max zoom-out. Try ~{:.1} (the range midpoint), \
                     then confirm in-browser",
                    rd, overall_min, overall_max, suggested
                ),
                kind: "label_depth_scale_reference_distance_outside_camera_range",
            });
        }
    }

    if let Some(catalog) = asset_catalog {
        let mut effect_keys: Vec<&String> = catalog.effects.keys().collect();
        effect_keys.sort();
        for key in effect_keys {
            if !used_effects.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "assets.ron".to_string(),
                    message: format!(
                        "effect {:?} is defined but never used in any SpawnEffect action",
                        key
                    ),
                    kind: "unused_effect",
                });
            }
        }
        let mut audio_keys: Vec<&String> = catalog.audio.keys().collect();
        audio_keys.sort();
        for key in audio_keys {
            if !used_audio.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "assets.ron".to_string(),
                    message: format!(
                        "audio {:?} is defined but never used in any PlaySound or PlayMusicLoop action",
                        key
                    ),
                    kind: "unused_audio",
                });
            }
        }
        let mut decal_keys: Vec<&String> = catalog.decals.keys().collect();
        decal_keys.sort();
        for key in decal_keys {
            if !used_decals.contains(key.as_str()) {
                warnings.push(StrictWarning {
                    source_file: "assets.ron".to_string(),
                    message: format!(
                        "decal {:?} is defined but never used in any ProjectDecal action or \
                         scene's target_indicator.texture",
                        key
                    ),
                    kind: "unused_decal",
                });
            }
        }
    }

    // Parse-failure protection, both directions: a malformed rules.ron/state_machine.ron/behavior
    // file already reports its own parse error, and would flood this check with secondary noise
    // once its rules/transitions/bindings are entirely absent from the two data sets below (same
    // reasoning as `check_ui_trigger_reachability`'s own gate). But unlike that forward check, an
    // unparseable SCENE is an equally real risk here: `do_validate` silently drops any scene that
    // fails to parse from the `scenes` list, shrinking `collect_reachable_ui_triggers`'s reachable
    // set -- so a live rule handling that scene's own (now-invisible) button could be wrongly
    // flagged orphaned. `orphan_rule_prereqs_clean` is true only when BOTH logic files and every
    // scene parsed cleanly.
    if orphan_rule_prereqs_clean {
        let reachable = collect_reachable_ui_triggers(project_config, scenes, prefab_catalog);
        warnings.extend(check_orphan_ui_rules(&reachable, rules, state_machine, behaviors));
    }

    warnings
}

// ── Core validation (shared by `run` and `validate_project`) ─────────────────

fn do_validate(project_dir: &Path, strict: bool) -> ValidationRun {
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut file_results: Vec<FileResult> = Vec::new();

    let project_config: Option<ProjectConfig> = find_project_ron(project_dir)
        .and_then(|name| try_parse(project_dir, &name, &mut file_results));

    // asset_catalog/prefab_catalog/stats_path/items_path are all ProjectConfig-configured paths
    // (see load_configured_catalog's doc comment) -- every shipped project happens to set the
    // first two to the "assets.ron"/"prefabs/prefabs.ron" convention path, which is why hardcoding
    // those two literals here never surfaced as a bug: a project that relocates either (or omits
    // it, same as a project legitimately omitting stats_path/items_path today) would previously
    // have silently lost every check depending on that catalog, exactly the items_path gap this
    // helper was originally written to close.
    let asset_catalog: Option<AssetCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.asset_catalog.as_deref()),
        "assets.ron",
        "asset_catalog",
        &mut file_results,
    );

    let prefab_catalog: Option<PrefabCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.prefab_catalog.as_deref()),
        "prefabs/prefabs.ron",
        "prefab_catalog",
        &mut file_results,
    );

    let stat_catalog: Option<StatCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.stats_path.as_deref()),
        "stats/stats.ron",
        "stats_path",
        &mut file_results,
    );

    let item_catalog: Option<ItemCatalog> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.items_path.as_deref()),
        "items/items.ron",
        "items_path",
        &mut file_results,
    );

    let mut scenes: Vec<(String, GameSceneV2)> = Vec::new();
    for path in glob_dir(project_dir, "scenes", ".scene.ron") {
        let r = rel(project_dir, &path);
        if let Some(scene) = parse_file::<GameSceneV2>(&path, &r, &mut file_results) {
            scenes.push((r, scene));
        }
    }

    let ResolvedLogicFiles { rules, rules_source, state_machine, state_machine_source } =
        resolve_logic_files(project_dir, project_config.as_ref(), &mut file_results);

    let mut behaviors: Vec<(String, StateMachineAsset)> = Vec::new();
    for path in glob_dir(project_dir, "behaviors", ".behavior.ron") {
        let r = rel(project_dir, &path);
        if let Some(b) = parse_file::<StateMachineAsset>(&path, &r, &mut file_results) {
            behaviors.push((r, b));
        }
    }

    let mut dialogues: Vec<(String, DialogueDef)> = Vec::new();
    for path in glob_dir(project_dir, "dialogues", ".dialogue.ron") {
        let r = rel(project_dir, &path);
        if let Some(d) = parse_file::<DialogueDef>(&path, &r, &mut file_results) {
            dialogues.push((r, d));
        }
    }

    let _model_fixes: Option<ModelFixesAsset> = load_configured_catalog(
        project_dir,
        project_config.as_ref().and_then(|c| c.model_fixes_path.as_deref()),
        "overrides/model_fixes.ron",
        "model_fixes_path",
        &mut file_results,
    );

    let all_actions = collect_actions(
        rules.as_ref().map(|r| (rules_source.as_str(), r)),
        state_machine.as_ref().map(|s| (state_machine_source.as_str(), s)),
        &behaviors,
        &dialogues,
    );

    let extra_scene_paths = discover_extra_scenes(
        project_dir,
        project_config.as_ref().map(|c| c.initial_scene.as_str()),
        &all_actions,
        &mut scenes,
        &mut file_results,
    );

    let logic_files_parsed_cleanly = file_results
        .iter()
        .filter(|r| {
            r.rel_path == rules_source
                || r.rel_path == state_machine_source
                || r.rel_path.starts_with("behaviors/")
        })
        .all(|r| r.is_ok());
    let extra_scene_path_set: HashSet<&str> = extra_scene_paths.iter().map(String::as_str).collect();
    let scenes_parsed_cleanly = file_results
        .iter()
        .filter(|r| r.rel_path.starts_with("scenes/") || extra_scene_path_set.contains(r.rel_path.as_str()))
        .all(|r| r.is_ok());

    let project = LoadedProject {
        project_dir,
        project_config: project_config.as_ref(),
        asset_catalog: asset_catalog.as_ref(),
        prefab_catalog: prefab_catalog.as_ref(),
        stat_catalog: stat_catalog.as_ref(),
        item_catalog: item_catalog.as_ref(),
        scenes: &scenes,
        dialogues: &dialogues,
        actions: &all_actions,
        rules: rules.as_ref().map(|r| (rules_source.as_str(), r)),
        state_machine: state_machine.as_ref().map(|s| (state_machine_source.as_str(), s)),
        behaviors: &behaviors,
        logic_files_parsed_cleanly,
        scenes_parsed_cleanly,
    };

    let mut cross_errors = cross_file_checks(project);
    cross_errors.extend(check_ui_trigger_reachability(project));
    cross_errors.extend(check_asset_root_paths(project));

    let strict_warnings = if strict { strict_checks(project) } else { Vec::new() };

    let all_valid = file_results.iter().all(|r| r.is_ok())
        && cross_errors.is_empty()
        && strict_warnings.is_empty();

    ValidationRun { project_name, file_results, cross_errors, strict_warnings, all_valid }
}

// ── Public: used by `watch` ───────────────────────────────────────────────────

pub fn validate_project(project_dir: &Path) -> ValidateResult {
    let vr = do_validate(project_dir, false);

    let mut errors = Vec::new();
    for fr in &vr.file_results {
        for e in &fr.errors {
            errors.push(format!("{}: {}", fr.rel_path, e));
        }
    }
    for ce in &vr.cross_errors {
        errors.push(format!("{}: {}", ce.source_file, ce.message));
    }

    ValidateResult {
        all_valid: vr.all_valid,
        file_count: vr.file_results.len(),
        errors,
    }
}

// ── Output ────────────────────────────────────────────────────────────────────

fn print_human(
    project_dir: &Path,
    file_results: &[FileResult],
    cross_errors: &[CrossFileError],
    strict_warnings: &[StrictWarning],
    all_valid: bool,
) {
    println!("Validating: {}", project_dir.display());
    println!();

    const CROSS_LABEL: &str = "Cross-file checks";
    const STRICT_LABEL: &str = "Strict checks";
    let col_width = file_results
        .iter()
        .map(|r| r.rel_path.len())
        .chain(std::iter::once(CROSS_LABEL.len()))
        .chain(std::iter::once(STRICT_LABEL.len()))
        .max()
        .unwrap_or(24)
        + 4;

    for result in file_results {
        let status = if result.is_ok() { "OK" } else { "ERROR" };
        println!("  {:<width$} {}", result.rel_path, status, width = col_width);
        if !result.is_ok() {
            for err in &result.errors {
                println!("    {err}");
            }
            println!();
        }
    }

    println!();

    let cross_status = match cross_errors.len() {
        0 => "OK".to_string(),
        1 => "1 error".to_string(),
        n => format!("{n} errors"),
    };
    println!("  {:<width$} {}", CROSS_LABEL, cross_status, width = col_width);
    for err in cross_errors {
        println!("    {}: {}", err.source_file, err.message);
    }

    if !strict_warnings.is_empty() {
        println!();
        let strict_status = match strict_warnings.len() {
            1 => "1 warning".to_string(),
            n => format!("{n} warnings"),
        };
        println!("  {:<width$} {}", STRICT_LABEL, strict_status, width = col_width);
        for w in strict_warnings {
            println!("    {}: {}", w.source_file, w.message);
        }
    }

    println!();

    let file_error_count = file_results.iter().filter(|r| !r.is_ok()).count();
    let total = file_results.len();

    if all_valid {
        println!("{total} files checked — all valid.");
    } else {
        let mut parts = Vec::new();
        if file_error_count > 0 {
            parts.push(format!(
                "{} file error{}",
                file_error_count,
                if file_error_count == 1 { "" } else { "s" }
            ));
        }
        if !cross_errors.is_empty() {
            parts.push(format!(
                "{} cross-file error{}",
                cross_errors.len(),
                if cross_errors.len() == 1 { "" } else { "s" }
            ));
        }
        if !strict_warnings.is_empty() {
            parts.push(format!(
                "{} unused definition{}",
                strict_warnings.len(),
                if strict_warnings.len() == 1 { "" } else { "s" }
            ));
        }
        println!("{total} files checked — {}.", parts.join(", "));
    }
}

fn print_json(
    project_name: &str,
    file_results: &[FileResult],
    cross_errors: &[CrossFileError],
    strict_warnings: &[StrictWarning],
    all_valid: bool,
) {
    let val = serde_json::json!({
        "valid": all_valid,
        "project": project_name,
        "files": file_results.iter().map(|r| serde_json::json!({
            "path": r.rel_path,
            "valid": r.is_ok(),
            "errors": r.errors.iter().map(|e| serde_json::json!({
                "type": "parse_error",
                "message": e,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "cross_file_errors": cross_errors.iter().map(|e| serde_json::json!({
            "type": e.error_type,
            "source": e.source_file,
            "message": e.message,
        })).collect::<Vec<_>>(),
        "strict_warnings": strict_warnings.iter().map(|w| serde_json::json!({
            "type": w.kind,
            "source": w.source_file,
            "message": w.message,
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&val).unwrap());
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(
    project_dir: &Path,
    mode: &OutputMode,
    strict: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !project_dir.is_dir() {
        return Err(format!("{}: not a directory", project_dir.display()).into());
    }

    let vr = do_validate(project_dir, strict);

    if mode.json {
        print_json(
            &vr.project_name,
            &vr.file_results,
            &vr.cross_errors,
            &vr.strict_warnings,
            vr.all_valid,
        );
    } else {
        print_human(
            project_dir,
            &vr.file_results,
            &vr.cross_errors,
            &vr.strict_warnings,
            vr.all_valid,
        );
    }

    if !vr.all_valid {
        std::process::exit(1);
    }

    Ok(())
}
