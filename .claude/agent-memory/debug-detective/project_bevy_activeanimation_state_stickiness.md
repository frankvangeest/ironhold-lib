---
name: bevy-activeanimation-state-stickiness
description: Bevy 0.18 ActiveAnimation keeps paused/repeat across replay() and start(), and paused clips are still fully sampled at weight 1.0 — three facts that break naive animation-state reasoning
metadata:
  type: project
---

Verified against `bevy_animation-0.18.0/src/lib.rs` + `transition.rs` (vendored in the cargo
registry, read directly — not from docs):

1. **`ActiveAnimation::replay()` (lib.rs:552) resets only `just_completed`/`completions`/
   `elapsed`/`last_seek_time`/`seek_time`. It does NOT reset `paused` and does NOT reset
   `repeat`.** And `AnimationPlayer::start()` (lib.rs:806) is
   `active_animations.entry(idx).or_default()` — it *reuses* the existing `ActiveAnimation` for a
   node that was played before. So replaying the same node index inherits both the old pause
   state and the old `RepeatAnimation`.
   Consequence for `animation_playback_system`: `if controller.should_loop { active_anim.repeat(); }`
   with no `else { set_repeat(Never) }` means a node that was once played looping stays
   `RepeatAnimation::Forever` on every later same-node replay. Only bites when one clip name is
   shared between a looping context (`base.idle`/a `clips:` alias) and a non-looping override —
   which is exactly the shape `prefabs/animation/corpse_policy_*.ron` deliberately creates.

2. **A paused animation is still sampled and still drives bones.** `advance_animations`
   (lib.rs:1004) and the event triggers (lib.rs:949, :1151) skip `paused`, but `animate_targets`
   only skips `active_animation.weight == 0.0` (lib.rs:1138). `advance_transitions` keeps setting
   the main animation's weight to `remaining_weight` regardless of pause. So a frozen pose costs
   full per-frame animation sampling + skinning for its whole lifetime — freezing is not a
   "bake and stop".

3. **`AnimationTransitions::play` (transition.rs) skips the fade-out transition for an outgoing
   clip that `is_paused()`**, so a paused clip never reaches
   `expire_completed_transitions`' `player.stop()` and stays blended at full weight. Also:
   `Duration::ZERO` makes `weight_decline_per_sec` `INFINITY` (works — weight hits 0 next frame),
   and replaying the *same* node pushes a transition for it then immediately drops it via the
   `retain(|t| t.animation != new_animation)` line, so same-node replay is self-correcting.

**How to apply:** when reasoning about `capabilities/animation.rs`, never assume
`transitions.play()` gives you a clean `ActiveAnimation`. Anything you set once (pause, repeat,
speed, weight) persists on that node until the node is `stop()`ed — which only happens when a
fade-out completes. See [[project_test_harness_no_animation_plugin]] for why no test can catch
any of this.
