//! Fixed-tick plan, Phase 1 (`planning/features/gameplay_fixed_tick_pipeline.md`): the delayed events, the stat pipeline and the
//! cooldowns count `Time<Fixed>` ticks, so they expire on the same `SimTick` whatever the frame rate, and a held clock (zero ticks)
//! freezes them. Each test steps the whole `GamePlugin` app with a pinned `TimeUpdateStrategy`, so a 144 Hz run has frames with no
//! tick and a 30 Hz run has frames with two or more.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use ironhold_core::capabilities::action_bar::CooldownMap;
use ironhold_core::runtime::scene_manager::DelayedEventQueue;
use ironhold_core::runtime::sim_tick::SimTick;
use ironhold_core::runtime::{GameEvent, LoadedStateMachine, LogicState, PauseHold, SimClock};
use ironhold_core::schema::{
    Action, ActiveModifier, FsmEventBinding, LiveStat, LoadedStats, StateMachineAsset, StatDef,
};

mod support;
use support::setup_test_app;

/// Ticks a 0.5 s timer needs: `0.5 / (1/64)`. Exact in f32, so a timer of 0.5 s expires on the 32nd tick after it was armed.
/// This is a *duration* in ticks, counted from the tick after the arming. A timer armed by the executor (in `Update`) starts counting at
/// the next frame's first tick, so the absolute `SimTick` of its expiry still depends on the frame cadence until the executor moves
/// into the tick (plan Phase 4); the tests arm between updates, where that offset is the same at every cadence.
const HALF_SECOND_TICKS: u64 = 32;

#[derive(Resource, Default)]
struct Observed {
    delayed: Option<u64>,
    modifier_expired: Option<u64>,
    cooldown_gone: Option<u64>,
    watch_cooldown: bool,
}

/// Runs in `FixedLast`, i.e. after every fixed-tick system of the same tick, and stamps what it sees with the tick's `SimTick`.
fn observe(mut events: MessageReader<GameEvent>, tick: Res<SimTick>, cooldowns: Res<CooldownMap>, mut seen: ResMut<Observed>) {
    for event in events.read() {
        if let GameEvent::Trigger(name) = event {
            if name == "test.delayed" {
                seen.delayed.get_or_insert(tick.0);
            }
            if name == "stat.modifier.expired:buff" {
                seen.modifier_expired.get_or_insert(tick.0);
            }
        }
    }
    if seen.watch_cooldown && seen.cooldown_gone.is_none() && cooldowns.0.is_empty() {
        seen.cooldown_gone = Some(tick.0);
    }
}

fn app_at(hz: f64) -> App {
    let mut app = setup_test_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / hz)));
    app.init_resource::<Observed>().add_systems(FixedLast, observe);
    // The first `update()` runs no tick (Bevy's first real-time delta is zero); warm up past it.
    for _ in 0..3 {
        app.update();
    }
    app
}

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn stat_def(regen_rate: f32) -> StatDef {
    StatDef { base: 0.0, min: 0.0, max: 100.0, soft_max: None, regen_rate, regen_delay: 0.0, thresholds: vec![] }
}

/// Arms one delayed event, one cooldown and one timed modifier, all 0.5 s long, at the current tick.
fn arm_half_second_timers(app: &mut App) {
    app.world_mut().resource_mut::<DelayedEventQueue>().0.push((0.5, "test.delayed".to_string()));
    app.world_mut().resource_mut::<CooldownMap>().0.insert("1".to_string(), (0.5, 0.5));
    let mut hp = LiveStat::new(stat_def(0.0));
    hp.active_modifiers.push(ActiveModifier { key: "buff".to_string(), remaining_secs: Some(0.5) });
    let mut stats = LoadedStats::default();
    stats.0.insert("hp".to_string(), hp);
    app.world_mut().insert_resource(stats);
    app.world_mut().resource_mut::<Observed>().watch_cooldown = true;
}

fn all_observed(app: &App) -> bool {
    let o = app.world().resource::<Observed>();
    o.delayed.is_some() && o.modifier_expired.is_some() && o.cooldown_gone.is_some()
}

#[test]
fn delayed_events_cooldowns_and_modifiers_expire_on_the_same_sim_tick_at_every_frame_rate() {
    for hz in [30.0, 60.0, 64.0, 144.0] {
        let mut app = app_at(hz);
        let start = tick(&app);
        arm_half_second_timers(&mut app);
        for _ in 0..600 {
            app.update();
            if all_observed(&app) {
                break;
            }
        }
        let o = app.world().resource::<Observed>();
        assert_eq!(o.delayed, Some(start + HALF_SECOND_TICKS), "delayed event at {hz} Hz");
        assert_eq!(o.modifier_expired, Some(start + HALF_SECOND_TICKS), "modifier expiry at {hz} Hz");
        assert_eq!(o.cooldown_gone, Some(start + HALF_SECOND_TICKS), "cooldown expiry at {hz} Hz");
    }
}

#[test]
fn regen_adds_the_same_amount_per_tick_at_every_frame_rate() {
    // 8 per second = exactly 0.125 per tick (a power of two), so the sum is exact and any frame-rate dependence shows.
    for hz in [30.0, 64.0, 144.0] {
        let mut app = app_at(hz);
        let start = tick(&app);
        let mut stats = LoadedStats::default();
        stats.0.insert("mana".to_string(), LiveStat::new(stat_def(8.0)));
        app.world_mut().insert_resource(stats);
        for frame in 0..120 {
            app.update();
            let ticks = (tick(&app) - start) as f32;
            let current = app.world().resource::<LoadedStats>().0["mana"].current;
            assert_eq!(current, ticks * 0.125, "regen after {ticks} ticks (frame {frame}) at {hz} Hz");
        }
    }
}

