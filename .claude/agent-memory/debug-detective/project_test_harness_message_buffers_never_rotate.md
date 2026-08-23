---
name: test-harness-message-buffers-never-rotate
description: setup_test_app's init_resource::<Messages<T>>() before add_plugins(GamePlugin) makes add_message a no-op, so UiEvent/GameEvent/SceneEvent/InputActionMessage/AppExit/MouseMotion/MouseWheel buffers never rotate — iter_current_update_messages() is cumulative-forever in every integration test
metadata:
  type: project
---

`crates/ironhold_core/tests/support/mod.rs`'s `setup_test_app()` calls
`.init_resource::<Messages<UiEvent>>()` / `GameEvent` / `SceneEvent` / `InputActionMessage` /
`AppExit` / `MouseMotion` / `MouseWheel` **before** `.add_plugins(GamePlugin)`. `SubApp::add_message::<T>()`
(bevy_app-0.18.0 `src/sub_app.rs:358`) is `if !self.world.contains_resource::<Messages<T>>() { register }`
— a **no-op when the resource already exists**. So `GamePlugin`'s `add_message::<GameEvent>()` etc. never
enter those types in `MessageRegistry`, and `message_update_system` never rotates their double buffers
for the entire life of the test app.

Two consequences that repeatedly look like something else:
- `Messages::<T>::iter_current_update_messages()` — documented as "messages since the last `update()`" —
  actually returns **every message ever written** in these tests. Code that reads it as a per-tick count
  is silently a cumulative count, and vice versa. A test can therefore pass for entirely the wrong reason.
- `Messages<T>::len()` grows unboundedly across a long test loop (memory only, harmless).

**Why:** verified 2026-08-20 with three probe apps: `setup_test_app` never swapped over 20 updates
(~25 real `FixedUpdate` ticks); `App + MinimalPlugins + add_message::<GameEvent>()` swapped after 1 update;
`init_resource THEN add_message` never swapped. So the cause is the ordering, not timing.

**How to apply:** when a test's assertion depends on message counts or "current frame" message contents,
do not trust `iter_current_update_messages()` semantics — assert on ECS component state instead, or
accumulate explicitly. If anyone ever "fixes" `setup_test_app` to use `add_message` (or drops the
pre-`init_resource` block), expect a wave of message-count assertions to flip meaning: lower bounds
become false failures, upper bounds become vacuous passes. Note also that buffer rotation is gated on
`FixedUpdate` actually running (bevy_time puts `signal_message_update_system` in `FixedPostUpdate`, and
`Time<Fixed>` is driven by wall clock), so even correctly-registered message types don't rotate in a
tight `app.update()` loop that finishes in under ~16 ms.

Related: [[project_test_harness_just_pressed_latch]], [[project_fixedupdate_vs_rapier_clock]].
