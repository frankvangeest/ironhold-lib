# Feature: Dynamic animation control (seek + freeze on `PlayAnimationOn`)

_Status: Done_
_Planned at: `237e548` (2026-08-26)_

> **Branched from `integration`, not `main`** (deviation from the usual `feature/{slug}` convention
> in root `CLAUDE.md`): the corpse-fix half of this plan edits `zombie_corpse`/`snake_corpse`/
> `spider_corpse` prefabs and `behaviors/lootable_corpse.behavior.ron`, which only exist on
> `integration` — `monster_corpse_loot.md` v2 hasn't been promoted to `main` yet (held back per
> Frank, more features to batch first). Branching from stale `main` would mean this feature
> can't see those files at all. Merge back into `integration` as normal once done.

<!-- Phases table — add only for multi-phase features; delete this block for single-phase ones -->

## What

Lets a designer command any animated entity to jump to an arbitrary point in a clip's
duration — 0%, 50%, 75%, 100%, anything in between — and choose whether playback then
continues from there or freezes at that exact frame. Exposed as two new optional fields on
the existing `Action::PlayAnimationOn { target, clip }`:

```ron
PlayAnimationOn(target: "{self}", clip: "death", start_at_fraction: 1.0, freeze: true)
```

`start_at_fraction` (0.0–1.0, default 0.0) is a fraction of the clip's duration, not seconds.
`freeze` (default `false`) pauses the clip exactly at that point instead of letting it play on.

Ships with two concrete uses of the new primitive: fixing `zombie_corpse`/`snake_corpse`/
`spider_corpse` (currently rendered in bind/rest pose — they have no `animation_policy` at
all) so they spawn already posed in their death frame, and a new standalone demo project,
`dynamic_animation_control`, that showcases the full fraction/freeze/continue matrix so a
designer can see and reproduce the capability without touching Rust.

## Why

Two independent needs converge on the same missing primitive:

1. **A real, currently-shipping visual bug.** `assets/projects/3rd_person_game_demo/prefabs/prefabs.ron`'s
   three corpse props have no `animation_policy`, so a killed monster's corpse shows the raw
   GLB in bind pose, not lying dead — the death animation the player just watched on the live
   monster is not reflected on its corpse at all. Every one of the three monster policies
   (`player_policy_zombie.ron`, `snake_policy.ron`, `spider_policy.ron`) already declares a
   `death` override with `looping: false`, so the fix only needs the corpse to *start* at that
   override's final frame — which needs an arbitrary-seek primitive, since simply re-applying
   the `death` override on the corpse would replay the whole clip from t=0 (3s for the zombie),
   duplicating what the player already watched on the now-despawned original entity.
2. **No existing action can start a clip anywhere but t=0.** `Action::PlayAnimationOn` always
   plays from the beginning; there is no authoring-level way to pose an entity mid-clip, freeze
   it there, or scrub through a clip for QA/cutscene/debug purposes. This is a general engine
   gap, not specific to corpses — Bevy's `bevy_animation` crate already exposes exactly the
   primitives needed (`ActiveAnimation::set_seek_time`/`pause`/`resume`, confirmed against the
   vendored `bevy_animation-0.18.0` source), so this is wiring, not new engine machinery.

## Approach

### Schema

