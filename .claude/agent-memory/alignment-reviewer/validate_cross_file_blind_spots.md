---
name: validate-cross-file-blind-spots
description: Structural blind spots in ironhold_cli validate.rs cross_file_checks — hardcoded stats.ron path, try_parse silent-None, ToggleOverlay/initial_scene uncovered scene paths, collect_actions missing dialogue files
metadata:
  type: project
---

Recurring gaps to check whenever a new `ironhold_cli validate` cross-file check is added
(`crates/ironhold_cli/src/commands/validate.rs`). Established during the
`feature/cli-validate-hardening` review (2026-08-30, verdict ALIGNED).

**A validate-only change is the second-easiest ALIGNED verdict after a runtime-warn diagnostic**
(see [[diagnostic-only-feature-pattern]]): no new schema field, no `ironhold_core` change, purely
additive exit-1 diagnostics. The whole review reduces to "does the check's lookup match what the
runtime actually resolves against, and is the coverage complete?"

**Why:** every check in this file is a hand-written match arm or hand-written catalog walk, so
coverage drifts field-by-field rather than being enforced by the compiler.

**How to apply — the four recurring blind spots:**

1. **`do_validate` hardcodes `"stats/stats.ron"`** while `ProjectConfig.stats_path` is a real
   configurable `Option<String>` (schema/project.rs:~225). Any project with a custom `stats_path`
   gets `stat_catalog == None` → *every* stat-keyed check silently skips. `items_path` is loaded
   correctly from `ProjectConfig` — that is the pattern to copy, not the stats one. Latent only:
   all shipped projects use the convention path.
2. **`try_parse` returns `None` for a missing file with no `FileResult` pushed** (validate.rs:90-100).
   So `items_path: Some("items/itmes.ron")` (typo) → catalog `None` → the check it gates is silently
   skipped *and* no `missing_file` error is reported. Any new configurable-path catalog inherits this.
3. **Scene-path actions are covered piecemeal.** `LoadScene`/`LoadSceneOverlay`/`PreloadScene` are
   checked on disk; **`ToggleOverlay(String)` is also a scene path and is not** (actively used in
   `primitive_world/logic/state_machine.ron:48-49`), and **`ProjectConfig.initial_scene` is never
   checked at all** — the single most load-bearing designer-authored scene path in a project.
   `project_dir.join(path)` is the correct disk check: `resolve_project_path` (scene_manager/mod.rs:734)
   is literally `format!("{project_root}/{path}")`, and no shipped RON interpolates `{var}` into a
   scene path, so no false-positive risk.
4. **`collect_actions` only walks `logic/rules.ron`, `logic/state_machine.ron`, `behaviors/*.behavior.ron`.**
   `DialogueChoice.do_actions` (`schema/dialogue.rs:49`, `dialogues/*.dialogue.ron`) is *not*
   collected, so no per-action check applies there. Affects every arm in the match, not just new ones.

**Correct-lookup verification (the one substantive thing to actually check):** trace where the
runtime resolves the key. `MerchantDef.currency_stat` reads `scene_state.loaded_stats.0`
(action_executor.rs:~1372) = the global stats.ron catalog, **not** a per-player `StatMap` — so
`stat_catalog.stats.contains_key` is right. Had it been player-scoped, checking stats.ron would
false-positive against `stat_templates`-only stats (cf. [[per-player-stat-pools-pattern]]).

**Scoping:** merchant checks are prefab-catalog-scoped (`source_file: "prefabs/prefabs.ron"`,
iterating `catalog.prefabs`) because `MerchantDef` is a prefab-local condition — same rule as
[[diagnostic-only-feature-pattern]]. Note `MerchantDef` lives at `PrefabDef.merchant`, not
`PrefabDef.components.merchant`.

**Sibling gap left open:** `ItemDef.currency_stat: Option<String>` (schema/items.rs:66) is a second
designer-authored global stat key, unchecked, whose runtime twin is a bare
`warn!("TakeAllFromContainer: currency_stat {:?} not found in stats")` (action_executor.rs:~1485) —
the exact runtime-warn-without-CLI-twin shape this file exists to close.
