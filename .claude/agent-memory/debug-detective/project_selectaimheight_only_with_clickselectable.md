---
name: selectaimheight-only-with-clickselectable
description: tag_spawned_entity inserts SelectAimHeight ONLY when click_selectable; a targetable-only prefab's select_aim_height is silently ignored (1.0 used)
metadata:
  type: project
---

`runtime/scene_manager/mod.rs::tag_spawned_entity` inserts `SelectAimHeight(select_aim_height)` inside the
`if click_selectable` branch. Any consumer querying `With<Targetable>` + `Option<&SelectAimHeight>` (e.g.
`aim_target_system`, first_person_starter v2) falls back to `SELECT_AIM_HEIGHT` = 1.0 for a prefab with
`targetable: true` but no `click_selectable`, ignoring its authored `select_aim_height`. Verified 2026-10-22 on
feature/first_person_starter_v2: training_dummy `select_aim_height: 0.0` -> aim point 0.1 m above a 1.8 m capsule.
Tests that spawn `Targetable` by hand (aim_target_tests.rs) never go through tag_spawned_entity, so they mask it.

**Why:** silent designer-field drop; aim point/catch circle land off the body.
**How to apply:** any new aim/LOS feature keyed on `Targetable` — check the marker insertion site, and demand a
test that spawns through the real prefab path. Related: [[action-spawn-rejects-primitive-prefabs]].
