---
name: project-runif-skip-stale-change-baseline
description: A run_if-skipped system does not advance its change-detection last_run, so Ref/Changed/Added sees every change made during the skip on its first run; gate in-body instead. Real pause (SimClock) relies on this.
metadata:
  type: project
---

A Bevy system skipped by `run_if` does not run, so its `last_run` tick stays put. On the first run after the skip,
`Ref<T>::is_changed()` / `Changed<T>` / `Added<T>` report every change made while it was skipped (for example a UI
`Interaction` that went to `Pressed` during a pause fires as a fresh click on resume). An early `return` inside the
body DOES advance the baseline. Message readers are similar: a skipped `MessageReader` keeps its cursor, so drain it
before returning or the resume frame consumes stale input (up to 2 frames of buffer).

**Why:** real_pause (2026-10-14) gates `action_bar_input_system` in-body for this reason, and `first_person_camera_system`/`fly_camera_system` drain `MouseMotion` before returning. Reviewed, not test-pinned (the planned "slot still Pressed after resume" test was missing at review time).

**How to apply:** when a review finds a new `.run_if(...)` on a system that reads `Ref`/`Changed`/`Added` or a `MessageReader`, check what happens on the first frame after the condition turns true again. Related: [[project-test-harness-just-pressed-latch]] (keyboard `just_pressed` latches in tests, so "fires again after resume" assertions pass whether the gate works or not).
