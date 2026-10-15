---
name: project-pipeline-system-sets
description: D3 GameplaySet/EmitSet/GameStreamSet in runtime/schedule.rs; one extra ApplyDeferred (Scene->{Ui,Game}); web toposort reshuffle effects (pipeline_warmup earlier, preload_* same-frame)
metadata:
  type: project
---

D3 (feature/pipeline_system_sets, reviewed 2026-10-14) puts the Update pipeline in sets: Emit{Scene,Ui,Game,DirectActions,Dialogue} -> Interpret -> Execute -> PostExecute. No set-level run_if anywhere, so the single-threaded executor's `sets_with_conditions_of_systems` loop stays empty (zero per-frame set overhead).

Facts verified in bevy_ecs 0.18 source:
- `ExecutorKind` default is SingleThreaded on wasm32 (executor/mod.rs); runs `schedule.systems` in toposort order.
- Auto sync points are deduped **per dependency distance** (auto_insert_apply_deferred.rs `get_sync_point(distance)`), so a new normal edge adds a node only if it creates a new distance level.
- An ApplyDeferred node = FixedBitSet `unapplied_systems.ones()` scan + `apply_deferred` per system run since last sync; each system is applied once per frame regardless, so an extra node only *redistributes* work (+ O(words) scan). Free on web; on native it is a parallelism barrier.
- Proxy (MinimalPlugins) count 7 -> 8 ApplyDeferred; Execute->PostExecute merged into the existing spawn_decal distance (no extra node).

Web order side effects (proxy dump): `pipeline_warmup_system` moved from after all syncs to before the first one -> `PipelineWarmup(4)` (inserted via commands by spawn_scene_v2) is seen one frame later; window length unchanged (3 marking frames). preload_audio/decals now read SceneEvent::Ready the SAME frame (were one frame late on web). All stat widgets now uniformly run before the stat chain (were mixed).

**How to apply:** when reviewing new set membership, count distance levels not edges; re-dump web order (`schedule.systems()` with SingleThreaded) since any set/edge reshuffles unconstrained systems. Related: [[project-real-pause]], [[project-perf-priority-list]].
