---
name: fixed-tick-timers-phase1
description: Reviewing gameplay_fixed_tick_pipeline phases (timers/SimTick onto FixedUpdate) - what is designer-visible, where the stale "each frame" doc sites hide, and the plan items that tend to be left out
metadata:
  type: project
---

Phase 1 (`feature/fixed_tick_timers`, reviewed 2026-11-19) moved `tick_delayed_events`, `stat_modifier -> regen -> effective`
and `cooldown_tick` into the FixedUpdate chain (after `npc_behavior`, before `motion`) and added `SimTick` (FixedFirst) +
`DebugState.sim_tick`. Code is RON-neutral: seconds stay seconds, no new schema, no new GameVariable, `FIXED_DT` derives
from `FIXED_TICK_RATE` (engine constant, no RON knob is correct, see [[fixed-timestep-det-math]]).

**Designer-visible effects (must be in docs, the code alone does not count):**
- Same-frame order inversion: delayed and `stat.modifier.expired:*` events now reach interpreters BEFORE that frame's
  targeting/action-bar/interact `GameEvent`s (until Phase 4). Plan requires docs/30 wording + one test.
- Timers round up to 1/64 s; repeating timers re-arm a step later; cooldown sweep/regen visibly step at 64 Hz on 144 Hz.
- Action-bar cost gate now sees THIS frame's regen (was previous frame) - schedule.rs "Accepted consequences" goes stale.
- `despawn_timer` and dialogue stay frame-based until Phase 4 - the docs/20 "Timing resolution" note must name only the
  Phase 1 timers.
- Pause is unchanged: held clock = zero ticks freezes them exactly as `Res<Time<Virtual>>` did; nothing was gated before.

**Stale-comment sites to grep each phase:** `src/CLAUDE.md` (`tick_delayed_events_system` "reads the virtual Res<Time>"),
`runtime/schedule.rs` rules (stat_effective "once in Stats") and "Accepted consequences", `capabilities/stats.rs` doc
comments ("this frame", "runs before the interpreter chain"), `scene_manager/mod.rs` DelayedEventQueue "each frame",
`schema/stats.rs` "Computed each frame".

**Plan items left out at first review:** all docs (docs/20, docs/30, STATUS.md JSON example), the interim-inversion
test, the `ironhold validate` info note for `delay_secs`/`cooldown_secs` in (0, 1/64). Check these at every later phase:
the plan lists exact doc line numbers per phase.
