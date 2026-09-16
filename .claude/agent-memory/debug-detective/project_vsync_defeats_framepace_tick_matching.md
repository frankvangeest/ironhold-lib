---
name: vsync-defeats-framepace-tick-matching
description: Bevy's default PresentMode is Fifo (hard vsync) and ironhold never overrides it, so a bevy_framepace cap of 64fps never engages on a 60Hz display — matching framepace to FIXED_TICK_RATE does NOT remove the 60/64 double-tick beat
metadata:
  type: project
---

`Window::default()`'s `present_mode` is `PresentMode::Fifo` (`bevy_window-0.18.0/src/window.rs:1233`,
the `#[default]`), and no ironhold code sets `present_mode` anywhere. `bevy_framepace` never touches
it either. So the **present rate is the display's refresh rate**, and a `Limiter::from_framerate(64.0)`
cap sitting *above* a 60Hz vsync simply never fires.

Consequence: `deterministic_fixed_timestep` v1 changed the native framepace from `60.0` to
`FIXED_TICK_RATE` (64.0) on the stated reasoning that "every frame then advances physics by exactly
one tick". That is false on every ordinary display. At 60 fps vsync with a 64 Hz `Time<Fixed>`, one
second is 60 frames and 64 ticks → **56 one-tick frames + exactly 4 two-tick frames, every second**.
144Hz is worse: the 15.625 ms framepace target lands between vsync intervals, so you present at 48 fps
and get ~16 two-tick frames/second.

There is no framepace/tick-rate pairing that removes the beat, because 64 Hz displays do not exist —
anything that depends on "one tick per rendered frame" is depending on something unachievable. Fix the
consumer, not the cadence.

**How to apply:** before accepting any claim that a frame-rate cap has synchronised something to
`FIXED_TICK_RATE`, check the present mode. And treat "N double-tick frames per second" as
`|refresh_rate − 64|`, which is a directly falsifiable prediction to hand a playtester.
Related: [[update-globaltransform-one-tick-stale]], [[fixedupdate-vs-rapier-clock]].
