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

This is regularly ignored. Confirmed missing for: `ModifyStat`, `SetStat`, `ApplyModifier`, `RemoveModifier`, `ShowDamagePopup`, `SetEntityVisible`, `EmitEventAfterDelay`, `LoadSceneOverlay`, `UnloadOverlay`, `ToggleOverlay`, `PlayAnimationOn`, `EmitEvent`, `ShowFloatingText`.

`ShowFloatingText` (struct variant: `entity` + `text`, schema at actions.rs ~line 133) is the worst-case version of this pattern: it is the *visible payoff* in the 3rd_person_game_demo targeting demo (`rules.ron` `target.changed` rule → `ShowFloatingText(entity: "{target}", text: "Selected!")`) yet has ZERO occurrences anywhere in docs/. A designer sees the effect in WASM and cannot find the action.

**Contrast (done right):** the targeting *fields* shipped with this same feature were documented correctly — `click_selectable`/`targetable` in the PrefabDef table (20_data_formats.md ~1051-1052), `target_next`/`target_range` in the InputMap table (~1147-1148), and all `target.*` events in 30_runtime_events_and_logic.md (~111-116). So the lag is action-table-specific, not feature-wide: schema FIELDS and EVENTS get documented, new struct-variant ACTIONS get missed.

Per-entity `PrefabDef` fields are also missed: `stat_label` and `world_stat_bar` are in the schema and used in `primitive_world/prefabs/prefabs.ron` (attack_dummy) but absent from the `PrefabDef fields` table at line 606 of `20_data_formats.md`.

`NpcDef` collider fields are missed too: `collider_radius` / `collider_height` (schema in catalog.rs ~line 1078/1082, optional `Option<f32>`, humanoid defaults 0.35 m / 1.6 m for sizing non-humanoid GLB NPCs) are absent from the `NpcDef` fields table in `20_data_formats.md` (table at ~line 1268, ends at `angular_damping`). Note the file already has `collider_radius`/`collider_height` rows for the PLAYER/movement block (~line 1218, defaults 0.4/1.8) — same field names, different block, different defaults; do not confuse the two when reviewing. The `NpcDef` table uses a `None (effective value)` default-column convention (see `fov_degrees` row) — new optional NPC rows should follow it.

New prefab KINDS are missed too: `kind: Foliage` (with `FoliageDef`/`FoliageClustersDef`/`FoliageMaterialDef`, schema in catalog.rs ~line 11-92) landed in foliage_demo with ZERO entries in `docs/20_data_formats.md` — no Foliage section, and the PrefabDef `kind` row (~line 956) still lists only `Actor`/`Prop`/`Primitive`. The `foliage` field is also absent from the PrefabDef fields table. Designer cannot author foliage from docs alone.

`CameraShake` is documented but **inaccurately**: the actions table row (20_data_formats.md ~line 3280) says it shakes "the active orbit camera" (singular) and only warns about flycam scenes. Reality (per `crates/ironhold_core/src/CLAUDE.md`'s "Known limitation" note): it fires on *both* cameras in a `split:` scene and silently no-ops entirely in a `party:` scene. The split/party caveat lives only in the developer CLAUDE.md, never reached designer docs. See [[camera-config-party-split-nesting]].

**Why:** the schema is the source of truth (Rust); designers only see the docs. A new Action that exists only in Rust + an example RON file is essentially un-discoverable for a designer building a new project from scratch.

**How to apply:** when reviewing any new Action variant or `PrefabDef` field, always check the three doc surfaces above and flag missing entries as blockers. Also flag missing entries on the `{self}` substitution list in `crates/ironhold_core/src/CLAUDE.md` (developer-side, not designer-side, but the project-internal reference for what `{self}` does).
