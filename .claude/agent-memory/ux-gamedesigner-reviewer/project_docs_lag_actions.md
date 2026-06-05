---
name: Docs lag the action schema
description: Designer-facing docs (20_data_formats.md actions table, 30_runtime_events_and_logic.md Appendix, STATUS.md ABI list) are consistently not updated when new Action variants are added to schema/actions.rs
type: project
---

When new `Action` variants land in `crates/ironhold_core/src/schema/actions.rs`, three doc surfaces are consistently missed:

1. `docs/20_data_formats.md` — the "Available actions" table at ~line 1143 (under `## logic/rules.ron — LogicRulesAsset`)
2. `docs/30_runtime_events_and_logic.md` — the `### Actions ✅` appendix at ~line 258 AND the `## Action model` action-category section at ~line 125
3. `docs/STATUS.md` — the `Engine ABI` actions list at ~line 85

There is an explicit reminder in `30_runtime_events_and_logic.md` at the end of the appendix:
> "New Messages or Actions must update `docs/STATUS.md` (Engine ABI section), this appendix, and `docs/20_data_formats.md` with an authoring example."

This is regularly ignored. Confirmed missing for: `ModifyStat`, `SetStat`, `ApplyModifier`, `RemoveModifier`, `ShowDamagePopup`, `SetEntityVisible`, `EmitEventAfterDelay`, `LoadSceneOverlay`, `UnloadOverlay`, `ToggleOverlay`, `PlayAnimationOn`, `EmitEvent`.

Per-entity `PrefabDef` fields are also missed: `stat_label` and `world_stat_bar` are in the schema and used in `primitive_world/prefabs/prefabs.ron` (attack_dummy) but absent from the `PrefabDef fields` table at line 606 of `20_data_formats.md`.

New prefab KINDS are missed too: `kind: Foliage` (with `FoliageDef`/`FoliageClustersDef`/`FoliageMaterialDef`, schema in catalog.rs ~line 11-92) landed in foliage_demo with ZERO entries in `docs/20_data_formats.md` — no Foliage section, and the PrefabDef `kind` row (~line 956) still lists only `Actor`/`Prop`/`Primitive`. The `foliage` field is also absent from the PrefabDef fields table. Designer cannot author foliage from docs alone.

**Why:** the schema is the source of truth (Rust); designers only see the docs. A new Action that exists only in Rust + an example RON file is essentially un-discoverable for a designer building a new project from scratch.

**How to apply:** when reviewing any new Action variant or `PrefabDef` field, always check the three doc surfaces above and flag missing entries as blockers. Also flag missing entries on the `{self}` substitution list in `crates/ironhold_core/src/CLAUDE.md` (developer-side, not designer-side, but the project-internal reference for what `{self}` does).
