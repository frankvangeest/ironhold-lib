---
name: project-effectdef-layers
description: EffectDef gained a `layers: Vec<LayerDef>` field; when non-empty, flat fields are ignored and each layer spawns independently at the same origin
metadata:
  type: project
---

`EffectDef` (in `assets.ron`) now supports a `layers: Vec<LayerDef>` field. Each layer has the same fields as a flat `EffectDef` (minus `layers` itself). When `layers` is non-empty, all flat top-level fields are ignored and each layer is emitted independently at the same origin.

Both single-layer (existing) and multi-layer formats remain valid. Defaults: `particle_count: 12`, `lifetime_secs: 1.0`, `color_start` white, `color_end` transparent.

**Canonical multi-layer example:** `assets/projects/particles_demo/assets.ron` (`"campfire_fire"` key — body + core layers).
**Canonical single-layer example:** `assets/projects/primitive_world/assets.ron` (`"campfire_fire"` key — flat fields).

Note: the same `campfire_fire` name is used in both single-layer and multi-layer forms across different projects. That naming collision is intentional (each project owns its own catalog), but designers copy-pasting between projects need to look at the surrounding shape (`layers: [...]` vs flat fields), not just the key.

**Why:** Lets designers compose multiple emitter behaviors (fire body + hot core) under one catalog key, avoiding two `SpawnEffect` calls and keeping the behavior file simple.

**How to apply:** When reviewing new effect definitions, flag any new field added to `LayerDef` that isn't also added to `EffectDef` (and vice-versa), since the doc says they accept the same fields. Also flag any single-layer effect that sets both `layers` and flat fields — the flat ones are silently ignored.
