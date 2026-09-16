---
name: time-and-animation-advance
description: How time/clock drives animation, motion, NPC, particles — why "skip our systems" does NOT freeze the world; relevant to any pause/slow-mo/freeze feature
metadata:
  type: project
---

## animation_playback_system only switches clips; Bevy advances them

`capabilities/animation.rs::animation_playback_system` only *selects/transitions* clips (`current != last_played` → `transitions.play(...)`). The per-frame advancement of `AnimationPlayer` is done by **Bevy's own `AnimationPlugin` (advance_animations)**, which ticks off `Time<Virtual>`. Skipping our system does NOT stop characters animating.

**Why it matters:** Any feature that wants to freeze/pause/slow the scene (e.g. static screenshot mode) must act on the **time source**, not on our systems. The highest-leverage lever is pausing `Time<Virtual>` (`time.pause()` / `relative_speed = 0.0`) — it freezes Bevy animation advance, motion, particle sim, and dt-based NPC movement in one move, including third-party systems we don't own. Fully WASM-safe (no threads/blocking).

## motion / npc / particles are clock consumers

`motion_system` (`capabilities/motion.rs`) uses `time.elapsed_secs()`/`time.delta_secs()`. `npc.rs::npc_behavior_system` reads `Res<Time>` (`time.delta_secs()` at ~line 155). Particle sim is dt-driven. All freeze when `Time<Virtual>` is paused.

**How to apply:** For a freeze/static mode, pause the virtual clock as the primary mechanism; only add per-system run-condition guards for the few systems that move per-frame regardless of dt or that do discrete one-shot logic. Don't build a global system-set just for this — the clock pause covers ~90% globally.

## Freeze must happen AFTER async load completes

Terrain gen (AsyncComputeTaskPool), GLB decode, pipeline warmup (`PipelineWarmup` counts down to 0), and audio preload happen post-boot and some key off elapsed time. Pausing the clock at boot risks a half-loaded state / deadlock. Freeze on `SceneEvent::Ready` (+ warmup complete) instead. `Time::pause()`/`unpause()` and `AnimationPlayer.seek_to(0.0)` are cleanly reversible — not a hard-to-resume hazard.

## `Time<Virtual>::max_delta` is a whole-clock knob, not a FixedUpdate-only one

Measured 2026-09-16 (`planning/investigations/fixed_timestep_max_delta.md`): the "spiral of death"
framing is wrong — Bevy's 250ms default clamp already prevents it; under sustained overload the
game degrades to **slow-motion** (falls behind real time) rather than compounding frame cost.
16-tick bursts happen on *every* WASM page load from instantiation+asset time alone and recover
cleanly. `feature/fixed_timestep_max_delta` made it project-authorable
(`ProjectConfig.max_fixed_delta_secs`, applied once in `check_project_loaded`'s shared tail).

**Why it matters:** despite the `max_fixed_*` name, `max_delta` clamps the *virtual clock's* whole
per-frame delta — every `Res<Time>` reader in `Update` (camera smoothing, motion, timers) sees the
clamp too, not just `FixedUpdate` catch-up. Anything reasoning about worst-case `delta_secs()`
(e.g. `MAX_WHEEL_NOTCHES_PER_FRAME`'s "250ms" justification in `capabilities/camera.rs`) is
project-dependent now.

**How to apply:** `AppState::LoadingProject` is entered exactly once (`lib.rs`, the
`ProjectConfigHandle` setup system) and never re-entered on scene load — so a one-shot
`set_max_delta` at project load cannot be silently reverted, and nothing else in core re-inserts
`Time<Virtual>` or touches `pause()`/`set_relative_speed()`. Note `advance_with_raw_delta` clamps
*before* applying `relative_speed`, so if slow-mo/fast-forward is ever added, effective catch-up =
`max_delta × relative_speed`.

## Test/tooling modes use URL-param → start_app → Resource, not RON

Pattern: `ironhold_web/src/lib.rs::read_url_params` reads `?project=` and `?scene=`, threads them into `start_app(project, scene)`, which inserts `InitialSceneOverride`. Harness-driven modes (static/frozen, etc.) should follow this same path (new `?param` → new `start_app` arg → new `Resource`), NOT a scene/project RON flag — the mode is ephemeral and harness-controlled, and the same content must run live in the browser and frozen under test. Changing `start_app`'s signature is a small breaking change touching all three crates in one commit. See [[arch_decisions]].
