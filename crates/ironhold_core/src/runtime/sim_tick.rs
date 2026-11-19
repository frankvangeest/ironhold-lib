//! `SimTick`: the number of fixed ticks the simulation has run (`planning/features/gameplay_fixed_tick_pipeline.md`, Phase 1).
//!
//! It is a stamp, not a clock. Timers keep counting seconds with the constant tick length
//! ([`crate::capabilities::physics::FIXED_DT`]); `SimTick` is for tagging things with the tick they happened on (latched
//! inputs, logs, the determinism harness's state hashes, replay). Fixed-tick systems read `Res<Time>::delta_secs()` or this
//! counter, **never `Time<Fixed>::elapsed()`** (it diverges from `Time<Virtual>::elapsed()` by the overstep).
//!
//! It advances in `FixedFirst`, the first thing of every tick, so every system of tick `n` sees `SimTick(n)`; the first tick
//! of a run is tick 1. A frame with no tick leaves it unchanged, a frame with two ticks advances it by two, and a held clock
//! (which runs zero ticks) freezes it. The pause control tick of a later phase advances it only when the tick is active.

use bevy::prelude::*;

/// Fixed ticks run so far. `SimTick(0)` before the first tick.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimTick(pub u64);

/// Advances [`SimTick`] by one. Runs first in every fixed tick.
pub fn advance_sim_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

/// Inserts [`SimTick`] and advances it in `FixedFirst`.
pub struct SimTickPlugin;

impl Plugin for SimTickPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimTick>().add_systems(FixedFirst, advance_sim_tick);
    }
}