#[test]
fn sim_tick_counts_exactly_the_ticks_that_ran() {
    // `Time<Fixed>::elapsed` advances one period per tick, so `elapsed * 64` is the number of ticks that ran.
    for hz in [30.0, 64.0, 144.0] {
        let mut app = app_at(hz);
        let mut frames_without_tick = 0;
        let mut frames_with_two_or_more = 0;
        for _ in 0..240 {
            let before = tick(&app);
            app.update();
            let ran = tick(&app) - before;
            frames_without_tick += (ran == 0) as u32;
            frames_with_two_or_more += (ran >= 2) as u32;
            let elapsed = app.world().resource::<Time<Fixed>>().elapsed().as_secs_f64();
            assert_eq!((elapsed * 64.0).round() as u64, tick(&app), "SimTick must equal the ticks that ran ({hz} Hz)");
        }
        if hz > 64.0 {
            assert!(frames_without_tick > 0, "a {hz} Hz run must have frames with no tick");
        }
        if hz < 64.0 {
            assert!(frames_with_two_or_more > 0, "a {hz} Hz run must have frames with two or more ticks");
        }
    }
}

#[test]
fn a_held_clock_freezes_the_timers_and_the_tick() {
    let mut app = app_at(64.0);
    arm_half_second_timers(&mut app);
    app.world_mut().resource_mut::<SimClock>().hold(PauseHold::Menu);
    // The frame in which the hold is taken may still run its tick; the clock pauses at the end of that frame.
    app.update();
    app.update();
    let frozen_tick = tick(&app);
    let delayed_left = app.world().resource::<DelayedEventQueue>().0[0].0;
    let cooldown_left = app.world().resource::<CooldownMap>().0["1"].0;
    for _ in 0..40 {
        app.update();
    }
    assert_eq!(tick(&app), frozen_tick, "a held clock runs no tick");
    assert_eq!(app.world().resource::<DelayedEventQueue>().0[0].0, delayed_left, "the delayed event is frozen");
    assert_eq!(app.world().resource::<CooldownMap>().0["1"].0, cooldown_left, "the cooldown is frozen");
    assert!(app.world().resource::<Observed>().delayed.is_none(), "nothing fires while held");

    app.world_mut().resource_mut::<SimClock>().release(PauseHold::Menu);
    for _ in 0..200 {
        app.update();
        if all_observed(&app) {
            break;
        }
    }
    assert!(all_observed(&app), "the timers run on again after the hold is released");
}

#[test]
fn a_delayed_event_still_reaches_the_interpreter_in_the_frame_it_fires() {
    // The timers now run in `FixedUpdate`, which runs before `Update` in a frame, so an event written by a tick is read by the
    // interpreters of the same `app.update()` (the latency did not get worse).
    let mut app = app_at(64.0);
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![FsmEventBinding {
            event: "test.delayed".to_string(),
            do_actions: vec![Action::Log("delayed_seen".to_string())],
        }],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));
    app.world_mut().resource_mut::<DelayedEventQueue>().0.push((0.001, "test.delayed".to_string()));
    app.update();
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.last_action, "Log(\"delayed_seen\")", "the event fired by the tick must be interpreted in the same update");
}

#[test]
fn the_debug_state_exposes_the_sim_tick() {
    // `update_debug_state` runs in `PostUpdate`, after the frame's ticks, so at the end of a frame `DebugState.sim_tick` equals `SimTick`.
    let mut app = app_at(64.0);
    app.update();
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(debug.sim_tick, tick(&app));
    let json = ironhold_core::debug_state_json(debug);
    assert!(json.contains(&format!("\"sim_tick\":{}", debug.sim_tick)), "the JSON for the browser harness carries sim_tick: {json}");
}

#[derive(Resource, Default)]
struct EmitBar(bool);

/// Stands in for an action-bar event: a `GameEvent` written by the `Update` game stream, in the set the action bar sits in.
fn emit_bar(mut armed: ResMut<EmitBar>, mut writer: MessageWriter<GameEvent>) {
    if armed.0 {
        armed.0 = false;
        writer.write(GameEvent::Trigger("test.bar".to_string()));
    }
}

#[test]
fn a_delayed_event_is_now_read_before_a_same_frame_action_bar_event() {
    // The interim order change of fixed-tick Phase 1 (docs/30 "What the rules read in a frame"): a delayed event fires in the tick,
    // which runs before `Update`, so the interpreters read it before the `Update` game stream's action-bar event. Before the change
    // `DelayedEvents` followed `ActionBar` in `Update`, so the bar event came first. The FSM runs both rules, so the *last* executed
    // action tells which event was read last.
    let mut app = app_at(64.0);
    app.init_resource::<EmitBar>()
        .add_systems(Update, emit_bar.in_set(ironhold_core::runtime::schedule::GameStreamSet::ActionBar));
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "a".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding { event: "test.delayed".to_string(), do_actions: vec![Action::Log("delayed".to_string())] },
            FsmEventBinding { event: "test.bar".to_string(), do_actions: vec![Action::Log("bar".to_string())] },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));
    app.world_mut().insert_resource(LogicState("a".to_string()));
    app.world_mut().resource_mut::<DelayedEventQueue>().0.push((0.001, "test.delayed".to_string()));
    app.world_mut().resource_mut::<EmitBar>().0 = true;
    app.update();
    let debug = app.world().resource::<ironhold_core::DebugState>();
    assert_eq!(
        debug.last_action, "Log(\"bar\")",
        "the delayed event (written by the tick) must be read before the action-bar event (written in Update), so \"bar\" runs last"
    );
}
