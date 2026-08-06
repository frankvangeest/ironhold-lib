---
name: material-override-vs-children
description: Top-level PrefabDef material vs per-child colors in children:[] — precedence is undocumented and, until room10, had zero shipped examples anywhere in the repo
metadata:
  type: project
---

A `kind: Primitive` prefab can specify surface appearance in **three** overlapping places with no
documented precedence:

1. top-level `material: "<key>"` (an `AssetCatalog.materials` override applied after the body is built)
2. the parent's own `primitive: (color:, roughness:, metallic:)`
3. each child's `primitive: (color:, ...)` **and** each child's own `material: Option<String>`
   (`ChildPrimitiveDef.material`, docs/20 "Composite prefabs (`children`)")

**Verified 2026-08-06 by multiline grep across every `assets/projects/**/*.ron`: no shipped prefab
combines a top-level `material:` with a non-empty `children:` list.** `local_coop_demo`'s
`player_p2_primitive_split_ring` (room10) is the first. So the interaction is both undocumented and
unprecedented — there is no example a designer can copy to learn the rule.

The risk shape: if the top-level override recurses into descendants, a composed body's carefully
authored per-child colours are flattened to one uniform material, destroying exactly the visual
separation the composed body exists to provide. If it does *not* recurse, a parent that sets no
`primitive.color` falls back to `project.primitive_default_color` instead of the intended tint.
Either way one of the two authoring intents silently loses.

**Why:** docs/20's `material` row says only "visual override, applies after the body is built
regardless of model source", and the `ChildPrimitiveDef` table documents a per-child `material`
without saying what wins.

**How to apply:** whenever a prefab review touches both `material:` and `children:`, treat the
precedence as unverified. Prefer the pattern all four `portal_to_roomN` prefabs use — no top-level
`material`, colours authored per child — and ask for a one-sentence precedence rule in docs/20
before accepting a mixed prefab. Related: [[primitive-player-fields]].
