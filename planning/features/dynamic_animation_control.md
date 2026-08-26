# Feature: Dynamic animation control (seek + freeze on `PlayAnimationOn`)

_Status: In Progress_
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
- [ ] `Action::PlayAnimationOn` — add `start_at_fraction: Option<f32>` + `freeze: bool` (`schema/actions.rs`)
- [ ] `AnimationRequests.queue`: `VecDeque<String>` → `VecDeque<AnimationRequest>` (clip/id name + the two new fields); give it a `From<&str>` or constructor so the existing one-liner call sites (`jump_enter`/`jump_exit` in `player.rs`, the `PlayAnimation` broadcast in `action_executor.rs`) don't need to change shape
- [ ] `ActiveOverride`: add `seek_fraction: Option<f32>` + `frozen: bool`, reset in `.clear()`; fix the pre-existing `looping` default/clear inconsistency while touching this struct (`#[derive(Default)]` yields `false`, `.clear()` sets `true` — currently benign since `clip` is `None` either way, but should not be left free to drift)
- [ ] `AnimationController`: add `pending_seek: bool`
- [ ] `animation_resolver_system`: merge `start_at_fraction`/`freeze` into all three `ActiveOverride`-construction branches via one shared helper; set `pending_seek = true` whenever a queued request is accepted, regardless of whether it names the already-current clip; force `looping = false` when `freeze` is requested
- [ ] `animation_playback_system`: replay condition becomes `current != last_played || pending_seek` (clearing `pending_seek` once applied); before calling `transitions.play()`, `.resume()` the previous clip's `ActiveAnimation` if it `.is_paused()`; after play, resolve clip duration via the `AnimationGraph`/`AnimationNodeType::Clip` and, when `seek_fraction` is set, `.set_seek_time(fraction * duration)` (not `.seek_to()`), then `.pause()` if `frozen`
- [ ] Clamp `start_at_fraction` to `[0.0, 1.0]` with a one-shot `warn!`; warn when a fraction is requested against a resolved-looping override
- [ ] `ironhold_cli validate`: new check — `PlayAnimationOn.start_at_fraction` outside `[0.0, 1.0]` in any rules/state_machine/behavior file → validate error (matches the `negative_coyote_time_secs` precedent; no `AnimationPolicy` file loading needed for this check)
- [ ] `cargo check -p ironhold_cli` — confirm `query actions` still compiles/reports correctly (struct-variant field addition, should be a no-op for `{ .. }`-style matches, but verify per the workflow's mandatory schema-change gate)
- [ ] Corpse fix: 3 new `prefabs/animation/corpse_policy_{zombie,snake,spider}.ron` files
- [ ] Corpse fix: `animation_policy:` added to `zombie_corpse`/`snake_corpse`/`spider_corpse` in `prefabs.ron`
- [ ] Corpse fix: `PlayAnimationOn(target: "{self}", clip: "death", start_at_fraction: 1.0, freeze: true)` added to `lootable_corpse.behavior.ron`'s `"fresh"` `entry_actions`
- [ ] New demo project `dynamic_animation_control` (frozen-pose scene + continue-from-fraction scene, RON/UI-button driven) + full new-project registration checklist
- [ ] Tests — resolver-level unit tests using a synthetic `AnimationController`/`graph_initialized: true` (mirroring `tests/scene_lifecycle_tests.rs`'s existing pattern, no real GLB assets needed) covering: (a) same-clip re-seek via `pending_seek` actually replays, (b) the override-id branch (not just raw clip name) honors `start_at_fraction`/`freeze` — the case most likely to be silently missed, (c) a previously-paused clip is resumed before its node plays again, (d) freeze state survives a simulated graph-reinit replay
- [ ] Test — `corpse_loot_interact_tests.rs`: a freshly-spawned corpse is immediately in its death pose (regression test for the actual bug this feature fixes)
- [ ] Docs — `docs/20_data_formats.md` (new fields + the documented `PlayAnimation`/`PlayAnimationOn` seek asymmetry), `docs/30_runtime_events_and_logic.md`, `crates/ironhold_core/src/CLAUDE.md` (resolver/playback field-ownership split — extend the existing "single writer of `AnimationController.current`" note, which is already slightly stale since the missing-node-index recovery path also writes it, into an explicit ownership table; document the "resume before replay" invariant from Task 2 above, since it is not otherwise rediscoverable from reading either function in isolation)

## Open questions
- Is `transition_ms: 0` required for the corpse's `PlayAnimationOn` call, or is the zombie
  policy's existing 200ms transition acceptable? There's no outgoing clip on a fresh corpse
  spawn, so the seeked pose should be exact either way, but worth confirming visually during
  playtest rather than assuming.
- Should the `AnimationOverrideDef`-level (policy-file) version of `start_at_fraction`/`freeze`
  go on the backlog now as a named future item, or just stay as a paragraph in this plan until a
  second use case actually wants it?

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
