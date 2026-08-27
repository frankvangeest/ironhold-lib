---
name: animation-seek-freeze-constraints
description: bevy_animation 0.18 facts that constrain any seek-to-fraction / freeze-at-frame animation feature (paused-clip never faded out, seek only applies on clip change, pose survives stop)
metadata:
  type: project
---

Verified against `bevy_animation-0.18.0/src/lib.rs` + `src/transition.rs` (2026-08-26) while
design-reviewing a "start clip at N% / freeze there" feature. These are library behaviors, not
our code, so they constrain any implementation.

**1. `AnimationTransitions::play` refuses to fade out a PAUSED old animation** (`transition.rs`
~line 84: `&& !old_animation.is_paused()`). No `AnimationTransition` entry is pushed, so
`advance_transitions` never decays its weight and `expire_completed_transitions` never calls
`player.stop()` on it. Net effect: once you `pause()` an `ActiveAnimation`, every *later* clip on
that entity is permanently blended 50/50 with the frozen pose, unrecoverable through our API.
**Why:** any freeze feature must `resume()` (or `player.stop()`) the previously-frozen node
*before* the next `transitions.play(...)`. Harmless for write-once entities (a corpse that never
animates again); fatal for a demo/QA scene that toggles freeze across clips.

**2. A seek can only be applied where `transitions.play()` is called**, and
`animation_playback_system` (`capabilities/animation.rs` ~line 200) only calls it when
`controller.current != controller.last_played`. A second command with the *same* clip and a
different fraction is a silent no-op. Needs an explicit dirty flag on `AnimationController`
(resolver sets, playback consumes) — not just durable state on `ActiveOverride`.

**3. Seek/freeze state must be DURABLE, not one-shot.** `animation.rs`'s GLTF-respawn recovery
(~lines 165–197, common on WASM when textures land after the initial scene spawn) resets
`graph_initialized=false` + `last_played=""`, forcing a full re-play. One-shot seek state would
silently un-freeze and replay from t=0 on the web only. Keep the fraction/frozen flag on
`ActiveOverride` so any replay re-applies it.

**4. `set_seek_time` vs `seek_to`:** `seek_to` sets `last_seek_time` to the *old* time, so every
AnimationEvent between old and new time fires on the next `update()`. `set_seek_time` (line 644)
does not. Prefer `set_seek_time` for a jump-to-fraction.

**5. Freeze holds the pose because `animate_targets` ignores `paused`** — the `!paused` guard
(line ~1151) only gates event triggering. `advance_animations` (line 978) skips paused clips, so
time stops but the pose is still sampled and written every frame. Corollary: a frozen entity still
costs full curve evaluation + skinning per frame — freezing is not a perf optimization. Also,
removing an `ActiveAnimation` entirely leaves the last-written bone transforms in place, so the
pose persists for free — that's the cheap path for a long-lived static pose, and it's also why
bug (1) stays invisible until something else plays.

**6a. `AnimationPlayer::start` → `ActiveAnimation::replay()` (lib.rs:552) resets ONLY
`just_completed`/`completions`/`elapsed`/`last_seek_time`/`seek_time` — NOT `paused`, NOT `repeat`,
NOT `weight`.** So any code path that replays the *same* node index twice inherits the previous
play's repeat mode and paused flag. Verified 2026-08-26. **Why it matters:** our
`animation_playback_system` only ever calls `active_anim.repeat()` when `should_loop` is true and
has no `else set_repeat(Never)`. That was unreachable while the gate was `current != last_played`
(same node twice was impossible); `pending_seek` makes it reachable, so a looping→non-looping
same-clip re-seek keeps `RepeatAnimation::Forever`. Prefer an unconditional
`set_repeat(if should_loop { Forever } else { Never })`, and an explicit `resume()` when
not freezing, over relying on pre-play cleanup alone.

**6b. `AnimationTransitions::play` with `new_animation == old_animation_index`** pushes a fade-out
transition for the old (same) node then immediately `retain`s it away, so a same-node replay is
safe and leaves no lingering transition. (transition.rs:78-100.)

**7. `start_at` of 1.0 only means "final frame" for a non-looping clip.** `update()` does
`seek_time %= clip_duration`, so on a `repeat()`ed clip a 100% seek wraps to 0. The existing
"hold last frame" behavior of `death`/`attack_light` overrides comes from `RepeatAnimation::Never`
+ `is_finished()` short-circuiting `update()` — no new mechanism needed for a natural end-hold.
