---
name: cli-validate-coverage-model
description: ironhold_cli validate silently skips checks when a catalog is absent; only two severity tiers exist (CrossFileError=hard, StrictWarning=--strict-only); hardcoded stats/prefab paths diverge from ProjectConfig's; scene-path coverage is partial; dialogue parse gate landed but its referential checks and query.rs's collector did not
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

4. **Dialogue coverage: the parse half landed, the *referential* half did not.**
   `feature/cli-validate-dialogues` (2026-09-04) added `glob_dir("dialogues", ".dialogue.ron")` +
   `parse_file::<DialogueDef>` to `do_validate` and extended `collect_actions` to walk
   `nodes[].choices[].do_actions` — so `collect_actions` now covers **all five** `Action`-bearing
   schema surfaces (grep `Vec<Action>` in `schema/`: dialogue.rs:49, project.rs:131/134/154/316),
   and no `Action` variant nests another `Action`, so no recursion is needed. What is still
   missing: **no existence check on `PrefabDef.dialogue`** (contrast `def.behavior`, checked at
   `validate.rs`~821 with the `bad_behavior_file` fixture) and **no arm for
   `Action::StartDialogue { dialogue_path }`** in `cross_file_checks` (falls through `_ => {}`).
   Runtime failure mode for a typo'd path is bad: `action_executor.rs`~1135 sets `ActiveDialogue`
   active and `asset_server.load()`s a handle that never resolves — panel opens and never
   closes, no ironhold-side `warn!`. Also unchecked despite dialogues now being parsed: `jump_to`
   node-id validity (which *does* have a runtime `warn!` at `capabilities/dialogue.rs`~165 — a
   textbook [[cli-runtime-mirror-check-pairs]] gap, and the only check that would flag anything in
   shipped content, since `npc_intro.dialogue.ron` has 8 `jump_to`s and zero `do_actions`),
   duplicate node ids, `portrait` texture-catalog key, and `DialogueCondition::StatAtLeast.stat_key`.

4b. **`query.rs` and `validate.rs` disagree on what "all the project's actions" means.**
   `query.rs::collect_logic` globs `scenes` and `behaviors` but **not** `dialogues`, so after the
   fix above `validate` walks dialogue `do_actions` while `query actions`/`query events` still
   don't. Whenever a new `Action` authoring surface is added, both collectors need it — they are
   two independent enumerations of the same surface set with no shared helper.

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
