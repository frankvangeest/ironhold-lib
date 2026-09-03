---
name: project-test-harness-just-pressed-latch
description: setup_test_app() omits InputPlugin, so ButtonInput<KeyCode>::just_pressed latches forever — release() alone does not clear it and silently makes keyboard-input tests vacuous
metadata:
  type: project
---

`crates/ironhold_core/tests/support/mod.rs::setup_test_app()` builds its app from `MinimalPlugins` + `init_resource::<ButtonInput<KeyCode>>()` and **never adds `bevy::input::InputPlugin`**. That plugin is what registers the `ButtonInput::<KeyCode>::clear` system in `PreUpdate`, so in the integration-test harness `just_pressed` is never cleared between `app.update()` calls — a single `.press(KeyCode::X)` reads as `just_pressed` on *every* subsequent update for the rest of the test.

`ButtonInput::release()` does **not** fix this: it clears `pressed` and sets `just_released`, leaving `just_pressed` set. The only correct teardown is `release(key)` **followed by** `clear_just_pressed(key)` — the idiom pre-existing tests in `local_coop_tests.rs` use, with an in-line comment documenting the trap.

Gamepads are **not** affected: `gamepad_event_processing_system` (registered explicitly in `setup_test_app`) clears each `Gamepad`'s digital state every frame, so gamepad `just_pressed` is correctly one-frame. Only the keyboard latches. `ui_panel_blocker.rs` builds its own app *with* `InputPlugin`, so it doesn't have the problem either.

**Why:** caught during the gamepad-routed action-bar-slots review (2026-07-31) — a new test pressed a key, called `release()` without `clear_just_pressed`, then asserted that a *gamepad* press fired the slot. The stale keyboard bit satisfied the assertion, so the test passed with the gamepad binding removed entirely. This is a silent false-pass, not a failure, which is why it survives review.

**How to apply:** whenever a test in `crates/ironhold_core/tests/` presses a `KeyCode` and then runs more than one `app.update()`, treat every later assertion as suspect until you confirm `clear_just_pressed` was called. The falsification test is cheap and decisive: neuter the thing under test (set the component field to `None`, remove the device) and re-run — if it still passes, the assertion was riding the latch. (This file used to also link to `project_gamepad_index_routing.md` — that memory was deleted since the positional `resolve_gamepad` lookup it described was removed and replaced by `BoundGamepad`/`gamepad_bind_system`; unrelated to this file's keyboard-latch finding regardless.)
