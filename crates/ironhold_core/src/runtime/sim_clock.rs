//! The single owner of "is the simulation running?" (`planning/features/real_pause.md`).
//!
//! Gameplay never reads `Time<Virtual>::is_paused()` and nothing else calls `Time<Virtual>::pause()`:
//! everything goes through [`SimClock`]. A hold is an idempotent set member (never a counter: a
//! counter can leak, see `panels_open`), so pausing twice needs one resume, and releasing one reason
//! (a designer's pause menu) cannot undo another (static scene mode, focus loss, a networked session).
//!
//! `Time<Virtual>` is the lever: while it is paused `FixedUpdate` runs zero ticks, so physics, the
//! player, NPCs and motion freeze with no per-system code, and every `Update` system that reads
//! `Res<Time>` freezes too. Edge-triggered *input* systems do not follow the clock and are gated with
//! [`sim_running`] (or an in-body `SimClock::is_running` check).

use std::collections::BTreeSet;

use bevy::ecs::message::{MessageRegistry, ShouldUpdateMessages};
use bevy::prelude::*;

use crate::runtime::messages::GameEvent;

/// Why the simulation is held. `Menu` is the only one a designer can set (`PauseGame`/`ResumeGame`/
/// `TogglePause`, and the Replace-mode auto-resume releases only this one). The others are reserved
/// for `static_scene_mode.md`, the focus-loss item and the network layer (Beta 0.6): a networked run
/// mode turns `Menu` into "open the menu, ask the host" and the host's broadcast is `Session`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PauseHold {
    Menu,
    Static,
    FocusLoss,
    Session,
}

impl PauseHold {
    /// Whether this hold is "the game is paused" as a designer sees it: it is what `game.paused` /
    /// `game.resumed` follow. `Menu` is the designer's own pause. `Static` is the browser-test harness
    /// (`?static=1`) and must never open a project's pause menu, so it stays silent. `FocusLoss` and
    /// `Session` are reserved and choose explicitly when they land.
    pub fn announces(self) -> bool {
        matches!(self, PauseHold::Menu)
    }
}

/// The set of active simulation holds. Empty = running. Ordered (`BTreeSet`) for determinism.
#[derive(Resource, Default, Debug)]
pub struct SimClock {
    holds: BTreeSet<PauseHold>,
}

impl SimClock {
    /// A clock that starts with `hold` already taken (the harness `Static` hold, from the first frame).
    pub fn with_hold(hold: PauseHold) -> Self {
        let mut clock = Self::default();
        clock.hold(hold);
        clock
    }

    /// True while any announcing hold is set (see [`PauseHold::announces`]).
    pub fn announces_pause(&self) -> bool {
        self.holds.iter().any(|h| h.announces())
    }

    pub fn is_running(&self) -> bool {
        self.holds.is_empty()
    }

    pub fn is_held(&self, hold: PauseHold) -> bool {
        self.holds.contains(&hold)
    }

    /// Returns true if the hold was newly added.
    pub fn hold(&mut self, hold: PauseHold) -> bool {
        self.holds.insert(hold)
    }

    /// Returns true if the hold was present and removed.
    pub fn release(&mut self, hold: PauseHold) -> bool {
        self.holds.remove(&hold)
    }

    /// Flips one hold; returns true if it is held afterwards.
    pub fn toggle(&mut self, hold: PauseHold) -> bool {
        if !self.holds.remove(&hold) {
            self.holds.insert(hold);
            true
        } else {
            false
        }
    }
}

/// Run condition for edge-triggered gameplay input systems: false while any hold is set.
pub fn sim_running(clock: Res<SimClock>) -> bool {
    clock.is_running()
}

