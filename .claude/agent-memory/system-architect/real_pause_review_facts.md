---
name: real_pause_review_facts
description: Verified facts from the 2026-10-13 re-review of planning/features/real_pause.md (at d2191ab) — what a Time<Virtual> pause does and does NOT freeze, the resume point, the clock-owner shape
metadata:
  type: project
---

Verified at `d2191ab` (2026-10-13) during plan-review of `real_pause.md` (verdict: Needs work).

- Virtual paused → `run_fixed_main_schedule` accumulates 0 → **zero FixedUpdate ticks**: input_translator, player_movement, collectible, trigger_zone, npc, motion, gamepad_bind and Rapier all freeze with no gate. Unpause gives no catch-up burst (paused real time is discarded; only overstep < 1 period survives).
- **NOT frozen by the clock (mouse look is not dt-scaled):** `first_person_camera_system` (camera.rs ~577) rotates the *player's* `Transform` from raw mouse delta, so it changes sim state while paused; `fly_camera_system` look (flycam.rs). Orbit/Party are dt-scaled and do freeze.
- `tick_delayed_events_system` reads `Res<Time>` in Update, which is Virtual, so it DOES freeze. The "no pause gate" note means no *LogicState* gate.
- Gameplay systems are **not AppState-gated**, so the old world keeps simulating during `LoadingScene`. Auto-resume belongs in `spawn_scene_v2`'s Replace branch (teardown), not in the executor's `LoadScene` arm.
- Overlay loads never rebuild `LoadedKeyBindings`/`LoadedGamepadBindings` (Replace branch only, scene_loader.rs ~134-164). An overlay's `scene_key_bindings` are silently ignored.
- `action_executor_system` sits at the 16-param ceiling. New resources go into `SceneStateParams`.
- Pause state must be an idempotent hold *set* (Menu/Static/FocusLoss), not a bool and not a counter (`panels_open` is the counter-leak precedent). The input gate reads the hold set, never `Time<Virtual>::is_paused()`.
- A claimed-pad trigger binding needs a once-per-frame dedupe per trigger: two players pressing Start in the same frame means two `toggle_pause` events, so the toggle cancels itself out.

Related: [[fixed_tick_pipeline_facts]], [[time_and_animation_advance]], [[schedule_update_vs_fixedupdate]], [[gamepad_input_pattern]].
