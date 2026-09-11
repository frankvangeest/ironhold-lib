---
name: cli-validate-never-calls-schema-validate
description: ironhold_cli validate parses RON but never calls AssetCatalog::validate()/PrefabCatalog::validate(), so every schema-level invariant is runtime-only and invisible to the CLI
metadata:
  type: project
---

`crates/ironhold_cli/src/commands/validate.rs` has **zero** `.validate()` calls (verified by grep
over all of `crates/ironhold_cli/src/`). It deserializes each RON file with `ron_from_str` and then
runs its *own* hand-written cross-file checks. The `validate()` methods on `AssetCatalog` /
`PrefabCatalog` / `StateMachineAsset` / `ProjectConfig` are called only from
`crates/ironhold_core/src/runtime/scene_manager/project_loader.rs`.

**Why:** it matters when reasoning about "does the CLI already cover X?" — a whole class of
invariants is enforced *only* at engine boot:
- `AssetCatalog::validate()` — empty `models[k].path`, empty `decals[k]`, `particle_count >
  MAX_PARTICLES_PER_EFFECT` (256), `flipbook` + `uv_distort` combined. Note it does **not** cover
  empty `textures[k]` or empty `audio[k].path` at all — those are unvalidated everywhere.
- `PrefabCatalog::validate()` — Foliage-without-`foliage`, empty `leaf_texture`, `toon_bands`
  range, Primitive-with-`model`, child `shape`/`prefab` exclusivity.
- Also true in reverse: `schema_version` mismatch is a hard runtime error but the CLI's own
  `assets_schema_version_regression` test is what guards it, not `validate`.

**How to apply:** never argue "the CLI already rejects that, `AssetCatalog::validate()` covers it."
It doesn't. If a CLI check's early-return is justified as "redundant because `validate()` already
rejects this", that reasoning is wrong — the two run in different processes. Conversely, when a CLI
check is *added* for something `validate()` also covers, that's duplication across two layers with
no shared source of truth, and they will drift.

Related: [[validate-hardcoded-source-file-literals]], [[action-ron-typos-are-silent]]
