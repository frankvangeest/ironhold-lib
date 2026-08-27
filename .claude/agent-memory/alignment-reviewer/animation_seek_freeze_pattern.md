---
name: animation-seek-freeze-pattern
description: Implemented pattern for Action::PlayAnimationOn.start_at_fraction/freeze — 5 forwarding touchpoints, the resolver 3-branch helper, and the freeze-without-fraction / unfreeze-in-place designer footguns
metadata:
  type: project
---

**Shipped 2026-08-26** on `feature/dynamic-animation-control` (branched from `integration`, not
`main`, because the corpse half depends on `monster_corpse_loot.md` v2). Field is
**`start_at_fraction`** (0.0–1.0 fraction of clip duration) + `freeze: bool` — NOT `start_at`;
the rename was deliberate because every other time-ish schema field is absolute and unit-suffixed.

**Why keep this:** this is now the reference implementation for "add a per-request parameter to an
action consumed via `AnimationRequests`". The three resolver branches are factored through one
`apply_seek_and_freeze(candidate, req)` helper specifically so a future field can't be wired into
only one branch — reuse that helper, don't add a 4th construction site.

**How to apply:** when reviewing any change to `animation_resolver.rs`/`animation.rs`, or any new
field on `AnimationRequest`/`ActiveOverride`/`AnimationController`.

## Touchpoints for a new `PlayAnimationOn` field (all verified present)

1. `schema/actions.rs` — `#[serde(default)]`, doc comment.
2. `message_interpreter.rs` ×2 — `rewrite_self` (~224) and `rewrite_target` (~317).
3. `capabilities/dialogue.rs::substitute_self_in_action` (~317).
4. `runtime/scene_manager/action_executor.rs` — consumes into `AnimationRequest`.
5. `tests/corpse_loot_interact_tests.rs::self_sub` — test-local rewrite clone.
   `action_bar.rs::action_needs_target` (~391) and `ironhold_cli/query.rs` (~578) both use
   `{ .. }` → no change needed. All 5 real sites reconstruct with named fields and no `..`, so an
   omission is a **compile error, not a silent break** — unusually safe for this action family.
6. `ironhold_cli/src/commands/validate.rs` — range check as `error_type:
   "animation_start_at_fraction_out_of_range"`. NOTE: `collect_actions` there covers
   rules.ron + state_machine.ron + `behaviors/*.behavior.ron` only — **dialogue `do_actions` are
   NOT walked**, so no action-level validate check applies to dialogue files. Pre-existing gap;
   re-flag whenever a new action-level validate check lands.

## Designer footguns introduced by this feature

- **`freeze: true` with no `start_at_fraction` is a partial no-op, not a no-op.** Schema doc says
  "has no effect when `start_at_fraction` is unset", but `apply_seek_and_freeze` still sets
  `frozen = true`, forces `looping = false`, and `wants_seek = true` (so the clip *replays*).
  Net effect: a looping clip silently becomes one-shot and never pauses. Doc/behavior mismatch.
- **No un-freeze in place.** `pending_seek` is only set when `start_at_fraction.is_some() ||
  freeze` — so re-issuing the same clip with `freeze: false` and no fraction does not replay and
  the `ActiveAnimation` stays paused. Workaround (RON-reachable, undocumented): pass an explicit
  `start_at_fraction`, e.g. `(clip: "x", start_at_fraction: 0.5, freeze: false)`.
- The clamp `warn!` and the looping+fraction≥1.0 `warn!` fire **per request**, not one-shot,
  despite the schema doc saying "one-shot warning".
- `freeze: true` against an override that has its own `duration:` still auto-expires on schedule
  (resolver's `expires_at`) and silently un-freezes. Fine for `death` (no duration); bites on
  `attack_light`/spider `attack`.
- `start_at_fraction`/`freeze` are **not** authorable on `AnimationOverrideDef` (policy file) —
  deliberately out of scope. Consequence: "spawn already posed" needs either a behavior file's
  `initial_state` `entry_actions` or a scene-level `scene.ready:{name}` rule targeting the spawn id.

## Corpse-pose recipe (3rd_person_game_demo)

Per-monster `prefabs/animation/corpse_policy_{zombie,snake,spider}.ron` with
`base.idle/walk/run/jump_loop` ALL set to the death clip (closes the 3 silent-fallback paths),
`overrides:` carrying only `death` (drop the live policy's `stop_action: "npc_revive"`), plus
`animation_policy:` on the `_corpse` prefabs and ONE line in the shared
`behaviors/lootable_corpse.behavior.ron` `"fresh"` entry_actions. Zombie needs
`animation_sources: ["anim_zombie","anim_locomotion","anim_hit_death"]`; snake/spider need none
(clips live in the model GLB).

## test_web.py has no scene-exclusion mechanism

`discover_scenes()` globs **every** `*.scene.ron` and baselines each one — a plan claiming a scene
is "excluded from screenshot baselines" cannot be honored today. Any demo scene containing a
*looping* animation will produce a flapping baseline against the 4% `BASELINE_DIFF_THRESHOLD`.
Flag this whenever a feature plan proposes a deliberately non-deterministic demo scene.
