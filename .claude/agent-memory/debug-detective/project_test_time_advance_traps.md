---
name: test-time-advance-traps
description: In integration tests Time::advance_by is silently undone by TimePlugin, and TimeUpdateStrategy::ManualDuration is clamped to Time<Virtual>'s 250ms max_delta — both make EmitEventAfterDelay tests pass vacuously
metadata:
  type: project
---

Two independent traps when a test needs seconds of simulated time (e.g. `EmitEventAfterDelay` /
`DelayedEventQueue`, which `tick_delayed_events_system` drains using `Res<Time>`):

1. **`world.resource_mut::<Time>().advance_by(d)` does nothing useful.** `TimePlugin`'s
   `time_system` runs in `First` and overwrites `Time` from the real clock on the next
   `app.update()`, so the injected delta is discarded. `action_tests.rs`'s delayed-event tests use
   this pattern and pass only because their thresholds (0.001 s fire / 15 s no-fire) happen to sit
   on either side of a real frame delta — they are not actually advancing time.
2. **`TimeUpdateStrategy::ManualDuration(d)` is clamped to 250 ms per update** by
   `Time<Virtual>`'s default `max_delta`. So `ManualDuration(1s)` advances 0.25 s, and a naive
   `for _ in 0..25 { update() }` loop reaches 6.25 s, not 25 s.

**Why:** both were hit while reproducing the corpse-loot bug (2026-08-24). Under trap 1 the
zombie's 10 s hide and 20 s respawn timers never fired, and under trap 2 they still didn't —
which made the test *look* like it proved "corpse stays in `dead_full` forever". A test written
this way silently asserts nothing about any timer-driven state change.

**How to apply:** to advance N seconds, set
`TimeUpdateStrategy::ManualDuration(Duration::from_millis(250))` once and run `ceil(N / 0.25)`
updates. Always print `DelayedEventQueue.0` after the loop — if the remaining values didn't move
as expected, time isn't advancing and any conclusion drawn from that state is void. Related:
[[project_test_harness_message_buffers_never_rotate]],
[[project_test_harness_just_pressed_latch]], [[project_fixedupdate_vs_rapier_clock]].
