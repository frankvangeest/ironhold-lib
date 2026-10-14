---
name: real-pause-pattern
description: Real pause (SimClock/PauseGame/ResumeGame/TogglePause, game.paused/resumed, global_claimed_gamepad_bindings) - plan facts plus the 2026-10-14 implementation-review findings and recurring doc traps
metadata:
  type: project
---

Plan-reviewed 2026-10-13 (Needs work), implementation-reviewed 2026-10-14 on `feature/real_pause` (uncommitted diff).

**Design facts (verified in code):**
- `SimClock` (`runtime/sim_clock.rs`) BTreeSet holds; `apply_sim_clock_system` compares want vs `Time<Virtual>::is_paused()`
  so `game.paused`/`game.resumed` fire only on a real change (ResumeGame+PauseGame same frame = no event).
- Auto-resume lives in `spawn_scene_v2` Replace branch, NOT the executor `LoadScene` arm. Consequence: `[LoadScene(x),
  PauseGame]` in one list does NOT start x paused (PauseGame holds this frame, the load releases it frames later). The
  plan and docs/30 claimed the opposite; the pause_tests test of that name only pauses after the load. To start a scene
  paused: `scene.ready:x -> PauseGame`.
- validate has NO engine-event allowlist: `check_orphan_event` only inspects `ui.button_pressed:*`, so bindings on
  `game.paused`/`audio.muted`/etc. are never flagged. Don't ask for an allowlist entry for new GameEvent names.
- `collect_reachable_ui_triggers` must include every binding map (global_claimed_gamepad_bindings was added) or
  orphan_binding false-positives.
- All 7 pause checks are `--strict`-only (validate has no non-strict warning tier; errors only).
- Run-start actions moved from `playing.entry_actions` to a `global_on` `scene.ready:main` rule in both demos; StopMusic
  moved to menu entry (3rd_person) / game_over entry (primitive_world). global_on runs BEFORE in-state `on:` Spawn.

**Recurring doc traps for pause:** docs/20 EmitEventAfterDelay row and docs/dev/lootable-corpse.md said "keeps counting
while paused" (stale after real pause); docs/30 second FSM example + STATUS.md example still put PlayMusicLoop in
playing.entry/StopMusic in exit (the free-heal anti-pattern); docs/30 event catalog (~L121) lacked game.paused/resumed;
new test file (pause_tests) not added to root CLAUDE.md test loop. No shipped project demos TogglePause or
global_claimed_gamepad_bindings.

Older plan facts: first-person mouse yaw rotates the player Transform (needs gate); overlay `scene_key_bindings` are dead
data (rebuilt only in Replace branch); `panels_open` must stay a separate condition from pause.

Related: [[intent-event-layer-pattern]], [[gamepad-hot-join-pattern]], [[rules-vs-state-machine-coexistence]],
[[fixed-timestep-det-math-pattern]], [[validate-cross-file-blind-spots]].