Extend the existing struct-variant `Action::PlayAnimationOn` (not the tuple-variant
`Action::PlayAnimation(String)` — extending a tuple variant is a breaking RON change for every
existing call site, and it broadcasts to every entity with `AnimationRequests` rather than a
specific target, so it's the wrong fit anyway):

```rust
PlayAnimationOn {
    target: String,
    clip: String,
    #[serde(default)]
    start_at_fraction: Option<f32>,
    #[serde(default)]
    freeze: bool,
},
```

`start_at_fraction`, not `start_at` — every other time-ish field in this schema is absolute and
unit-suffixed (`duration`, `delay_secs`, `transition_ms`, `coyote_time_secs`); an unsuffixed
`start_at` reads as seconds to anyone used to the rest of the schema. Both fields are
`#[serde(default)]`, so every existing `PlayAnimationOn(target:, clip:)` call site across every
project keeps parsing unchanged. `PlayAnimation(String)` is explicitly out of scope for seeking —
document this asymmetry in `docs/20_data_formats.md` (a designer who wants this on the player
uses `PlayAnimationOn(target: "player_01", ...)`).

### Internal plumbing — four correctness issues found in design review, all must be fixed before this ships

Two independent pre-implementation reviews (system-architect, alignment-reviewer) surfaced the
same call-site blocker plus three Bevy-animation-specific bugs the naive version of this design
would ship with. All four are addressed here, not deferred:

1. **The corpse's `PlayAnimationOn` cannot fire from the monster's own death/swap sequence.**
   `action_executor_system` runs before `drain_spawn_queue_system` in `lib.rs`'s chained
   `Update` set, and `Action::Spawn` only enqueues a `QueuedSpawn` — the corpse entity (and its
   `AnimationRequests` component) does not exist yet within the same action list that calls
   `Spawn(...)`. Putting `PlayAnimationOn(target: "{self}_corpse", ...)` in
   `enemy_zombie.behavior.ron`'s `zombie.swap_to_corpse` handler hits `action_executor.rs`'s
   "no entity with spawn id" warning and silently drops, with no retry (`ActionQueue` is FIFO,
   one-shot). **Fix:** fire it from `behaviors/lootable_corpse.behavior.ron`'s `"fresh"` state
   `entry_actions` instead (alongside the existing `SetDespawnTimer`), which runs against the
   corpse's own `{self}` once *its* behavior asset resolves. This is also the better design —
   the death-pose concern belongs in the corpse's own file, matching how the decay-timer
   ownership is already reasoned about there. Race-safe: `AnimationRequests` is inserted at
   spawn time regardless of whether the animation policy has finished loading yet — a queued
   request just waits in the `VecDeque` until `AnimationPolicyComponent` attaches.

2. **A frozen (paused) clip is never released and poisons every later clip on the same entity.**
   `AnimationTransitions::play`'s fade-out guard skips creating a transition for the outgoing
   clip when it `is_paused()`. A paused clip therefore never decays out via
   `advance_transitions`/`expire_completed_transitions` — it sits in `AnimationPlayer`'s
   `active_animations` at weight 1.0 forever, permanently blended against whatever plays next.
   Invisible for a corpse (nothing else ever plays on it again) but immediately visible in the
   `dynamic_animation_control` demo, which cycles freeze on and off across many clips on the
   same entities. **Fix:** in `animation_playback_system`, immediately before calling
   `transitions.play(...)` for a new clip, resolve the *previous* (`last_played`) clip's node
   index and call `.resume()` on it if `.is_paused()`.

3. **Re-seeking within the same clip is a silent no-op.** Playback only re-triggers when
   `controller.current != controller.last_played`. If a request targets the clip that's already
   current (e.g. "jump to 25%, then to 75%, same clip" — exactly the demo project's QA matrix),
   nothing happens. **Fix:** add `AnimationController.pending_seek: bool`, set by the resolver
   whenever it accepts a *new* queued request (even one naming the already-current clip) and
   cleared by playback once applied. Playback's replay condition becomes
   `current != last_played || pending_seek`. `seek_fraction`/`frozen` stay durable fields on
   `ActiveOverride` (not consumed-and-cleared) — this is also what keeps a frozen corpse's pose
   surviving `animation.rs`'s documented GLTF-hierarchy-respawn recovery path (a WASM-specific
   case where Bevy's `SceneSpawner` replaces the animated hierarchy after initial spawn,
   forcing a second `transitions.play()`); if the seek were one-shot, that second play would
   silently restart the clip from t=0, unfrozen, at exactly the point this feature is meant to
   prevent it — a web-only regression that would be hard to reproduce and diagnose.
4. **Use `set_seek_time`, not `seek_to`.** `ActiveAnimation::seek_to` intentionally replays every
   animation event between the old and new time on the next update; a 0→duration jump would
   replay the clip's entire event track. `set_seek_time` is the documented no-events variant.
   Harmless today (no GLB in this project uses animation events) but not a safe assumption to
   leave standing.

Additional design decisions:

- **Merge `start_at_fraction`/`freeze` into all three of `animation_resolver_system`'s
  `ActiveOverride`-construction branches** (override-id lookup, semantic `clips` alias, raw clip
  name) — not just the raw-clip-name branch, which is the intuitive-but-wrong single site. The
  corpse case specifically resolves via the **override-id** branch (`clip: "death"` matches
  `AnimationOverrideDef.id == "death"` in every monster policy, it is not a raw clip name), so
  an implementation that only wires the new fields into the raw-clip-name branch will silently
  drop them for exactly this feature's primary use case. Factor the three branches' construction
  through one shared helper so this can't diverge again (this codebase has hit the "N near-
  identical construction sites drift apart" bug shape before — see `tag_spawned_entity`'s doc
  comment).
