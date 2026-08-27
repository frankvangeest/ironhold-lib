---
name: test-harness-no-animation-plugin
description: setup_test_app() omits bevy AnimationPlugin, so no test can observe Bevy's own animation pipeline — seek/pause/fade-out assertions only echo back what ironhold's own system wrote
metadata:
  type: project
---

`crates/ironhold_core/tests/support/mod.rs`'s `setup_test_app()` adds `MinimalPlugins`,
`StatesPlugin`, `TransformPlugin`, `AssetPlugin`, `ScenePlugin` and `GamePlugin` — but **never
`bevy::animation::AnimationPlugin`**. `Assets<AnimationClip>` is registered by hand
(`.init_asset::<bevy::animation::AnimationClip>()`), which is the correct minimal fix for
`animation_playback_system`'s `Res<Assets<AnimationClip>>` param, but it does not bring in any of
Bevy's animation *systems*.

So in every test: `advance_animations`, `advance_transitions`, `expire_completed_transitions` and
`animate_targets` never run. Practical consequences:

- `seek_time` never advances, so a `freeze: false` seek is indistinguishable from a frozen one.
- An `ActiveAnimation` is never removed from `AnimationPlayer.active_animations`, so an assertion
  like "the old clip is still present, mid fade-out" passes because nothing *could* remove it.
- Weights stay at whatever was set at insert time; no blend/fade behavior is observable.
- Nothing about [[bevy-activeanimation-state-stickiness]] (sticky `repeat`, paused-clip sampling)
  is reachable by a test.

**How to apply:** an animation test that asserts on `ActiveAnimation` fields is only asserting
what ironhold's own system wrote one moment earlier — treat it as a unit test of the writer, not
evidence that playback behaves correctly. To make a fade-out/looping/advance assertion real, add
`bevy::animation::AnimationPlugin` to that specific test file's app (do not add it to
`setup_test_app()` wholesale without checking the ~26 other suites).
