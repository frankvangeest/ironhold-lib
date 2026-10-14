---
name: project-real-pause
description: real_pause (SimClock -> Time<Virtual>::pause) review 2026-10-14; paused virtual time stops FixedMain so Bevy message buffers are never swapped -> unbounded growth while paused; gates themselves are O(1)
metadata:
  type: project
---

real_pause (feature/real_pause, reviewed 2026-10-14): `runtime/sim_clock.rs` SimClock (BTreeSet of holds) is the only path to
`Time<Virtual>::pause()`; `apply_sim_clock_system` is `run_if(resource_changed::<SimClock>)` in the interpreter->executor chain.
Executor arms and spawn_scene_v2 check via Deref first (`is_held`) so idle frames do not mark it changed -> free when idle.

**Bevy 0.18 trap (verified in bevy_time-0.18.0/src/lib.rs:92-95 + bevy_ecs message/update.rs:49):** TimePlugin sets
`MessageRegistry.should_update = Waiting`, only `signal_message_update_system` in FixedPostUpdate flips it to Ready. Paused
`Time<Virtual>` -> zero FixedMain runs -> `message_update_system` never runs -> EVERY `Messages<T>` (PointerHits from ui+sprite
picking backends each frame, CursorMoved/MouseMotion/WindowEvent, AssetEvent<Mesh> from particle rebuild, UiEvent, GameEvent)
grows for the whole pause. WASM linear memory never shrinks; Vec doubling copies = hitches on long pauses.
Recommended fix: a `Last` system that sets `should_update = Ready` while `!SimClock::is_running()` (FixedUpdate message
consumers in this engine are FixedUpdate-written — InputActionMessage, CollisionEvent — so dropping paused Update messages is safe).

**Why:** pause is a state players leave idle for minutes; reviewers will meet this again in static_scene_mode / FocusLoss holds.
**How to apply:** any future feature that holds Time<Virtual> paused or relative_speed 0 must check message buffer clearing.
Also: while paused, rebuild_pool_meshes_system keeps re-tessellating frozen particles every frame (simulate_pool iter_mut marks
ParticlePool changed even at dt=0); animation graph still evaluated (see [[project-animation-hot-path]]). Not regressions.
Demo RON moved run-start actions (PlayMusicLoop, PreloadGlb x12, SpawnEffect warm-ups) from playing.entry to global_on
scene.ready:main -> same frame, net win (previously re-ran per unpause, pushing duplicate preloaded_glbs handles each time).
