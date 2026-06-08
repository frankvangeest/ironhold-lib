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

## Scene Loading

- ~~**Consolidate the 5 entity-spawn sites behind one "attach standard components" helper**~~ _(observed at `728c997` 2026-06-08; promoted to backlog `34bc77d` 2026-06-08 → Queued ▸ Engine / Runtime)_

- ~~**Insert `PrefabKey` on dynamic `Action::Spawn` spawns**~~ _(observed at `728c997` 2026-06-08; promoted to backlog `34bc77d` 2026-06-08 → Queued ▸ Engine / Runtime)_

- **Consolidate conditional prefab-feature application (the sibling divergence)** _(observed at `cef818a` 2026-06-08)_ — After `tag_spawned_entity` unified the *metadata* set, the *feature* set is still applied per-path: `interactable` / `trigger_zone` / `behavior` / `stat_templates` are inserted in `spawn_prefab_instance` (`entity_spawner.rs:53-74`) for GLB prefabs but re-implemented inline in the single-mesh and composite primitive branches (`scene_loader.rs`). Concrete basis: this is the same "works for one kind, silently missing for another" bug class one level down — a future helper (taking `&PrefabDef`) applied at the primitive branches and inside `spawn_prefab_instance` would close it; deliberately left out of the metadata-consolidation commit to keep that change's boundary clean (per system-architect review).

