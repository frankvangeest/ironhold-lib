---
name: Targeting capability + {target} substitution pattern
description: How the targeting capability (click/Tab selection) wires into the data-driven pipeline; the SetTarget/capability GameVariable asymmetry footgun; consult when reviewing targeting or CurrentTarget changes
type: project
---

The targeting capability (`capabilities/targeting.rs`) selects entities by **screen-space projection** (`camera.world_to_viewport`), NOT mesh raycasting — deliberate, because Bevy mesh picking raycasts bind-pose geometry and misses animated/skinned GLB characters. `SELECT_PIXEL_RADIUS` (70px) and `SELECT_AIM_HEIGHT` (1.0m) are hardcoded constants; acceptable as engine UX tuning (not game-content config) but candidates for schema fields if a designer ever needs per-project tuning.

**Designer entry points (all RON-reachable):**
- `PrefabDef.click_selectable: bool` and `PrefabDef.targetable: bool` (both `#[serde(default)]`, backward-compatible).
- `InputMap.target_next: String` (default "Tab") and `InputMap.target_range: f32` (default 30.0) in the player prefab `inputs` block. Note: browsers intercept Tab for focus, so web projects should override to e.g. `"KeyT"`.
- `Action::SetTarget(String)` / `Action::ClearTarget` (tuple/unit variants → positional RON).
- `{target}` substitution in `rewrite_target()` (message_interpreter.rs) — fills the current `CurrentTarget` spawn ID into action fields. Runs in all 3 interpreter systems (rules, FSM, entity-FSM). Supported fields mirror `{self}`: key, entity, event, id, spawn_point, and SetVariable's value.

**Marker insertion — three spawn paths (same footgun as TriggerZone/PrefabKey):**
`click_selectable`/`targetable` markers + `PrefabKey`(catalog key, distinct from `SpawnId` instance id) must be inserted in ALL THREE scene_loader.rs branches: composite primitive, single-mesh primitive, and GLB actor/prop. The GLB actor branch historically omitted `SpawnId`+SpawnRegistry registration entirely — verify it inserts `SpawnId`, `PrefabKey`, AND `spawn_registry.entities.insert(...)`, or targeting + all id-targeted actions silently miss GLB scene entities.

**ASYMMETRY FOOTGUN (flag if reintroduced):** The capability writes three GameVariables directly (`target_display`, `target_name`, `target_id`) via `write_target_vars`/`clear_target_vars` so UI labels update without rule wiring (same self-managed-state pattern as action_bar). BUT `Action::SetTarget`/`ClearTarget` in action_executor.rs only set `CurrentTarget` + emit events — they do NOT write those GameVariables. So a rule-driven `SetTarget("orc_01")` updates the target but leaves a `bind: "target_display"` label stale, while click/Tab selection updates it. Either route SetTarget/ClearTarget through the same var-writing helper, or document that designers must pair `SetTarget` with `SetVariable("target_display", ...)`. This is a designer-reachability gap: the same logical outcome (set target) produces different UI depending on whether it came from input or from a rule.

**GLB player movement footgun:** the GLB player path (`entity_spawner.rs::spawn_player_entity`) must insert `SpeedMultiplier(1.0)` — `player_movement_system`'s query requires it, and without it the GLB player is silently filtered out and never moves. The primitive player path inserts it in scene_loader.rs. Both paths must stay in sync.
