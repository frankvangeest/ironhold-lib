---
name: real_pause_review_facts
description: Verified facts from real_pause.md plan review (d2191ab) + post-impl review (2026-10-14) — what a Time<Virtual> pause freezes/misses, resume point, hold-set owner, message-buffer no-flip while paused, [LoadScene, PauseGame] ends UNpaused
metadata:
  type: project
---

Verified at `d2191ab` (2026-10-13) during plan-review of `real_pause.md` (verdict: Needs work).

- Virtual paused → `run_fixed_main_schedule` accumulates 0 → **zero FixedUpdate ticks**: input_translator, player_movement, collectible, trigger_zone, npc, motion, gamepad_bind and Rapier all freeze with no gate. Unpause gives no catch-up burst (paused real time is discarded; only overstep < 1 period survives).
- **NOT frozen by the clock (mouse look is not dt-scaled):** `first_person_camera_system` (camera.rs ~577) rotates the *player's* `Transform` from raw mouse delta, so it changes sim state while paused; `fly_camera_system` look (flycam.rs). Orbit/Party (incl. wheel zoom) are dt-scaled and do freeze.
- `tick_delayed_events_system` reads `Res<Time>` in Update, which is Virtual, so it DOES freeze. The "no pause gate" note means no *LogicState* gate.
- Gameplay systems are **not AppState-gated**, so the old world keeps simulating during `LoadingScene`. Auto-resume belongs in `spawn_scene_v2`'s Replace branch (teardown), not in the executor's `LoadScene` arm.
- Overlay loads never rebuild `LoadedKeyBindings`/`LoadedGamepadBindings` (Replace branch only, scene_loader.rs ~134-164). An overlay's `scene_key_bindings` are silently ignored.
- `action_executor_system` sits at the 16-param ceiling. New resources go into `SceneStateParams`. `dialogue_tick_system` is now also at 16 (post real_pause).
- Pause state must be an idempotent hold *set* (Menu/Static/FocusLoss), not a bool and not a counter (`panels_open` is the counter-leak precedent). The input gate reads the hold set, never `Time<Virtual>::is_paused()`.
- A claimed-pad trigger binding needs a once-per-frame dedupe per trigger: two players pressing Start in the same frame means two `toggle_pause` events, so the toggle cancels itself out. (Dedupe is per-map only: keyboard Escape + pad Start in one frame still double-toggles.)

Post-implementation review (2026-10-14, feature/real_pause worktree, uncommitted):
- **I was wrong in the plan review:** `[LoadScene(x), PauseGame]` does NOT start the new scene paused. Both execute in the same executor drain; `spawn_scene_v2` (Replace branch) runs on a later frame and releases `Menu`. Net: old world frozen during LoadingScene, new scene running. Same for "pause pressed mid-load". To start a scene paused, pause from `scene.ready:<x>`.
- **Message buffers never flip while paused** (bevy_time 0.18 sets `should_update = Waiting`; only `signal_message_update_system` in FixedPostUpdate re-arms it, and 0 fixed ticks run). Consequences: every `Messages<T>` (MouseMotion, CursorMoved, UiEvent, GameEvent…) grows for the whole pause; and any `run_if`-gated Update reader would **replay the whole pause backlog** on resume (the reverse of the usual "gated reader loses messages"). Shipped gates are safe only because gated systems read `ButtonInput` state, not messages — the drain-before-return rule matters for future `PlayerInputSet` members.
- In-body gate (not `run_if`) for `action_bar_input_system` is correct: an early return still advances `last_run`, so a `Ref<Interaction>` Pressed during pause is not "changed" after resume.

Related: [[fixed_tick_pipeline_facts]], [[time_and_animation_advance]], [[schedule_update_vs_fixedupdate]], [[gamepad_input_pattern]].
