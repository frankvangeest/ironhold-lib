---
name: project_particle_warmup_recipe
description: docs/20 "Warming up particle pipelines (web builds)" recipe (added 2026-10-08) - canonical shipped examples, the 3 pipeline kinds mapped to designer fields, and its traps
metadata:
  type: project
---

docs/20 ~1869 recipe: fire one off-screen `SpawnEffect` (y=-100) per particle pipeline kind used. Kinds (pooled renderer groups by GroupKey): Additive (no sprite, or sprite + `additive: true`), Blend (sprite with `additive` omitted/false - the DEFAULT for sprites), Flame (`uv_distort` or `uv_scroll_speed` > 0). Particle group entities carry NoFrustumCulling, so off-screen y=-100 does compile them.

Canonical shipped examples: `particles_demo/logic/state_machine.ron` (global_on `scene.ready:main`, also warms the DECAL pipeline via ProjectDecal - the doc recipe omits decals) and `3rd_person_game_demo/logic/state_machine.ron`. `primitive_world/assets.ron` is the only project containing one effect of each kind (hit_spark / campfire_smoke / campfire_fire). There is no `campfire_body` key anywhere.

Traps flagged in review 2026-10-08: snippet said "playing state entry_actions" (initial_state entry is skipped, see [[project_fsm_designer_traps]]) while shipped demos use scene.ready; `Ambient`-priority warmup effects are silently skipped when the budget is full, so the warmup silently does nothing. particles_demo's RON comment still says "four variants" with sphere/FlameParticleMaterial wording.
