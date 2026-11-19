---
name: project-fixed-tick-timer-semantics
description: Timers moved Update->FixedUpdate (fixed-tick Phase 1) change edge behaviour - zero-length timers no longer fire while paused, fire up to 1 tick early vs Update timers, expiry dedupe is per tick
metadata:
  type: project
---

Reviewed 2026-11-19 on `feature/fixed_tick_timers` (delayed events, cooldowns, stat modifier/regen moved into the FixedUpdate chain).

- A timer armed in `Update` (executor/flush) is counted from the last tick *before* arming, so it fires up to one tick (15.6 ms) EARLIER than an equal-length timer still on `Time<Virtual>` in Update (`SetDespawnTimer`, dialogue advance) armed in the same frame. Never later. Loops re-armed every fire no longer drift (tick-quantised).
- Old Update timers ran with `dt = 0` while paused, so an entry already `<= 0` (delay_secs 0, cooldown 0, modifier 0 s) still fired/expired during a pause. In FixedUpdate a held clock runs zero ticks, so they wait for resume. No shipped RON uses a 0 delay (checked).
- `tick_modifiers` dedupes expiry "per key per call": per frame before, per tick now, so a 2+-tick frame can emit `stat.modifier.expired:k` twice in one interpreter pass.
- Test harness: first `app.update()` runs zero ticks (same as old zero virtual delta), then one tick per update at the pinned 1/64 s, so step-count tests are unaffected; 250 ms steps = exactly 16 ticks.

**Why:** these are the non-obvious seams between the two clocks until Phase 4 moves the rest.
**How to apply:** when reviewing later fixed-tick phases or a "timer fires a frame early/never during pause" report, start here. Related: [[project-fixedupdate-vs-rapier-clock]], [[project-delayed-events-uncancellable]].
