---
name: cli-validate-never-calls-schema-validate
description: ironhold_cli validate only wires AssetCatalog/PrefabCatalog .validate(); StatCatalog/ItemCatalog/GameSceneV2/StateMachine/ProjectConfig invariants stay runtime-only and invisible to the CLI
metadata:
  type: project
---

`crates/ironhold_cli/src/commands/validate.rs` used to have **zero** `.validate()` calls. As of
`feature/cli_validate_gap_closures` (reviewed 2026-09-11, verify it merged before relying on this)
`cross_file_checks` calls exactly two: `AssetCatalog::validate()` → `invalid_asset_catalog` and
`PrefabCatalog::validate()` → `invalid_prefab_catalog`. Everything else is still CLI-invisible.

`project_loader.rs` calls **six** (`:41` ProjectConfig, `:260` StateMachineAsset, `:302` AssetCatalog,
`:312` PrefabCatalog, `:319` StatCatalog, `:342` ItemCatalog) — and only ever `error!`s, never aborts
the load, so the CLI is deliberately stricter (exit 1) than the engine on the two it does wire.

Still unwired in the CLI, including the two with the highest-value invariants:
- `StatCatalog::validate()` — `min > max`, `base` outside `[min, max]`, `soft_max < max`, a modifier
  referencing an undefined stat.
- `ItemCatalog::validate()` — `max_stack == 0` (silently unusable item), empty `display_name`.
- `GameSceneV2::validate()`, `StateMachineAsset::validate()`, `ProjectConfig::validate()`.

Both wired `validate()`s `return Err` on the **first** violation, so unlike every other
`cross_file_checks` check (report-all + sorted) only one invariant per catalog surfaces per run.

**Why:** it matters when reasoning about "does the CLI already cover X?" — a whole class of
invariants is enforced *only* at engine boot:
- `AssetCatalog::validate()` — empty `models[k].path`, empty `decals[k]`, `particle_count >
  MAX_PARTICLES_PER_EFFECT` (256), `flipbook` + `uv_distort` combined. Note it does **not** cover
  empty `textures[k]` or empty `audio[k].path` at all — those are unvalidated everywhere.
- `PrefabCatalog::validate()` — Foliage-without-`foliage`, empty `leaf_texture`, `toon_bands`
  range, Primitive-with-`model`, child `shape`/`prefab` exclusivity.
- Also true in reverse: `schema_version` mismatch is a hard runtime error but the CLI's own
  `assets_schema_version_regression` test is what guards it, not `validate`.

**How to apply:** check which of the six is actually wired before arguing "the CLI already rejects
that". Wiring one retroactively is a breaking change for **test fixtures**, not for shipped projects:
wiring `PrefabCatalog::validate()` forced ~40 CLI fixtures to be edited (schema_version 1→2,
Primitive prefabs needed a `shape`), and 7 were still missed while staying green only because their
tests assert exit 1 on a *content* match. Before wiring another, statically scan every fixture for
the invariant rather than trusting a green suite.

Related: [[validate-hardcoded-source-file-literals]], [[action-ron-typos-are-silent]]
