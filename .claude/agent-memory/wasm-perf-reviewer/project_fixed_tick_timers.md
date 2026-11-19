---
name: project-fixed-tick-timers
description: Fixed-tick plan Phase 1 - delayed events, cooldowns and stat chain moved Update->FixedUpdate; per-tick cost model and the keys-clone hot spot
metadata:
  type: project
---

Phase 1 of `gameplay_fixed_tick_pipeline.md` (reviewed 2026-11-19, branch feature/fixed_tick_timers) moved
`tick_delayed_events_system`, `stat_modifier/regen/effective_value_system` and `cooldown_tick_system` into the
FixedUpdate `.chain()` (between npc_behavior and motion) and added `SimTick` (one u64 += 1 in FixedFirst).

- Cost scales with ticks not frames: 144 Hz = ~0.44x runs (win), 60 Hz = 1.07x, 30 fps = ~2.1x per frame, stall = max 16
  ticks (250 ms max_delta / 1/64). Rapier's 16 steps dominate any burst; the stat chain is microseconds.
- None of the moved systems has Commands -> no new auto ApplyDeferred nodes in the fixed chain.
- No `Changed<StatMap>` / `LoadedStats.is_changed()` readers exist, so the unconditional DerefMut marking is harmless.
- Hot spot (pre-existing, now per tick): `stat_modifier_system` (and `stat_threshold_system` in Update) clone every key
  into a `Vec<String>` per StatMap entity; `IndexMap::iter_mut()` gives `(&String, &mut LiveStat)` so it is avoidable.
- Content scale 2026-11: global stats 3-9 per project; StatMap entities 0-1 per shipped scene (≤4 in co-op).

**Why:** later phases (interpreters, despawn timers, dialogue into the tick) multiply these per-tick costs.
**How to apply:** when Phase 2+ moves more systems into FixedUpdate, re-check per-tick allocations and Commands (sync points);
ask whether the keys-clone fix landed. Related: [[project-deterministic-fixed-timestep]], [[project-real-pause]].
