---
name: primitive-player-fields
description: Which PrefabDef fields apply to tags:["player"] prefabs after player_model_source_unification v1/v2; canonical primitive-player examples (room7 bare capsule, room10 composed body)
metadata:
  type: project
---

A `kind: Primitive` (capsule) player prefab routes through the same spawn pipeline as a `kind: Actor`
(GLB) player, so both honor the same fields.

**Fields that take effect on any `tags: ["player"]` prefab (primitive or GLB):**
`player_index`, `material`, `stat_templates` (→ per-player StatMap), `stat_label`/`world_stat_bar`,
and (primitive only) `children` for a cosmetic composed body — the physics capsule still comes
entirely from `primitive.radius`/`primitive.height`.

**Still silently no-op on any player prefab (deliberate scope boundary, not a bug):**
`behavior`, `interactable`, `dialogue`, `inventory`, `trigger_zone`.

**Primitive-player-only unsupported contexts (all three now documented in docs/20's "Special tag:
player", each with a runtime warning rather than silence):** `scene.terrain: Some(...)`,
`Action::Spawn` (character-select), and a primitive prefab listed in `join_prefab_keys` (hot-join —
GLB/`Actor` only). Only a prefab placed directly in a scene's `entities:` list spawns a primitive
player. (The hot-join gap I previously flagged as undocumented was closed in the v2 change.)

**Open ambiguity, worth re-checking every time:** whether a top-level `material:` override recurses
into `children:`. See [[material-override-vs-children]] — room10's composed body is the first shipped
prefab anywhere to combine the two, and neither docs/20's `material` row nor its `ChildPrimitiveDef`
table states the precedence.

**Also unstated:** child `offset` values in a composed player body are absolute magic numbers, not
derived from `primitive.radius`/`height`. Change the capsule's `height` and the head/shoulders stay
put. No comment or docs note warns about this.

**Canonical examples:**
- `local_coop_demo` room7 + `player_p1_primitive`/`player_p2_primitive` — 2 **bare-capsule** primitive
  players in vertical split, distinct tints + own mana `world_stat_bar` (decorative there; nothing
  spends it). Proof the single-primitive-player cap is gone. Do not retrofit — it is the regression
  baseline.
- `local_coop_demo` room10 + `player_p2_primitive_split_ring` — the **mixed** GLB(P1)+primitive(P2)
  pairing and the first **composed `children:` body** on any player prefab. P2 mirrors
  `player_p2_split_ring` field-for-field (all 13 `inputs` fields, `movement`, `camera`,
  `stat_templates`, `player_index`); only `kind`/`shape`/`model`/`display_name`/`material`/
  `primitive`/`children` and `world_stat_bar.offset` (2.3 vs 2.8, shorter body) differ.
- `primitive_world`'s `player_capsule` — the definitive single-primitive-player regression baseline
  (uses global `player_health`, NOT per-instance `stat_templates`, so it does NOT exercise StatMap).

**Known unavoidable gap, pre-empted on screen in room10:** no primitive player has an
`animation_policy`, so a primitive body slides instead of walking. A composed body also has **no
facing indicator** (symmetric head + shoulders) — combined with the missing walk cycle, an idle
primitive player can't tell which way they're pointing.

**How to apply:** when reviewing a new player-related field, check it's forwarded so both model
sources get it; flag docs/20 if a new field's applicability to primitive vs GLB players isn't stated.
Related: [[world_stat_bar_style_landscape]], [[owner_player-player_index-wiring]],
[[local-coop-demo-room-conventions]].