- **`freeze: true` forces the resulting override's `looping` to `false`** — pausing and looping
  are contradictory, and the existing resolver hardcodes `looping: true` for the semantic-alias
  and raw-clip branches. Document that a `freeze: true` request against an override that also
  has its own `duration: Some(_)` will still auto-expire that override on schedule
  (`animation_resolver_system`'s existing `expires_at` check) and silently un-freeze — not an
  issue for `death` (no `duration` in any of the three monster policies) but worth a doc-comment
  callout since it will surprise someone reusing this on `attack_light` (which does have
  `duration`).
- **Resolve clip duration via the existing `AnimationGraph`/`AnimationNodeType::Clip`**, not a
  new parallel `clip name → Handle<AnimationClip>` map. `animation_playback_system` already has
  the graph's `AnimationNodeIndex` and access to `Assets<AnimationGraph>`; a second map keyed by
  clip name duplicates `node_indices`' key space and is exactly the "two maps built from one
  source, allowed to desync" shape this codebase has already had bugs from.
- **Clamp `start_at_fraction` to `[0.0, 1.0]` with a one-shot `warn!`** at the point of use
  (defense in depth — see CLI validate below for the design-time catch). Also warn when a
  fraction is requested against an override whose `looping` resolves to `true`: `update()`'s
  `seek_time %= clip_duration` means a fraction ≥ 1.0 on a looping clip wraps to effectively
  `0.0`, which is very unlikely to be what was intended outside a one-shot pose.
- **Not in scope for this pass:** promoting `start_at_fraction`/`freeze` to authorable fields on
  `AnimationOverrideDef` (the per-character policy file's `overrides:` list), plus an
  `AnimationPolicy.initial_override` for a fully declarative "spawn already posed, no behavior
  file needed" prop. There is exactly one authoring site for this feature's own corpse use case
  (the shared `lootable_corpse.behavior.ron`), so there's no repetition to eliminate yet — revisit
  if a second use case wants the same frozen fraction fired from several scattered RON sites.

### Corpse fix

- Add three small, corpse-specific animation policy files —
  `prefabs/animation/corpse_policy_{zombie,snake,spider}.ron` — rather than pointing the corpse
  prefab at the *live* monster's full policy file. Same `animation_sources` GLBs (already cached
  from the live monster), but `base.idle`/`walk`/`run`/`jump_loop` all point at the death clip
  and `overrides` carries only the `death` entry, copied from the monster's own policy. This
  closes three independent silent-fallback paths that would otherwise make a corpse visibly
  "stand up and idle" (the no-active-override fallback, the missing-node-index recovery fallback,
  the graph-validation fallback) — with the full monster policy reused as-is, any of those three
  landing on `base.idle` would resolve to a standing idle loop, which is a strictly worse
  degraded state for a corpse than for a live monster.
- Add `animation_policy: "prefabs/animation/corpse_policy_zombie.ron"` (and snake/spider
  equivalents) to the three `_corpse` prefab entries in
  `assets/projects/3rd_person_game_demo/prefabs/prefabs.ron`. `spawn_prefab_instance` already
  attaches the full animation component stack to any prefab kind with `animation_policy` set,
  `Prop` included — no engine change needed for this half.
- Add one line to `behaviors/lootable_corpse.behavior.ron`'s `"fresh"` state `entry_actions`:
  `PlayAnimationOn(target: "{self}", clip: "death", start_at_fraction: 1.0, freeze: true)`.
  Since the file is shared and `{self}`-relative, this one line covers all three corpse types
  (and any future one) with no per-monster duplication.

### `dynamic_animation_control` demo project

New project under `assets/projects/`, driven entirely by `logic/rules.ron` and UI buttons
(`ui.button_pressed:{id}` → `PlayAnimationOn(...)`) so the no-recompile claim is visually
self-evident and reproducible by a designer. Two scenes/sections, split deliberately:

- **Frozen poses** (deterministic) — one row per fraction (0%, 50%, 75%, 100%) with
  `freeze: true` and `transition_ms: 0`, so each is an exact, reproducible pose. This scene is
  the project's `screenshot_baselines/scenes/dynamic_animation_control_main.png` regression
  guard for the whole seek path.
- **Continue-from-fraction** (non-deterministic by construction — excluded from screenshot
  baselines) — the same fractions with `freeze: false`, plus at least one case seeking into a
  **looping** clip mid-loop, to visually confirm playback resumes/wraps sensibly from a non-zero
  start rather than only ever being exercised against one-shot clips.

Standard new-project registration (root `CLAUDE.md`): add to `test_web.py`'s `PROJECTS` list,
generate the baseline screenshot, add an `index.html` gallery card, add any new GLB/animation
catalog entries to the project's `assets.ron`, then `python tools/asset_checker/check.py` and
`python tools/build_asset_manifest.py`.

## Tasks
- [x] `Action::PlayAnimationOn` — add `start_at_fraction: Option<f32>` + `freeze: bool` (`schema/actions.rs`)
- [x] `AnimationRequests.queue`: `VecDeque<String>` → `VecDeque<AnimationRequest>`, with `From<&str>`/`From<String>` so `jump_enter`/`jump_exit`/the `PlayAnimation` broadcast stay one-liners
- [x] `ActiveOverride`: `seek_fraction`/`frozen` added; `.clear()` simplified to `*self = Self::default()` (post-review fix — was a hand-enumerated field list, exactly the shape that already drifted once)
- [x] `AnimationController`: `pending_seek: bool` + `graph_handle: Option<Handle<AnimationGraph>>` (the latter added mid-implementation — see below)
- [x] `animation_resolver_system`: `apply_seek_and_freeze` merges into all three branches; `pending_seek` is assigned (not just set-true) per accepted candidate, gated on `wants_seek` so an ordinary non-seek re-request keeps its old no-restart behavior; NaN fraction laundered to `0.0` before clamping (post-review fix)
- [x] `animation_playback_system`: `current != last_played || pending_seek` replay gate; resumes a previously-paused clip before switching away from it; resolves clip duration via `AnimationController.graph_handle` (not the entity's `AnimationGraphHandle` component, which is deferred-command-stale for one frame during a re-init — found by the test suite, not by inspection); `set_seek_time` (not `seek_to`); `set_repeat(Forever/Never)` unconditionally every play, not just conditionally (post-review fix — see below); `pause()`/`resume()` unconditionally based on `frozen`, independent of whether a fraction was given (post-review fix — see below)
- [x] Clamp `start_at_fraction` to `[0.0, 1.0]` with a runtime `warn!` (not one-shot — corrected in docs to match); warn when a fraction is requested against a resolved-looping override
- [x] `ironhold_cli validate`: new `animation_start_at_fraction_out_of_range` check, covering rules/state_machine/behavior files
- [x] `cargo check -p ironhold_cli` clean; `cargo run -p ironhold_cli -- query actions` spot-checked (`PlayAnimationOn ×12` reported correctly)
- [x] Corpse fix: 3 new `prefabs/animation/corpse_policy_{zombie,snake,spider}.ron` files
- [x] Corpse fix: `animation_policy:` added to `zombie_corpse`/`snake_corpse`/`spider_corpse` in `prefabs.ron`
- [x] Corpse fix: `PlayAnimationOn(target: "{self}", clip: "death", start_at_fraction: 1.0, freeze: true)` added to `lootable_corpse.behavior.ron`'s `"fresh"` `entry_actions`
- [x] New demo project `dynamic_animation_control` (frozen-pose scene + continue-from-fraction scene, RON/UI-button driven, all 3 `PlayAnimationOn` resolution branches exercised) + new-project registration (`test_web.py` PROJECTS list, `index.html` card — baseline screenshot still pending, see Playtest below)
- [x] Tests — `animation_seek_freeze_tests.rs`, 8 tests (grew from the originally-planned 4 during post-implementation review): same-clip re-seek, override-id branch, paused-clip resume, freeze-survives-reinit, **`freeze: true` with no `start_at_fraction` still pauses** (regression for a real bug found by 3 independent reviews), **`should_loop: false` on a same-node re-seek doesn't stay stuck at `RepeatAnimation::Forever`** (regression for a real bug found by system-architect + debug-detective), out-of-range clamp, NaN-fraction laundering
- [x] Test — `corpse_loot_interact_tests.rs::a_freshly_spawned_corpse_immediately_requests_its_frozen_death_pose` (verified meaningful by debug-detective: proves the override-id resolution path specifically, not a trivial pass)
- [x] Test infra fix — `tests/support/mod.rs` needed `.init_asset::<AnimationClip>()` added (was entirely missing; every test touching `animation_playback_system` panicked once `Res<Assets<AnimationClip>>` was added — found immediately by the first full-suite run)
- [x] Docs — `docs/20_data_formats.md`, `docs/30_runtime_events_and_logic.md` (+ new "adding a corpse for a new monster type" checklist), `docs/STATUS.md`, `assets/projects/CLAUDE.md`, `crates/ironhold_core/src/CLAUDE.md` (new "Animation resolver/playback pipeline" section with the field-ownership table)

**Two real bugs found by the mandatory post-implementation review pass (alignment-reviewer, system-architect, debug-detective, ux-gamedesigner-reviewer, wasm-perf-reviewer — all 5 launched in parallel), fixed before merge:**
1. **`freeze: true` with no `start_at_fraction` silently did nothing** — `pause()` lived inside the `if let Some(fraction) = seek_fraction` block, so a freeze-only request replayed the clip once and held its *last* frame (via Bevy's own non-looping-completion behavior) instead of pausing at frame 0 as documented. Found independently by 3 of the 5 reviewers. Fixed by hoisting `pause()`/`resume()` out to run unconditionally on `active_override.frozen`.
2. **A same-node re-seek could get stuck at `RepeatAnimation::Forever`** — `AnimationPlayer::start()` (called by `transitions.play()`) reuses the existing `ActiveAnimation` when replaying the *same* node index, and its `.replay()` resets timing fields but not `repeat`. The old code only called `.repeat()` conditionally (`if should_loop`), with no `else` — harmless before this feature (same-node replay was unreachable), but `pending_seek` makes it reachable (e.g. an entity idling on a looping base clip, then seeked via a non-looping override on the *same* clip name — exactly the corpse policies' own `base.idle == death clip` shape). Found by system-architect and debug-detective independently, both citing the vendored Bevy source. Fixed by calling `set_repeat(Forever/Never)` unconditionally every play.

Also fixed as part of the same pass: `pending_seek` is now assigned per-candidate rather than only ever set true (closes a same-frame priority-race edge case); `ActiveOverride::clear()` simplified to `*self = Self::default()`; NaN fraction laundered before clamping; `test_web.py`'s baseline-screenshot function checked browser console errors *before* the create-baseline early-return, not after (previously skipped entirely on a project's first-ever baseline run); a real `NON_DETERMINISTIC_SCENES` exclusion mechanism added to `test_web.py` for `continue.scene.ron` (its header comment previously claimed an exclusion that didn't exist); the demo's `flycam` speed tuned down from engine defaults (100/200 units/sec is sized for terrain-scale worlds, not a 24m diorama); stale `PlayAnimationOn` signatures fixed in `docs/30_runtime_events_and_logic.md`, `docs/STATUS.md`, `assets/projects/CLAUDE.md`; `continue.scene.ron` gained a 4th entity exercising the raw-clip-name resolution branch (closing a real gap — the file's own comments claimed 3 branches were exercised while only 2 were).

**Logged to `planning/claude_suggestions.md` as separate, out-of-scope follow-ups** (not fixed here): `Action` has no `deny_unknown_fields`, so a typo'd field name silently no-ops rather than erroring (needs a full RON sweep across every project before adopting — too large a change to fold into this feature); `ironhold_cli validate`'s action-collector never walks dialogue `do_actions`; the test harness has no real `bevy::animation::AnimationPlugin`, so Bevy's own fade-out/event systems never actually run in any test (found to matter for one test's assertion meaningfulness); a frozen `AnimationController` pays full per-frame skeletal evaluation forever, not just at the seek moment (Bevy quirk — `paused` only gates event triggering, not sampling); promoting `start_at_fraction`/`freeze` to `AnimationOverrideDef` + a new `AnimationPolicy.initial_override` for fully declarative posing (this feature's demo needed 7 rule lines to pose 7 props, which is exactly the repetition that would justify it — deferred per this plan's original scope boundary, revisit if a second real use case appears).

## Open questions
- **Resolved during review** — `transition_ms: 0` is not required for the corpse. Confirmed by
  system-architect: there's no outgoing clip on a fresh corpse spawn, so `AnimationTransitions`
  has nothing to blend from and the seeked pose is exact regardless of transition duration.
  `corpse_policy_zombie.ron` keeps the original 200ms (matching the live monster's policy) for
  consistency; the demo's own `zombie_policy.ron` still uses `0` since it *does* need an exact
  pose the instant it's set, for the screenshot baseline. Still worth a visual glance during
  playtest, but not expected to matter.
- **Resolved during review** — the `AnimationOverrideDef`-level version stays a paragraph in this
  plan, not a named backlog item yet. Logged to `planning/claude_suggestions.md` instead (see
  the "Two real bugs" section above) with the concrete evidence for when to promote it: if a
  second use case needs the same frozen fraction fired from several scattered RON sites (this
  feature's own demo needed 7 rule lines for 7 props, which is suggestive but not yet a second
  independent use case).

## Playtest checklist

`python serve.py` then open both `?project=3rd_person_game_demo` and
`?project=dynamic_animation_control`. **Note: this session's sandboxed headless-Chromium
environment has no WebGPU adapter available at all** (confirmed against an existing,
already-shipped project too — not specific to this feature), so none of the below could be
verified in-session. All of it needs a real browser pass.

**Round 1 playtest finding, fixed:** Frank found the exact race debug-detective's original
"theoretical" note (below, struck through) was actually predicting, every time, not just on cold
cache — a corpse briefly rendered T-pose, then played through its death fall, then **snapped back
to standing and fell again** before settling. Root cause: `animation_policy_loader_system` wrote
`controller.current = policy.base.idle` (= the death clip, for a corpse) directly, the instant the
policy asset loaded — bypassing the seek/freeze machinery entirely. Since that load path is
faster than the one that eventually applies the real posing request (behavior file loads →
entry_actions fire → action queue → executor → animation request → next resolver tick), the death
clip played unseeked and looping (`should_loop: true`, the un-corrected default) for however many
frames the slower path took to catch up — visually: falls (first loop), loop wraps back to start
("stands"), falls again (the real request finally wins and freezes it).

**Fix:** added `AnimationPolicy.initial_override: Option<String>` — an override id applied
*synchronously* the moment the policy attaches, before any fallback window can be reached at all.
`AnimationOverrideDef` gained its own `start_at_fraction`/`freeze` fields (consulted only by
`initial_override`, not by ordinary `PlayAnimationOn` calls referencing the same override id —
keeps the two paths independent, no ambiguity about which `freeze: false` means "explicit" vs
"inherit"). All three corpse policies now set `initial_override: "death"` with
`start_at_fraction: 1.0, freeze: true` on the override itself; the now-redundant
`PlayAnimationOn` in `lootable_corpse.behavior.ron` was removed. New unit tests in
`animation_seek_freeze_tests.rs` cover `resolve_initial_override` directly (4 tests, pure
function, no ECS needed) plus updated `corpse_loot_interact_tests.rs` coverage proving the pose
is correct *immediately*, with zero updates, not eventually. Console logs from a real playtest
(with temporary `[DIAG]` logging, since removed/downgraded to `debug!`) confirmed the fix applies
correctly — exactly one playback event, seeked and frozen, no looping.

**Round 2 playtest finding, also fixed:** with the looping bug gone, Frank confirmed what remained
was a *separate*, smaller issue — the corpse still briefly shows in bind pose (reads as
"standing") before settling into its death pose, since the GLTF mesh can render before the
animation graph finishes initializing and applying the pose. General engine characteristic (true
for any animated entity, not corpse-specific) that predates this feature, but Frank asked for it
to be fixed before this batch ships. **Fix:** `AnimationController.awaiting_reveal: bool` — when
`initial_override` resolves, the entity is spawned `Visibility::Hidden` and only revealed
(`Visibility::Inherited`) by `animation_playback_system` once that exact pose is confirmed
applied. Deliberately scoped to `initial_override` users only, not every animated entity —
hiding players/NPCs too would risk a much longer invisible window on a slow connection where the
GLTF mesh itself is still streaming in, for no benefit (an ordinary `base.idle` fallback has no
"wrong" pose worth hiding). New regression test
`awaiting_reveal_entity_stays_hidden_until_the_pose_is_confirmed_applied`. Full test suite
re-confirmed green (22/22 binaries); WASM dev build rebuilt — **ready for a third playtest round.**

**Round 3 playtest finding, fixed:** the bind-pose flash from round 2 persisted after the
hide-until-revealed fix. Two debug-detective + system-architect investigation passes found the
real mechanism: the corpse was spawned with default Visibility::Inherited, and bevy_scene's
SpawnScene step instantiates the (often GLB-cache-warm) mesh hierarchy in that same spawn frame
- rendering one real frame in bind pose - well before animation_policy_loader_system (a separate,
unordered system group) even gets a chance to see the entity, and that system is itself gated
behind an async AnimationPolicy RON fetch. The awaiting_reveal hide only ever fired after that
first bad frame had already rendered. Fix: hide at spawn itself (Visibility::Hidden in the same
command batch spawn_prefab_instance creates the entity with), for every animation_policy entity,
not just ones with initial_override - widening the hide from "only known-posed entities" to "any
entity whose pose isn't confirmed yet" is what actually closes the gap, since there's no way to
know synchronously whether the not-yet-loaded policy has an initial_override at all. This makes a
stuck-hidden-forever entity a real new failure mode, so a bounded 5-second failsafe
(awaiting_reveal_since + a check in animation_playback_system) force-reveals if a pose is never
confirmed (broken animation_policy/model reference) - an incorrect pose beats a permanently
invisible entity. The loader's "no initial_override" branch now also reveals immediately instead
of relying on animation_playback_system's play block to do it. Full test suite green; WASM dev
rebuilt; playtest confirmed the flash is gone.

**Round 4 playtest finding, fixed:** with the flash gone, a new (expected, given round 3's fix)
gap appeared - the original monster despawns in the same event batch that queues the corpse
spawn, but the corpse now stays hidden for several frames while its animation_policy loads, so
there's a visible window with nothing on screen between "monster gone" and "corpse revealed."
System-architect found the key fact that made this a one-line RON fix instead of a new engine
primitive: the dying monster's own death clip (should_loop: false) is already frozen on its exact
last frame - the same GLB, same clip, same sample time, same at_entity-copied transform the
corpse spawns into - so overlapping them for a second is a bit-identical no-op, not a crossfade
problem. Fix: replaced the immediate Despawn("{self}") with SetDespawnTimer(entity: "{self}",
delay_secs: 1.0) in the swap_to_corpse handler of all three enemy_*.behavior.ron files - the old
monster just stays visible a beat longer while the corpse loads underneath it. No new Action, no
new event, no schema change. (Considered and rejected: a dedicated Action::ReplaceEntity - it
can't be atomic without the same readiness signal internally, and would duplicate Action::Spawn's
~130 lines of resolution logic.) Playtest confirmed by Frank: seamless, no gap, no flash -
3rd_person_game_demo corpse fix fully confirmed working.

**Round 5 playtest finding, fixed:** dynamic_animation_control's own UI (not the corpse fix) had
overlapping, illegible text - both the per-model captions and the top-left title/hint/button/
legend column. Root cause: every Label/Button UI node renders at a hardcoded font size (22px/26px
respectively) regardless of the RON size: field, which only sets the layout box - a fact that
cost several iterations to pin down empirically (real per-character width is much wider than a
first guess, ~15px/char at 22px font). Fix: replaced each zombie's full-sentence world-space
caption with a short colour-coded token ("0%"/"50%"/"75%"/"100%" in main.scene.ron, "A"-"D" in
continue.scene.ron) plus a screen-space legend panel keyed by the same colours (the "contact
sheet" pattern already used by custom_materials/particles_demo) - considered and rejected a
one-at-a-time click-through catalog per ux-gamedesigner-reviewer's advice, since this demo is a
parameter sweep whose whole teaching value is comparing all four at once, and a real catalog
would need net-new per-page interpreter-state machinery this codebase doesn't have (no
value-based LogicRule conditions, no dynamic button text). Added label_depth_scale to both scenes
(8 of ~14 other example projects already have it; this one was the outlier), narrowed specimen
spacing to a 3m pitch so nothing falls outside the camera frustum on a 16:9-or-narrower canvas,
and re-spaced/shortened the top UI column (title/camera hint/nav button/legend) to clear the real
22px/26px line heights. Also replaced em-dash with an ASCII hyphen in all newly-authored text -
the game's embedded font has no glyph for it (confirmed pre-existing in custom_materials too, out
of scope to fix broadly here). ironhold_cli validate + ron_lint/ron_validation green after every
iteration; baseline screenshot regenerated with a real, non-headless GPU browser (python
test_web.py --real-gpu, Playwright's bundled Chromium - no system browser install needed).
Playtest confirmed by Frank.

**`3rd_person_game_demo` — the corpse fix:**
- [x] Kill a zombie/snake/spider; confirm the corpse appears immediately lying in its death pose
      — no T-pose/standing flash at all now (not just no fall→stand→fall glitch — the bind-pose
      flash itself should be gone too), no replay of the death animation.
- [x] Kill the same monster slot a second time (wait for or trigger respawn); confirm the new
      corpse also poses correctly and the old one's decay/interact/loot behavior is unaffected
      (this is exactly what `corpse_loot_interact_tests.rs` already covers headlessly — this is
      the visual confirmation).
- [x] ~~Specifically watch for a brief flash of movement on a corpse's very first spawn in a
      fresh session~~ — was the round-1 looping bug, confirmed fixed.
- [x] ~~Bind-pose/standing flash before the correct pose settles~~ — was the round-2 finding,
      fixed via hide-until-confirmed above.

**`dynamic_animation_control` — the new demo:**
- [x] `main.scene.ron` loads with 4 zombies frozen at 0%/50%/75%/100% of the death clip, each
      visibly further along than the last (0% = still standing, 100% = fully down).
- [x] Multi-line entity labels (`\n` in `label: text:`) actually render as separate lines and
      don't overlap neighboring labels — this is the first use of a multi-line world label in
      this repo, so it needs visual confirmation, not just an assumption it works.
- [x] Flycam moves at a sane speed for the 24m scene (tuned down from engine defaults during
      review — confirm it doesn't still feel too fast/slow).
- [x] "View continue-playing examples" button navigates to `continue.scene.ron`; its 4 zombies
      show: two `death`-clip entities visibly finishing their fall and holding the last frame,
      one visibly walking in a loop (seeked to 50% first), one in an idle loop (the raw-clip-name
      case). "Back to frozen poses" button returns correctly.
- [x] No console errors/warnings beyond expected ones on either scene.

**Once playtest is confirmed**, the baseline screenshot for `main.scene.ron` still needs
generating from a real browser environment: `python test_web.py --project
dynamic_animation_control --update-baselines --skip-build` (the `continue` scene is excluded from
baselining on purpose — see `test_web.py`'s `NON_DETERMINISTIC_SCENES`).

## Acceptance criteria
- Given `PlayAnimationOn(target: "npc_01", clip: "wave", start_at_fraction: 0.5, freeze: true)`,
  when executed, then the entity's pose matches 50% of `wave`'s duration and stops advancing.
- Given the same command with `freeze: false`, then playback continues forward from the 50%
  point instead of restarting at 0%.
- Given a zombie/snake/spider is killed, when its corpse spawns, then it appears immediately in
  the death pose — no bind/T-pose, no idle stand, no replay of the death animation.
- Given two different clips are frozen back-to-back on the same entity, then the earlier frozen
  clip does not remain permanently blended into whatever plays next.
- Given the same clip is re-seeked to a different fraction while already current, then the new
  seek takes effect immediately.
- `cargo test -p ironhold_core --test '*'` and `cargo check -p ironhold_cli` both pass.
