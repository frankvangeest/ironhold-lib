---
name: animation-seek-freeze-pattern
description: Pre-implementation review findings for Action::PlayAnimationOn start_at/freeze (clip seeking + pose freezing) — resolver branch-merge trap, executor-vs-drain_spawn_queue ordering trap, WASM re-play trap
metadata:
  type: project
---

Reviewed at plan stage (2026-08-26, no code yet) for the `monster_corpse_loot` follow-up: give
`*_corpse` prefabs a frozen last-frame death pose via new `Action::PlayAnimationOn.start_at:
Option<f32>` (0–1 fraction) + `freeze: bool`.

**Why:** the three traps below are all invisible in a diff and each individually silently breaks the
primary use case. They come from reading `animation_resolver.rs` + `animation.rs` + `lib.rs`'s
system chain together, which a per-file review won't do.

**How to apply:** when reviewing any change that (a) adds fields to an action consumed via
`AnimationRequests`, or (b) fires an action at an entity spawned by `Action::Spawn` in the same
`do_actions` list.

1. **`animation_resolver_system` has three branches** for a queued command — override `id`,
   semantic `clips` alias, raw clip name — checked in that order. Every shipped monster policy
   (`player_policy_zombie.ron`, `snake_policy.ron`, `spider_policy.ron`, `player_policy_human.ron`)
   declares an *override* with `id: "death"`, so `clip: "death"` hits the **first** branch and
   `build_active_from_def` builds the whole `ActiveOverride` from the def. Any per-request params
   must be merged on top in all three branches or they vanish silently. All three branches are also
   gated on `candidate.priority >= active.priority` — a lower-priority request is dropped with no
   log.
2. **`action_executor_system` runs BEFORE `drain_spawn_queue_system`** (`lib.rs` chained set), and
   drain is rate-limited to `SPAWNS_PER_FRAME = 2`. So `Spawn(...)` + any action targeting that new
   spawn ID in the same `do_actions` list always warns "no entity with spawn id". `ActionQueue` is
   FIFO with no retry. The correct designer path is the spawned prefab's **own** `behavior:` file's
   `initial_state` `entry_actions`.
3. **`animation.rs`'s GLTF-respawn staleness check resets `last_played`** (documented as common on
   WASM when textures land after the initial scene spawn), forcing a second `transitions.play()`.
   Any seek/pause state must be **declarative on `ActiveOverride` and re-applied on every play**,
   never consumed-and-cleared, or the pose silently reverts to t=0 unfrozen on WASM only.
4. **Reusing a live monster's `animation_policy` on a corpse prop is a latent footgun**: its
   `base.idle` is a standing idle loop, and there are three fallback paths to it
   (resolver step 4 else-branch, step 4b graph-validation fallback, `animation.rs`'s missing-node
   reset) — any of them makes a corpse stand up and idle. A dedicated corpse policy with
   `base.idle` set to the death clip makes every fallback benign.
5. Touchpoints for adding a field to `PlayAnimationOn`: `message_interpreter.rs` ×2 (rewrite_self,
   rewrite_target), `dialogue.rs::substitute_self_in_action`, `action_executor.rs` (consume),
   `tests/corpse_loot_interact_tests.rs` (test-local rewrite_self clone). `action_bar.rs:391` and
   `ironhold_cli/query.rs:578` already use `{ .. }` and need no change. All the forwarding sites
   reconstruct with named fields and no `..`, so an omission is a **compile error, not a silent
   break** — unusually safe for this action family.
6. `ironhold_cli validate` parses no `prefabs/animation/*.ron` and validates no clip names at all —
   a numeric range typo has zero design-time catch today. Follow the established runtime-`warn!`
   + matching-`validate`-error twin pattern (see [[diagnostic_only_feature_pattern]]).
