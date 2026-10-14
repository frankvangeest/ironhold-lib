---
name: real-pause-plan-review
description: 2026-10-13 plan-review of planning/features/real_pause.md - verified overlay scene_key_bindings are NOT merged, recommended pause authoring pattern, validate rule shape, audio-ducking footgun, missing doc surfaces
metadata:
  type: project
---

Verdict at `d2191ab`: Needs work (RON surface right, details underspecified).

- **Verified:** overlay loads (`LoadSceneOverlay`/`ToggleOverlay`) only spawn UI; `scene_key_bindings` and
  `scene_unclaimed_gamepad_bindings` are rebuilt ONLY on Replace loads. A key in `pause.scene.ron` is dead,
  yet `unreachable_trigger` validates it as live. docs/20 ~164 does not say so.
- Recommended pattern: `PauseGame` in the paused state's `entry_actions`, `ResumeGame` in the SAME state's
  `exit_actions` (mirrors LoadSceneOverlay/UnloadOverlay). `TogglePause` only for flat `global_on` Escape.
  Pause/Resume idempotent, not a counter.
- Plan's validate rule "a ResumeGame reachable anywhere" is too loose; propose same-state exit check (error),
  TogglePause-in-entry/exit and PauseGame-in-.behavior.ron warnings. Action-bar slot do_actions are a blind spot.
- Footguns: `ToggleMute` in pause entry/exit un-mutes an already-muted player; `StopMusic`/`PlayMusicLoop`
  restarts the track; `EmitEventAfterDelay` armed from the pause menu (fade-then-LoadScene) never fires = soft-lock.
- Joined gamepad players have NO pause route; docs/20 ~3254-3261 already names `"Start": "toggle_pause"` as dead.
  Suggested shape: `global_claimed_gamepad_bindings` (mirror of the unclaimed map). local_coop_demo has no pause.
- Missing doc surfaces in plan: README ~94/101/127, docs/15 Logic-wiring row, docs/20 ~3254 gamepad note,
  docs/20 ~164 overlay caveat, 3rd_person state_machine.ron comments ~177-179 and ~195-197, backlog ~240 caveat item.
  Plan's docs/30 line refs stale (now ~223-257, ~409-450, ~675-678).

**How to apply:** at implementation review, check these items first. Related: [[pause-is-cosmetic]],
[[fixed-tick-pipeline-review]], [[ui-trigger-wiring]], [[gamepad-input-system]], [[docs-lag-actions]].
