# Cursor capture, the pointer-lock report and `hide_own_body`

Long-form notes for `FirstPersonCameraDef.capture_cursor` / `hide_own_body` and the right-stick ownership rule. The hard rules are
in `crates/ironhold_core/src/capabilities/CLAUDE.md` ("Cursor capture and the right stick (first person)"). Design history and the
decisions behind this: `planning/features/first_person_starter.md` ("Spike A result" and "Decisions made").

## Why the runner reports the lock state

Bevy 0.18 exposes the grab as the `CursorOptions` component on the window (`grab_mode`, `visible`). `bevy_winit` applies it only on
`Changed<CursorOptions>`, tries `Confined` then falls back to `Locked` (and the reverse) in `attempt_grab`, and notes that the grab "can
change through external means, leaving the cache outdated". On the web, winit 0.30 implements the grab as
`canvas.request_pointer_lock()` / `document.exit_pointer_lock()` (`web/web_sys/canvas.rs`), **always returns `Ok`**, treats `Confined` as
unsupported, and has **no `pointerlockchange` or `pointerlockerror` listener**. Consequences:

- a request the browser refuses (no recent user activation, Chrome's re-lock delay of about a second after an Escape exit) fails
  silently;
- Escape releases the lock without any event reaching Bevy, and the page never sees the Escape key press;
- `CursorOptions.grab_mode` therefore stays `Locked` while the real state is unlocked, and re-grabbing needs a write that marks the
  component changed (assigning through `DerefMut` does, even for an equal value).

So `ironhold_web` (`install_pointer_lock_reporting`) registers `pointerlockchange`/`pointerlockerror` listeners and calls
`ironhold_core::capabilities::report_pointer_lock(bool)`; core keeps the answer in a process-wide atomic (the listeners have no handle to
the Bevy `World`; on wasm32 everything runs on one thread) and `cursor_capture_system` copies it into `CursorLockState` each frame. A runner
that never calls `runner_reports_pointer_lock()` (native) makes the system set `CursorLockState.locked` itself right after it writes the
grab: native winit grabs synchronously.

## The system's behaviour

- Wanted = some `FirstPersonCameraMode` camera has `capture_cursor: true` (and, with the `inspector` feature, the inspector is not open).
- Wanted and not paused: the first left click (the user gesture) writes `grab_mode: Locked, visible: false`. With a reporting runner a
  request that was never confirmed is repeated on the next click.
- Not wanted, or paused: the cursor is released (`None`, visible) and the request forgotten. After a pause one click captures again.
- A lock the engine asked for, had, and no longer has, with no release from the engine, emits `GameEvent::Trigger("cursor.lock_lost")`
  once. The project decides what that means (the starter pauses in RON). An engine-owned `SimClock` hold was rejected: `ResumeGame` only
  releases the `Menu` hold, so a designer could never release it.
- The test hook `reset_pointer_lock_report` and the mutex in `tests/cursor_capture_tests.rs` exist because the report channel is
  process-wide.

## Right-stick ownership

Stick-X `Turn` comes from `input_translator_system` (`FixedUpdate`); stick look lives in `first_person_camera_system` (`Update`). To avoid
turning the character twice the translator skips the `Turn` for any player targeted by a camera carrying `FirstPersonCameraMode`
(`apply_camera_mode` inserts and removes that component). It is decided from the live camera and not from `InputMap.controls` because
`SetCameraMode` can change the camera at runtime, which `validate` cannot see.

## `hide_own_body`

`Visibility::Hidden` on the player root, inserted in the spawn bundle in `spawn_player_entity_core` (so there is no first-frame flash: a
`scene.ready` rule always shows at least one frame of the inside of the head), plus the `OwnBodyHidden` marker. `assemble_player_config`
also drops the nameplate, stat label and world stat bar. `Action::SetCameraMode` hides the body when a hiding first-person camera takes
over and un-hides it, only if the marker is present, when the camera leaves. `RenderLayers` was rejected: a GLB's descendants spawn
asynchronously and a non-zero layer loses layer-0 light shadows anyway. Limits: the body casts no shadow; `Action::SetEntityVisible` on
the player overrides it (last writer wins); split-screen first person would need per-camera layers (out of scope).
