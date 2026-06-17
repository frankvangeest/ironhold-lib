---
name: npc-collider-canonical-example
description: Canonical worked example for NpcDef collider_height/collider_radius is the 3rd_person_game_demo snake/spider prefabs, NOT the docs' own examples
metadata:
  type: project
---

NpcDef `collider_height` / `collider_radius` (Option<f32>, default None → 0.35 m radius / 1.6 m height humanoid capsule) are documented in docs/20_data_formats.md (table rows ~1287-1288, callout ~1257) but the doc's own `orc_guard` and `rat` examples do NOT set them.

The only live worked example of these fields in use is `assets/projects/3rd_person_game_demo/prefabs/prefabs.ron`:
- `enemy_snake`: collider_height 0.8, collider_radius 0.3 (low ground-hugging body)
- `enemy_spider`: collider_height 1.2, collider_radius 0.4 (taller creature)

**Why:** Recurring pattern in this repo — fields land in schema + docs table, but the documented *examples* lag behind real example projects (see [[project_docs_lag_actions]]). Designers tuning their own creature have a table entry but no copyable example pairing the field with a visible result, and no documented loop for discovering the right value for a given GLB (inspect glb reports mesh bounds but docs don't link the two).

**How to apply:** When reviewing NPC/collider changes, point designers at the snake/spider prefabs as the short-vs-tall pair. If asked to improve, recommend adding the fields to the doc's `rat` example and a cross-reference + a "start from visual height" discovery hint near the table.
