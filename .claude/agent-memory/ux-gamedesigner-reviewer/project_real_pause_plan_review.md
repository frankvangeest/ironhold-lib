---
name: real-pause-plan-review
description: real_pause plan-review (2026-10-13) + implementation review (2026-10-14) - what shipped, remaining stale "timers use real time" doc lines, validate LoadScene-as-way-out false negative, strict-only soft-lock checks
metadata:
  type: project
---

Plan-review verdict at `d2191ab`: Needs work. Implementation review (feature/real_pause, uncommitted, 2026-10-14): Needs work (small).

Shipped surface: `PauseGame`/`ResumeGame`/`TogglePause` (unit), `game.paused`/`game.resumed` events,
`global_claimed_gamepad_bindings` (project only), Replace-`LoadScene` auto-resume, docs/30 "Pausing the game"
section (~269-312), 7 `--strict`-only validate codes in docs/15 (~166-172), both demos moved run-start actions to
`global_on` `scene.ready:main`.

Open at implementation review (check these first next time):
- Stale "timers use real time" lines: docs/20 `EmitEventAfterDelay` row ("keeps counting while paused"),
  docs/20 + docs/30 ~825 `SetDespawnTimer` "real time".
- Copy-paste examples still teach `playing.entry_actions: [PlayMusicLoop]` + `exit_actions: [StopMusic]`
  (docs/30 schema-pattern ~474, STATUS ~155 with only a caveat comment) - contradicts the new rule.
- `game.paused`/`game.resumed` missing from the docs/30 GameEvent list (~96-130) and docs/20 event table.
- `unrecoverable_pause` treats a `LoadScene` in ANY own `on:` binding as a way out -> pause menu with Resume
  transition + "Main menu" LoadScene button and no ResumeGame validates clean but Resume leaves the game frozen.
- All 7 checks strict-only; soft-lock-grade ones (`unrecoverable_pause`, `pause_without_resume`) silent in plain validate.
- docs/30 "HUD button keeps firing under the overlay" contradicts docs/20/30 "backdrop blocks clicks" - needs verification.
- 3rd_person paused comment points at "audio.* actions" for ducking - none exist.

Verified (plan stage): overlay `scene_key_bindings`/`scene_unclaimed_gamepad_bindings` are NOT merged (Replace loads only).

**How to apply:** on any follow-up touching pause/timers, grep "real time" and "keeps counting". Related:
[[pause-is-cosmetic]], [[docs-lag-actions]], [[validate-coverage-gaps]], [[gamepad-input-system]].
