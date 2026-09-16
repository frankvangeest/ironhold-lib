use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

/// `FixedUpdate`'s tick rate — the single source of truth for this plugin's
/// `TimestepMode::Fixed::dt` below, the schedule's own period (`Time<Fixed>`, also set by this
/// plugin), and `capabilities::player`'s jump-timing math (which imports this constant directly
/// — see `player.rs`), so none of the three can independently drift apart. Before this constant
/// existed, `Time<Fixed>`'s period was just Bevy's unmodified 64Hz engine default and the other
/// two were separate `64.0` literals — pulling it out doesn't change behavior on its own, it's
/// `PhysicsPlugin::build`'s explicit `Time<Fixed>` insert below that makes the schedule's rate
/// actually derive from this constant rather than merely coincide with it.
pub const FIXED_TICK_RATE: f32 = 64.0;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Always register CollisionEvent so MessageReader<CollisionEvent> is safe in all
        // contexts, including headless tests where RapierPhysicsPlugin is skipped.
        app.add_message::<CollisionEvent>();
        // Makes `FIXED_TICK_RATE` authoritative over the schedule's own period, not just over
        // `TimestepMode::Fixed::dt` below — without this, `FixedUpdate` runs at Bevy's *default*
        // 64Hz (`bevy_time`'s `DEFAULT_TIMESTEP`), which only coincides with `FIXED_TICK_RATE`
        // today; a future Bevy upgrade changing that default, or a later slow-motion/pause
        // feature calling `Time::<Fixed>::from_hz` elsewhere, would silently desync the solver's
        // `dt` from the schedule's actual tick rate. Found during v1 review (see
        // `planning/features/deterministic_fixed_timestep.md`).
        #[cfg(not(test))]
        app.insert_resource(Time::<Fixed>::from_hz(FIXED_TICK_RATE as f64));
        // Must be inserted before `add_plugins` below: `RapierPhysicsPlugin::build()` only
        // `init_resource`s `TimestepMode` (a no-op if the resource already exists), so
        // pre-inserting `Fixed` here is what prevents both the plugin's own default
        // `TimestepMode::Variable` and a spurious startup `warn!` ("TimestepMode is set to
        // Variable, it is recommended to use Fixed if physics is in FixedUpdate") — see
        // `bevy_rapier3d-0.33.0/src/plugin/plugin.rs:328-337`. `dt` matches `FIXED_TICK_RATE`
        // above, not a separate 60Hz/`max_dt`-derived value — see
        // `planning/features/deterministic_fixed_timestep.md`'s Approach section for why reusing
        // the old `TimestepMode::Variable::max_dt` clamp value here would be wrong (it clamped a
        // wall-clock step, it was never the schedule's own tick rate).
        #[cfg(not(test))]
        app.insert_resource(TimestepMode::Fixed {
            dt: 1.0 / FIXED_TICK_RATE,
            substeps: 1,
        });
        // `in_fixed_schedule()`: runs Rapier's PhysicsSet::{SyncBackend, StepSimulation,
        // Writeback} in `FixedUpdate` instead of the default `PostUpdate`, so every device steps
        // the solver with the same `dt` regardless of frame rate — see the feature plan's "Why".
        // The existing `FixedUpdate` gameplay chain in `lib.rs` is ordered `.before(PhysicsSet::
        // SyncBackend)` to preserve the same read-before-step relationship it already had when
        // physics lived in a separate, later-running schedule.
        #[cfg(not(test))]
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule());

        // Physics collider overlay — starts disabled; toggle with F9 when the
        // inspector feature is enabled. Gated by not(test) for the same reason
        // as RapierPhysicsPlugin: the resource won't exist in headless tests.
        #[cfg(all(not(test), feature = "inspector"))]
        app.add_plugins(bevy_rapier3d::render::RapierDebugRenderPlugin {
            enabled: false,
            ..default()
        });
    }
}