/// Keeps every `Messages<T>` buffer flowing while the sim is held.
///
/// Bevy only swaps/clears message buffers after a `FixedUpdate` pass (`signal_message_update_system` runs in
/// `FixedPostUpdate`), and a paused `Time<Virtual>` runs zero fixed ticks, so without this every buffer
/// (pointer hits, cursor/mouse motion, asset events, `UiEvent`/`GameEvent`) would grow for the whole pause: several MB
/// per minute on a pause screen that may sit open for ten minutes, and WASM linear memory never shrinks. Re-arming the
/// signal each held frame makes the update system run every frame, exactly as it does when there is no fixed
/// timestep. Messages written in `Update` while held therefore age out normally; the messages `FixedUpdate`
/// consumers read are themselves written in `FixedUpdate`, so none is lost. Any future hold (`Static`, `FocusLoss`)
/// gets this for free because it only looks at `is_running`. A `run_if`-gated `Update` system that owns a
/// `MessageReader` must still drain it before returning, or it replays the backlog on resume.
pub fn keep_messages_flowing_while_held(clock: Res<SimClock>, registry: Option<ResMut<MessageRegistry>>) {
    if clock.is_running() {
        return;
    }
    if let Some(mut registry) = registry {
        if registry.should_update == ShouldUpdateMessages::Waiting {
            registry.should_update = ShouldUpdateMessages::Ready;
        }
    }
}

/// The **only** caller of `Time<Virtual>::pause()`/`unpause()` and the only emitter of `game.paused`
/// / `game.resumed`. It compares the clock to the real `Time<Virtual>` state instead of trusting the
/// change flag (an executor arm taking `ResMut<SimClock>` can mark it changed without changing it),
/// so the clock is paused/unpaused only on an actual running <-> held transition. `game.paused`/`game.resumed` follow
/// the *announcing* holds (`PauseHold::announces`) instead, so the events fire when a designer's pause starts or ends
/// (including the auto-resume on a Replace-mode scene load), never for the harness `Static` hold.
pub fn apply_sim_clock_system(
    clock: Res<SimClock>,
    mut time: ResMut<Time<Virtual>>,
    mut game_events: MessageWriter<GameEvent>,
    mut announced: Local<bool>,
) {
    let want_paused = !clock.is_running();
    if want_paused != time.is_paused() {
        if want_paused {
            time.pause();
            info!("Simulation held ({:?}): Time<Virtual> paused", clock.holds);
        } else {
            time.unpause();
            info!("Simulation resumed: Time<Virtual> unpaused");
        }
    }
    // `game.paused`/`game.resumed` follow the announcing holds only, not the clock: a `Static` hold
    // (harness) never opens a pause menu, and a `PauseGame` while it is held still announces.
    let want_announced = clock.announces_pause();
    if want_announced != *announced {
        *announced = want_announced;
        let event = if want_announced { "game.paused" } else { "game.resumed" };
        game_events.write(GameEvent::Trigger(event.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_are_an_idempotent_set_not_a_counter() {
        let mut c = SimClock::default();
        assert!(c.is_running());
        assert!(c.hold(PauseHold::Menu));
        assert!(!c.hold(PauseHold::Menu), "second hold is a no-op");
        assert!(!c.is_running());
        assert!(c.release(PauseHold::Menu));
        assert!(c.is_running(), "one release undoes any number of holds");
        assert!(!c.release(PauseHold::Menu), "releasing an absent hold is a no-op");
    }

    #[test]
    fn releasing_one_reason_does_not_undo_another() {
        let mut c = SimClock::default();
        c.hold(PauseHold::Static);
        c.hold(PauseHold::Menu);
        c.release(PauseHold::Menu);
        assert!(!c.is_running(), "static mode still holds");
        assert!(c.is_held(PauseHold::Static));
    }

    #[test]
    fn toggle_flips_only_its_own_hold() {
        let mut c = SimClock::default();
        assert!(c.toggle(PauseHold::Menu));
        assert!(c.is_held(PauseHold::Menu));
        assert!(!c.toggle(PauseHold::Menu));
        assert!(c.is_running());
    }
}
