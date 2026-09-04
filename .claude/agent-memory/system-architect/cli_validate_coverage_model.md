---
name: cli-validate-coverage-model
description: ironhold_cli validate silently skips checks when a catalog is absent; only two severity tiers exist (CrossFileError=hard, StrictWarning=--strict-only); hardcoded stats/prefab paths diverge from ProjectConfig's; scene-path coverage is partial
metadata:
  type: project
---

`ironhold_cli validate`'s cross-file checks are **all conditional on the relevant catalog having
parsed**, and `try_parse()` returns `None` with *no diagnostic* when the file simply doesn't exist.
Net effect: a typo in a *configured catalog path* makes every check that depends on that catalog
silently vanish — validate exits 0 and reports nothing.

**There are exactly two severity tiers, and no mid-tier "warning" in the default run.**
`CrossFileError` (pushed in `cross_file_checks`) is *always* a hard error → exit 1. The only softer
signal is `StrictWarning`, produced by `strict_checks(asset_catalog, prefab_catalog, scenes,
actions)` and surfaced **only** under `--strict` (still exit 1 when present). Any proposed check
described as "a warning, not an error" therefore belongs in `strict_checks`, not as a
`CrossFileError` push — plan docs routinely get this backwards. `strict_checks` already receives
`scenes` + `prefab_catalog`, so heuristic scene↔player-prefab checks fit there with no plumbing;
`jump_cannot_clear_ground_sensor` / `negative_coyote_time_secs` are the precedent (both are
`--strict` warnings even though `crates/ironhold_core/src/CLAUDE.md` calls them "errors").

Three concrete asymmetries that keep resurfacing when reviewing `crates/ironhold_cli/src/commands/validate.rs`:

1. **Hardcoded vs configured catalog paths.** `do_validate` loads `prefabs/prefabs.ron` and
   `stats/stats.ron` as hardcoded convention paths, but `ProjectConfig` exposes `prefab_catalog`
   and `stats_path` as configurable (and `items_path`, which the merchant check *does* resolve via
   config). Every shipped project happens to use the conventional paths, so this is latent, not
   live. A project that relocates stats.ron loses all stat checks with no warning.
   `resolve_project_path()` (scene_manager/mod.rs) is a plain `format!("{root}/{path}")`, so the
   CLI's `project_dir.join(path)` is a faithful mirror of runtime resolution — no shared-asset
   special-casing to worry about.

2. **Scene-path existence coverage is partial.** Four `Action` variants carry a project-relative
   `.scene.ron` path: `LoadScene`, `LoadSceneOverlay`, `PreloadScene`, `ToggleOverlay`. Plus
   `ProjectConfig.initial_scene`. Any of these not covered by an existence check fall through
   `cross_file_checks`'s `_ => {}` catch-all silently.

3. **`source_file` for prefab-catalog-derived errors is the hardcoded string
   `"prefabs/prefabs.ron"`** at ~10 sites — consistent, but it will read wrong the day a project
   uses a different `prefab_catalog` path.

4. **`.dialogue.ron` is never parsed by `validate` at all** (confirmed 2026-09-04): `do_validate`
   parses the project config, `assets.ron`, `prefabs/prefabs.ron`, `stats/stats.ron`,
   `glob_dir("scenes", ".scene.ron")`, `logic/rules.ron`, `logic/state_machine.ron`,
   `glob_dir("behaviors", ".behavior.ron")`, and `overrides/model_fixes.ron` — no dialogue glob.
   `DialogueChoiceDef.do_actions` is therefore an `Action` authoring surface with **no design-time
   parse gate whatsoever**, and no `ironhold_core` test touches dialogue either. Latent rather than
   live today only because the single shipped dialogue file
   (`3rd_person_game_demo/dialogues/npc_intro.dialogue.ron`) contains zero `do_actions`.

5. **The standing `cargo test -p ironhold_cli` gate covers only 9 of the 15 project dirs.**
   `crates/ironhold_cli/tests/validate_projects.rs` hardcodes one `#[test]` per project and is
   missing `camera_modes`, `dynamic_animation_control`, `foliage_demo`, `stats_demo`,
   `blank_project`, and `integration_tests`. `test_web.py`'s `PROJECTS` list (14, everything but
   `integration_tests`) is the only broad gate, and it only runs on `integration` batches. So a
   "manual sweep of all 14 projects validated clean" claim is real but **not reproducible at
   feature-branch speed** — adding the missing one-liners to `validate_projects.rs` is the cheap fix
   any reviewer should recommend when a change's safety argument rests on such a sweep.

**Why:** these gaps are invisible by construction — the failure mode is *absence* of output, so
they don't show up in test runs or in "all projects validate clean" verification.

**How to apply:** when reviewing any new `validate.rs` check, ask (a) what happens when its catalog
is missing vs malformed (malformed is loud via `parse_file`; missing is silent), and (b) whether
the path it resolves matches the runtime's resolution. When a change adds a check over a *set* of
enum variants or config fields, enumerate the full set and confirm none were missed.
