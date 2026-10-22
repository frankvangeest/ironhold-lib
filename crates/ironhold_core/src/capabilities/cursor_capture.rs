//! Cursor capture for first-person cameras (`FirstPersonCameraDef.capture_cursor`).
//!
//! `planning/features/done/first_person_starter.md`, "Spike A result": on the web, winit implements the
//! grab as a fire-and-forget `canvas.request_pointer_lock()` that always reports success and has
//! no `pointerlockchange` listener, so Bevy's `CursorOptions.grab_mode` cannot tell the engine
//! whether the browser really granted the lock, nor that the user released it with Escape. The
//! web runner therefore reports the real state through [`report_pointer_lock`], and this module
//! turns it into the platform-agnostic [`CursorLockState`] resource. **`ironhold_core` never
//! touches the DOM**: the runner pushes, core reads.
//!
//! - **Native** (no runner report): winit's grab is synchronous and reliable, so this system sets
//!   [`CursorLockState::locked`] itself right after it changes the grab.
//! - **Web** (the runner called [`runner_reports_pointer_lock`]): `locked` is whatever the runner
//!   last reported, and this system never sets it.
//!
//! The system grabs and hides the cursor on the first left click while a `FirstPerson` camera with
//! `capture_cursor: true` is active (browsers need a user gesture for pointer lock), releases it
//! while the simulation is paused (so the pause menu is clickable) or the camera mode changes, and
//! emits `GameEvent::Trigger("cursor.lock_lost")` when a lock it asked for disappears without the
//! engine releasing it (the browser's Escape). A project maps that event to `PauseGame` in RON.
//! Mouse look is gated on `CursorLockState::locked` in `first_person_camera_system`.
//!
//! Cosmetic window state only: this system never pushes to the `ActionQueue`.
//!
//! **It is the only writer of `CursorOptions` in the engine.** The release branch below writes whenever
//! the cursor is grabbed or hidden while no capture is wanted, so a second writer (another system that
//! hides the cursor) would make it write every frame, and on the web every write re-requests pointer
//! lock. Add any new cursor behaviour here.

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use std::sync::atomic::{AtomicU8, Ordering};

use crate::capabilities::camera::{ActiveCameraMode, FirstPersonCameraMode};
use crate::runtime::{GameEvent, SimClock};

/// What the runner last reported: `0` = the runner does not report (native), `1` = it reports and
/// the pointer is not locked, `2` = it reports and the pointer is locked.
///
/// A process-wide atomic because the web runner's DOM event listeners run outside the Bevy
/// `World` and have no handle to it; on wasm32 everything runs on one thread.
static RUNNER_LOCK_REPORT: AtomicU8 = AtomicU8::new(0);

/// Called once by a runner that can observe the real pointer-lock state (the web runner), before
/// the app starts. From then on [`CursorLockState::locked`] follows [`report_pointer_lock`] and
/// the engine no longer assumes its own grab request succeeded.
pub fn runner_reports_pointer_lock() {
    // `1`, not `0`: "reports, not locked". Never downgrade a report that already says locked.
    let _ = RUNNER_LOCK_REPORT.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
}

/// Called by the runner from its `pointerlockchange` / `pointerlockerror` listeners with the real
/// state (`document.pointerLockElement` is the canvas).
pub fn report_pointer_lock(locked: bool) {
    RUNNER_LOCK_REPORT.store(if locked { 2 } else { 1 }, Ordering::SeqCst);
}

/// Test hook: forget any runner report so a test starts from the native case.
#[doc(hidden)]
pub fn reset_pointer_lock_report() {
    RUNNER_LOCK_REPORT.store(0, Ordering::SeqCst);
}

/// Whether the cursor is really captured right now. Read by mouse-look gating (and, later, the
/// mouse-button action-bar slots). Written only by [`cursor_capture_system`].
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CursorLockState {
    pub locked: bool,
}

/// Per-run bookkeeping for [`cursor_capture_system`].
#[derive(Default)]
pub struct CaptureProgress {
    /// The engine asked for the lock and has not released it since.
    requested: bool,
    /// `CursorLockState::locked` at the end of the previous run, to spot a lock that vanished.
    was_locked: bool,
}

/// See the module docs.
pub fn cursor_capture_system(
    sim: Res<SimClock>,
    mouse: Res<ButtonInput<MouseButton>>,
    cameras: Query<&ActiveCameraMode, With<FirstPersonCameraMode>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut lock: ResMut<CursorLockState>,
    mut progress: Local<CaptureProgress>,
    mut events: MessageWriter<GameEvent>,
    #[cfg(feature = "inspector")]
    inspector_enabled: Option<Res<crate::inspector::InspectorEnabled>>,
) {
    let Ok(mut cursor) = windows.single_mut() else { return };

    let runner = RUNNER_LOCK_REPORT.load(Ordering::SeqCst);
    let runner_reports = runner != 0;
    if runner_reports {
        let reported = runner == 2;
        if lock.locked != reported {
            lock.locked = reported;
        }
    }

    // A lock the engine asked for, had, and no longer has, with no release from us: the browser
    // took it (Escape). Tell the project once; it decides what to do (the starter pauses).
    if progress.requested && progress.was_locked && !lock.locked && runner_reports {
        events.write(GameEvent::Trigger("cursor.lock_lost".to_string()));
        progress.requested = false;
        // Release fully: the browser took the lock, so the cursor must be visible again even in a
        // project that does not pause on `cursor.lock_lost` (nothing else would show it).
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    progress.was_locked = lock.locked;

    #[allow(unused_mut)]
    let mut wanted = cameras
        .iter()
        .any(|mode| matches!(mode, ActiveCameraMode::FirstPerson(fp) if fp.capture_cursor));
    #[cfg(feature = "inspector")]
    if inspector_enabled.is_some_and(|e| e.0) {
        wanted = false; // the inspector needs a free cursor
    }
    let paused = !sim.is_running();

    if wanted && !paused {
        // The click is the user gesture browsers require. Re-request on a later click when a
        // request was never confirmed (Chrome refuses a re-lock for about a second after Escape).
        let needs_request = !progress.requested || (runner_reports && !lock.locked);
        if needs_request && mouse.just_pressed(MouseButton::Left) {
            // Assigning through `DerefMut` marks `CursorOptions` changed even when the value is
            // identical, which is what makes `bevy_winit` re-apply the grab.
            cursor.grab_mode = CursorGrabMode::Locked;
            // With a reporting runner the cursor is hidden only once the lock is real (below): a
            // refused request must not leave the player with an invisible cursor.
            cursor.visible = runner_reports;
            progress.requested = true;
            if !runner_reports && !lock.locked {
                lock.locked = true;
            }
        }
        if runner_reports && progress.requested {
            let hidden = lock.locked;
            if cursor.visible == hidden {
                cursor.visible = !hidden;
            }
        }
    } else if progress.requested || cursor.grab_mode != CursorGrabMode::None || !cursor.visible {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        progress.requested = false;
        if !runner_reports && lock.locked {
            lock.locked = false;
        }
    }
}
