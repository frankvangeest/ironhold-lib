# Claude Suggestions

> Raw development-time observations. Frank reviews and promotes good ones to `backlog.md`.
> Format: **title** _(observed at `<hash>` <YYYY-MM-DD>)_ — what + why, both one sentence.

---

## Performance


- ~~**Extend pipeline warmup to cover Text2d and UI pipelines**~~ _(promoted to backlog `e02d9e1` 2026-05-05; see `planning/features/pipeline_warmup_2d_ui.md`)_

- **Particle texture atlas for ≤4 total draw calls** _(observed at `0221d9e` 2026-05-19)_ — The pool renderer gives O(distinct textures) draw calls; the campfire's 6 Kenney flame sprites still cost 6 draw calls — packing them into one atlas PNG at startup (or as a prebuilt asset) and remapping UVs would hit the ≤4 target. Concrete basis: campfire_body uses sprites: [flame_01..04] = 4 groups, campfire_core [flame_05..06] = 2 groups.


## Particles / Visual

- **Glow halo layer on fire effects (SDR-safe "fake bloom")** _(observed at `a16bd98` 2026-05-23)_ — Add a third layer to `campfire_fire` (and similar fire effects): a large (size ~1.5), near-transparent (alpha ~0.06 start, 0.0 end), additive-blend, solid-orange quad with zero spread and slow drift upward; this approximates the soft halo bloom would add without touching HDR or post-processing. Concrete basis: investigated while implementing and reverting Bloom — the existing multi-layer EffectDef system supports this entirely in RON with no code changes.

## Testing

- **Further split `integration_tests.rs` as it grows** _(observed at `c07c1e0` 2026-05-27)_ — `integration_tests.rs` is still 2447 lines / 69 tests after the domain split; as FSM, scene-loading, and spawn-pipeline tests accumulate, splitting into `fsm_tests.rs`, `scene_lifecycle_tests.rs`, and `spawn_tests.rs` would keep individual files under ~30 tests. Concrete basis: current file mixes 6 distinct subsystems with no internal headers separating them.

## Audio

- **Collapse dual `GlobalVolume` write in audio actions** _(observed at `43c5a84` 2026-06-10)_ — `action_executor_system` writes `GlobalVolume` directly after mutating `AudioState`, but mutating `AudioState` also trips `is_changed()`, so `audio_state_system` writes it again the following frame; benign today (idempotent), but two sources of truth — if `GlobalVolume` writes become expensive, collapse to a single writer by having the executor mutate only `AudioState` and letting `audio_state_system` be the sole `GlobalVolume` writer.

## Scene Loading

- ~~**Consolidate the 5 entity-spawn sites behind one "attach standard components" helper**~~ _(observed at `728c997` 2026-06-08; promoted to backlog `34bc77d` 2026-06-08 → Queued ▸ Engine / Runtime)_

- ~~**Insert `PrefabKey` on dynamic `Action::Spawn` spawns**~~ _(observed at `728c997` 2026-06-08; promoted to backlog `34bc77d` 2026-06-08 → Queued ▸ Engine / Runtime)_

- ~~**Consolidate conditional prefab-feature application (the sibling divergence)**~~ _(observed at `cef818a` 2026-06-08; promoted to backlog `cdee26b` 2026-06-08 → Queued ▸ Engine / Runtime)_

- **Push `stat_overrides` into `spawn_prefab_instance` so the StatMap is built once** _(observed at `5df25da` 2026-06-11)_ — The GLB actor path currently builds `StatMap` twice: `spawn_prefab_instance` builds at template defaults, then `scene_loader` overwrites with override values; the feature only works by accidental last-write-wins, not by design. Add `stat_overrides: &HashMap<String, f32>` to `spawn_prefab_instance` (callers without an `entity_def` pass `&HashMap::new()`), move the override-aware build there, and delete the second insert from the scene_loader GLB else-branch.

- **Add optional `collider_radius`/`collider_height` to `NpcDef` for GLB actor sizing** _(observed at `cd1d321` 2026-06-14)_ — GLB NPC physics capsules are currently fixed at 0.35 m / 1.6 m in `entity_spawner.rs::spawn_prefab_instance`; a very large or very small creature gets a mismatched collider with no RON field to correct it. Pattern to follow: `MovementConfig` already uses `collider_radius: Option<f32>` / `collider_height: Option<f32>` with defaults, so the NpcDef change is a direct parallel that keeps minimal RON valid.

- **Add integration test asserting GLB Actor `components.npc` attaches `NpcAgent` + `LocomotionState`** _(observed at `cd1d321` 2026-06-14)_ — The `spawn_prefab_instance` GLB NPC path is new and untested; GLB-actor-missing-component bugs are historically real (see `tag_spawned_entity` notes). A test that spawns `enemy_snake` and queries for `NpcAgent` + `LocomotionState` would prevent silent regressions in this code path.

- **Dynamic `Action::Spawn` entities miss `motion`, `stat_label`, and `world_stat_bar`** _(observed at `5df25da` 2026-06-11)_ — `spawn_prefab_instance` (called by `drain_spawn_queue_system`) handles behavior/stats/interactable/trigger_zone but not motion, stat_label, or world_stat_bar — so a rule-spawned `Spawn(prefab: "enemy_orc_melee")` produces an enemy with no floating health bar or motion, while a scene-placed entity has both. Fix: absorb `motion` into `spawn_prefab_instance` (it's prefab-derived, not entity_def-derived); `stat_label`/`world_stat_bar` need a separate mechanism for dynamic spawns (e.g. an `Added<StatMap>` observer) since they push onto scene-load-time deferred vectors.

