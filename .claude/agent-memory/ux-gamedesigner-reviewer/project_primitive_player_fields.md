---
name: primitive-player-fields
description: Which PrefabDef fields apply to tags:["player"] prefabs after player_model_source_unification v1; canonical primitive-player example
metadata:
  type: project
---

After `player_model_source_unification.md` v1, a `kind: Primitive` (capsule) player prefab routes
through the same spawn pipeline as a `kind: Actor` (GLB) player, so both honor the same fields.

**Fields that take effect on any `tags: ["player"]` prefab (primitive or GLB):**
`player_index`, `material`, `stat_templates` (→ per-player StatMap), `stat_label`/`world_stat_bar`.

**Still silently no-op on any player prefab (deliberate scope boundary, not a bug):**
`behavior`, `interactable`, `dialogue`, `inventory`, `trigger_zone`.

**Primitive-player-only limits (not yet supported at all, "v3-deferred"):** a primitive player
combined with `scene.terrain: Some(...)` (validate error + runtime warn) or referenced from a
character-select `Action::Spawn` (runtime warn). Only the immediate, non-terrain scene-load path
spawns primitive players.

**Canonical examples:**
- `local_coop_demo` room7 scene + `player_p1_primitive`/`player_p2_primitive` prefabs — 2 primitive
  players in vertical split, distinct material tints (tint_blue/tint_red) + own mana `world_stat_bar`.
  This is the proof the single-primitive-player cap is gone.
- `primitive_world`'s `player_capsule` — the definitive single-primitive-player regression baseline
  (uses global `player_health`, NOT per-instance stat_templates, so it does NOT exercise StatMap).

**Why:** collapses the old separate ~165-line inline primitive-player spawn block into
`spawn_player_entity_core`. Docs live in docs/20_data_formats.md `### Special tag: "player"`.

**How to apply:** when reviewing a new player-related field, check it's forwarded in
`assemble_player_config` and inserted in the shared post-dispatch code so both model sources get it;
flag docs/20 if a new field's applicability to primitive vs GLB players isn't stated.
Related: [[world_stat_bar_style_landscape]], [[owner_player-player_index-wiring]].
