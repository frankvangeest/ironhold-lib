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

- ~~**Push `stat_overrides` into `spawn_prefab_instance` so the StatMap is built once**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Active ▸ Engine / Runtime)_

- ~~**Add optional `collider_radius`/`collider_height` to `NpcDef` for GLB actor sizing**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Active ▸ Engine / Runtime)_

- ~~**Add integration test asserting GLB Actor `components.npc` attaches `NpcAgent` + `LocomotionState`**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Active ▸ Engine / Runtime)_

- ~~**Dynamic `Action::Spawn` entities miss `motion`, `stat_label`, and `world_stat_bar`**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Queued ▸ Engine / Runtime; see `planning/features/dynamic_spawn_components.md`)_

- **Tune `collider_radius`/`collider_height` on `enemy_snake` and `enemy_spider` prefabs** _(observed at `36dd927` 2026-06-14)_ — The snake and spider currently use the default 0.35 m / 1.6 m humanoid capsule; `snake01.glb` is a low ground-hugging model and its 1.6 m capsule will visibly mismatch its body, potentially blocking approach at `approach_distance: 1.5 m`. Tune after in-game observation — suggested starting values: snake `collider_height: 0.8, collider_radius: 0.3`; spider `collider_height: 1.2, collider_radius: 0.4`.

- **Per-prefab `depth_scale: Some(true)` override silently ignored on dynamic spawns** _(observed at `7e9eb47` 2026-06-15)_ — `StatLabelDef.depth_scale` and `WorldStatBarDef.depth_scale` let designers force depth scaling on a per-label basis, but `drain_dynamic_stat_ui_system` hardcodes `depth_scale: None` because no scene context is available; the fix is to carry a resolved `Option<(f32,f32)>` into `DynamicStatUiEntry` by storing the current scene's `label_depth_scale` in a resource (e.g. `LoadedLabelDepthScale`) at scene load time so the dynamic path can read it.

