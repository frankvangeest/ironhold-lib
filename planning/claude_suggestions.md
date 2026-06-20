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

- ~~**Tune `collider_radius`/`collider_height` on `enemy_snake` and `enemy_spider` prefabs**~~ _(promoted to backlog `0f86e07` 2026-06-17 → Queued ▸ Gameplay & Environment; see `planning/features/collider_tuning_creatures.md`)_

- ~~**Per-prefab `depth_scale: Some(true)` override silently ignored on dynamic spawns**~~ _(promoted to backlog `0f86e07` 2026-06-17 → Queued ▸ Engine / Runtime; see `planning/features/depth_scale_dynamic_spawn.md`)_

- **Extract `assemble_player_config` shared helper to prevent scene-loader/executor drift** _(observed at `d9d0232` 2026-06-16)_ — `PlayerConfig` is now assembled in two places: `scene_loader.rs` (for scene-placed player entities) and `action_executor.rs` (for `Action::Spawn` player-tagged prefabs); a future `PlayerConfig` field added to one site but not the other silently diverges the two paths. Extracting a shared `assemble_player_config(prefab_def, model_path, position, spawn_id) -> PlayerConfig` helper (with the missing-animation_policy warning) would make the two sites identical at the call level.

- **Tune `SELECT_PIXEL_RADIUS` for tighter click-to-select feel** _(observed at `16eccff` 2026-06-17)_ — The current 70 px screen-space selection radius feels loose at close range (can click 1–2 m to the side of an enemy and still select); dropping to 40–50 px would require more deliberate targeting without being too tight for casual play. Concrete basis: play-test observation with hitbox debug overlay showing the mismatch between the visible sphere and the selectable area.

- **Add skybox or procedural atmosphere to remove "all ground" background** _(observed at `d9d0232` 2026-06-16)_ — Without a skybox the ClearColor (dark grey) fills any area not covered by geometry; the large 100×100 sand ground plane covers the entire screen at max camera pitch, making the whole background sandy. A Bevy `EnvironmentMapLight` cube map or a procedural sky dome (gradient blue) would fill the background correctly at any pitch angle and dramatically improve scene readability at steep camera angles.

- **Add collider-tuning doc example + discovery hint for NPC capsules** _(observed at `be229b7` 2026-06-17)_ — `docs/20_data_formats.md` NpcDef table has `collider_radius`/`collider_height` but no worked example in the doc prefab; the `enemy_snake`/`enemy_spider` prefabs are now the only live examples of non-humanoid capsule sizing. Adding a `rat` example to the doc table and a one-liner ("start from the model's visual height, or run `ironhold inspect glb` to read bounds") near line 1288 would make capsule tuning self-discoverable for designers authoring new creatures.

- **`spawn_prefab_instance` capability checklist — new prefab fields silently skip GLB actors** _(observed at `c669ee1` 2026-06-20)_ — Each new `PrefabDef` capability field (e.g. `dialogue`, `motion`, `stat_label`) must be manually added to `spawn_prefab_instance` in `entity_spawner.rs`; omitting it silently applies the field to primitive/composite entities but not to GLB Actors — as happened with `dialogue`. The promoted suggestion to consolidate the spawn sites (#34bc77d) would eliminate this class of bug entirely.

- **Stale `spider.hide:{self}` delay timer could hide a newly-respawned spider** _(observed at `0ddcf9b` 2026-06-19)_ — The `enemy_spider.behavior.ron` "dead" state schedules `EmitEventAfterDelay(event: "spider.hide:{self}", delay_secs: 15.0)`; if the spider respawns (transitions "dead" → "alive") before 15 s elapsed, the entity FSM interpreter has already moved on — the pending delay fires anyway and calls `SetEntityVisible(visible: false)` on the now-alive spider. The cleanest fix is to track the pending delay ID and cancel it on state exit, or to guard the hide handler with a condition (e.g. check that state is still "dead"). Concrete basis: the current delay system has no cancel/guard mechanism; this is a latent edge case for any short respawn_timer.

