---
name: cli-validate-coverage-model
description: ironhold_cli validate silently skips checks when a catalog is absent; hardcoded stats/prefab paths diverge from ProjectConfig's configurable ones; scene-path coverage is partial
metadata:
  type: project
---

`ironhold_cli validate`'s cross-file checks are **all conditional on the relevant catalog having
parsed**, and `try_parse()` returns `None` with *no diagnostic* when the file simply doesn't exist.
Net effect: a typo in a *configured catalog path* makes every check that depends on that catalog
silently vanish — validate exits 0 and reports nothing.

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

**Why:** these gaps are invisible by construction — the failure mode is *absence* of output, so
they don't show up in test runs or in "all projects validate clean" verification.

**How to apply:** when reviewing any new `validate.rs` check, ask (a) what happens when its catalog
is missing vs malformed (malformed is loud via `parse_file`; missing is silent), and (b) whether
the path it resolves matches the runtime's resolution. When a change adds a check over a *set* of
enum variants or config fields, enumerate the full set and confirm none were missed.
