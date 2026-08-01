# Feature: Local Co-op Hot Join/Leave

_Status: In Progress (v1 Done, v2 Revised after plan-review — recommend a confirmation pass before
implementation)_
_Planned at: `a59815c` (2026-07-19)_
_v2 drafted at: `1fcef14` (2026-07-31); revised after plan-review at `2026-08-01` (see the
"Revision" note in Approach — both system-architect and ux-gamedesigner-reviewer independently
found the seat/slot conflation bug; system-architect additionally resolved the trigger-mechanism
open question and found the executor param-budget/queued-join-renumbering gaps; ux-gamedesigner-
reviewer additionally specified the demo leave keys, the event-reachability gap, and the
accidental-press/hold-to-confirm requirement)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Hot **join** (keyboard only) into an already-`Grid`-split scene, up to `MAX_SPLIT_PLAYERS`, incremental single-camera-add | Done | 2026-07-20 |
| v2 | Hot **leave** (despawn + slot renumber + cleanup) | Revised — recommend confirmation pass | — |

**Plan-review note (2026-07-19):** Both reviewers returned Needs-more-design-work on the first
draft's "despawn every camera and rebuild the whole layout" strategy for join — resolved by a
complete redesign to an **incremental single-camera add** (below), which turned out to
simultaneously fix four separate issues the first draft had: (1) **system-architect** found the
original `assemble_split_layout(player_entities, player_configs, ...)` design was actually
uncallable at join time — existing players' `PlayerConfig`s are consumed at scene load and never
retained anywhere, so a full-layout rebuild couldn't reconstruct them; incremental add sidesteps
this entirely by never touching existing players. (2) The claimed `Added<SplitViewportSlot>`
double-spawn trap for in-place slot renumbering doesn't actually apply to a mutate-not-reinsert
approach — moot for v1 anyway now that existing cameras are never touched; flagged as a "verify
empirically, don't presuppose" item for v2's renumbering work instead. (3) Despawning the
currently-rendering camera risked a one-frame full-window flash; incremental add never despawns
anything. (4) `action_executor_system` is already at Bevy's 16-`SystemParam` ceiling and lacks
`DynamicStatUiQueue`/`ActiveTonemapping` — resolved by routing `Action::JoinPlayer` through the
existing deferred `PendingEntitySpawns`/`drain_spawn_queue_system` mechanism (which already has
both), exactly mirroring how `Action::Spawn`'s dynamic player path already sidesteps the same
ceiling, rather than plumbing new resources into the executor directly.
**ux-gamedesigner-reviewer** found: (a) a single scene-wide `join_prefab_key: String` would give
every joiner the *same* control scheme, colliding at the 3rd/4th seat — resolved with a per-slot
`join_prefab_keys: Vec<String>` list instead, reusing `local_coop_demo`'s existing per-scheme
prefabs (IJKL/Numpad) as the join-slot prefabs; (b) `room6` (the existing 4-way Grid demo) already
starts at the 4-player cap, so it can't demonstrate growth — resolved by wiring the demo into a
scene that starts at 2 players with 2 more join slots configured; (c) added tasks for joiner spawn
position, `PlayerIndex` assignment, the missing `rules.ron` wiring, and an on-screen join prompt
for in-play discoverability. Gamepad-join is deferred to v2 (its own new schema surface — binding a
gamepad button to a global, not-yet-claimed trigger — plus unclaimed-pad scanning logic; keeping
v1 keyboard-only avoids conflating two separate pieces of new input infrastructure).

**Amendment (2026-07-19, second review pass):** a follow-up verification round on this redesign
found the rewrite still had gaps: (1) **v1 scope narrowed to `Grid` only** — `ActiveSplitSlotCount`
is `Some` only for `Grid` (`None` for `Vertical`/`Horizontal`), and `split_screen_viewport_system`'s
`Vertical`/`Horizontal` arms are hardcoded to exactly 2 hardcoded halves (slot 0 vs. "else") — a 3rd
camera joined into a Vertical scene would silently overlap player 2's viewport rather than reflowing,
so the scope guard now checks specifically for `Grid`, not the original three-orientation list. (2)
The plan's "reads scene config directly" hand-wave for `join_prefab_keys` was the same class of gap
the original despawn-strategy blocker was — now named explicitly: `SceneHandleV2` +
`Res<Assets<GameSceneV2>>` (already kept current on every load), bundled into an existing
`SystemParam` struct since `action_executor_system` is at the 16-param ceiling. (3) A same-frame
double-join race was found — two `JoinPlayer` actions processed in the same executor run would both
read the same live player count before either flushes, producing a duplicate slot/`PlayerIndex`;
resolved by having the executor arm also account for already-queued `is_hot_join` entries when
computing the next slot, not just the live ECS count. (4) `join_spawn_point` was dropped in favor of
reusing the existing `spawn_points` map convention (`player_{N}_start` keys, already used by
`room6`) rather than introducing a second, parallel position mechanism. (5) The lobby-full on-screen
prompt needed an actual hide mechanism, not just a static Label — a `coop.lobby_full` event, emitted
when a join brings the count to the cap, wired via `rules.ron` to hide the prompt (with a fallback
to "always visible" if `SetEntityVisible` turns out not to target UI `Label` entities — needs
verification during implementation, flagged rather than assumed). (6) Minor precision: the join
task must call `spawn_player_entity_core` (camera-less) explicitly, not `spawn_player_entity`
(which spawns its own dedicated full-window camera — wrong for this feature).

## What
Lets a new player join an **already split-screen** running local-coop scene at runtime — a "press
to join" key spawns a new player entity and one new split camera, growing the layout live — instead
of player count being fixed for the scene's entire lifetime as it is today.

## Why
Today `spawn_players_and_camera` runs exactly once, at scene load, from a fixed RON-authored player
list. This backlog item was Icebox/undrafted, gated softly on Stage 6 (N-way `Grid` split) landing
first — which it has. `ActiveSplitSlotCount`'s own doc comment already anticipates this feature by
name: it's a stored, write-once resource specifically *so a future hot-join system has something
authoritative to rewrite* rather than fighting a live camera count.

## Approach

**Scope cut, validated against source**: `is_split_screen`-gated widget rank-duplication
(`stat_label`/`world_stat_bar`/`world_labels`/damage-popup ranks, and the runtime-computed
equivalent for dynamically-spawned widgets via `DynamicStatUiQueue`) is gated purely on a boolean
(`player_configs.len() >= 2 && first.camera.split.is_some()` at scene load; `active_split.0.is_
some() || dynamic_split.0.is_some()` at runtime for dynamic spawns) — **never on live player
count**. Once that boolean is true, all `MAX_SPLIT_PLAYERS` (4) ranks are already pre-allocated,
dormant, waiting for a camera. This means: **v1 is scoped to hot-joining into a scene that is
already `Grid`-split at load** (2+ players), growing up to `MAX_SPLIT_PLAYERS` — a 3rd/4th
joiner's stat labels/bars/nameplates/action-bar (if `owner_player`-scoped to their slot) all become
visible with **zero widget-side changes**, verified against both the scene-load and runtime-spawn
gates. A scene that starts single-player and would need to transition *into* split-screen live is a
structurally separate, larger problem and is not attempted here.

**Also explicitly out of scope for v1**: `Vertical`/`Horizontal` split (structurally capped at
exactly 2 — `ActiveSplitSlotCount` is `None` for these orientations, and `split_screen_viewport_
system`'s arms for them are hardcoded to exactly 2 halves with no reflow logic; a 3rd camera would
silently overlap player 2's viewport rather than joining a grid), `dynamic` split mode (hardcodes
exactly 2 targets by construction), `party` mode (its `Vec<Entity>` of targets is never mutated by
any system today), and primitive/capsule players (local co-op has never extended to them, per
`crates/ironhold_core/src/CLAUDE.md`'s "four player-construction sites" section) — GLB players only,
`Grid` orientation only, matching every other local-coop feature's own scoping precedent.

**v1 join strategy — incremental single-camera add, not a layout rebuild.** A join **never touches
any existing player or camera**. It: (1) builds the joiner's `PlayerConfig` from the scene's
per-slot `join_prefab_keys[current_live_count]`, (2) spawns exactly one new player entity, (3)
spawns exactly one new `OrbitCamera` tagged `SplitViewportSlot(current_live_count)` +
`Camera { order: current_live_count as isize, .. }` (a narrow new helper, `spawn_split_camera_for_
player`, extracted from the existing `Grid`-branch loop body — the *only* piece of `spawn_players_
and_camera` this feature factors out; nothing about the multi-player collection/dispatch logic
changes), and (4) increments `ActiveSplitSlotCount` by one. `split_screen_viewport_system` already
recomputes **every** camera's viewport from that stored count each frame via a live `Query` — so
existing cameras (slots `0..N-1`, untouched) and the new one (slot `N`) all reflow into the new
grid automatically, with no code change to that system at all. `Added<SplitViewportSlot>` fires
exactly once, for the new camera only, so its HUD "P{n}" label and target-HUD readout spawn
correctly without touching any `Added<>`-gated consumer.

**Executor plumbing — reuse the existing deferred-spawn queue, don't fight the param ceiling.**
`action_executor_system` is already at Bevy's 16-`SystemParam` ceiling and has neither
`DynamicStatUiQueue` nor `ActiveTonemapping` in scope — both needed to spawn a player entity
correctly. `Action::Spawn`'s existing dynamic-player path already hit this exact wall and resolved
it by deferring through `PendingEntitySpawns` → `drain_spawn_queue_system` (which *does* have both
resources). `Action::JoinPlayer` reuses the identical mechanism: the executor arm resolves the
joiner's config and pushes a `QueuedSpawn` carrying it, tagged with a new `is_hot_join: bool` flag.
`drain_spawn_queue_system` gains one new branch: when `is_hot_join` is set, instead of calling
`spawn_player_entity` (today's single-dedicated-full-camera path, wrong for split), it calls
`spawn_player_entity_core` directly (camera-less) followed by the new incremental camera add (steps
3-4 above). The `MAX_SPLIT_PLAYERS` cap and the "current split state is `Grid`" scope guard are
both checked at the executor arm, before enqueueing — a full or wrong-mode scene `warn!`s and
no-ops immediately, never reaching the queue. **The joiner's config comes from `SceneHandleV2` +
`Res<Assets<GameSceneV2>>`** (the scene handle is already kept current on every load) to read the
current scene's `join_prefab_keys` — bundled into an existing `SystemParam` struct (e.g.
`SceneStateParams`) rather than added as a bare `Res`, since `action_executor_system` is already at
Bevy's 16-param ceiling. **Same-frame double-join safety**: the "next slot" the executor arm assigns
is computed from the live player count *plus* any `is_hot_join` entries already sitting unflushed in
`PendingEntitySpawns` this frame — not just the live ECS count — so two `JoinPlayer` actions
processed in one executor run don't collide on the same slot/`PlayerIndex`.

**Join trigger (keyboard only for v1)** — reuses `global_input_system`'s existing designer-
authored `scene_key_bindings` → `UiEvent::ButtonPressed` mechanism (already global, not tied to any
player's `InputMap` — the correct property, since a not-yet-joined player has no `CharacterController`
yet). A scene authors e.g. `"KeyJ": "join"`; a `rules.ron` rule handles it and emits
`Action::JoinPlayer`. Gamepad join-press detection needs its own new schema surface (binding a
gamepad button to a global trigger — nothing like this exists today) plus a raw scan for an
unclaimed pad; **split out into its own feature doc, `planning/features/gamepad_hot_join.md`**
(2026-07-29), rather than folded into this file's v2 — it's a self-contained piece of new input
infrastructure with no dependency on hot-*leave*, matching this session's precedent of splitting
`gamepad_action_bar_slots.md` out from `gamepad_controller_input.md`. This file's v2 now covers
hot-leave only.

**Amendment (2026-07-20, real-hardware finding from `gamepad_controller_input.md`'s playtest):**
v2's "raw scan for an unclaimed pad" cannot be "any connected `Gamepad` entity with no player's
`gamepad_index` pointing at it" — confirmed live with a real Xbox 360 controller (Windows 11 +
Chromium-based browser) that a single physical controller can register as **two separate browser
gamepad entries**, one live and one permanently dead (reports zero for every axis/button forever).
An unclaimed-pad scan that doesn't check for *actual live input* (not just "connected, unclaimed")
would non-deterministically bind the join to the dead duplicate, producing a joined player who
never responds to anything — a much worse failure mode for hot-join than the existing static
`gamepad_index` limitation, since a designer/player has no way to retry with a different index once
the join has already happened. v2's scan must require some minimum real signal (e.g. a
button/axis actually reporting a nonzero pressed/analog state on the current frame) before treating
an unclaimed pad as "the" one to bind, not just presence in the connected-gamepads query. See
`planning/claude_suggestions.md`'s matching entry for the underlying observation.

**Amendment (2026-07-20, v1 implementation findings):** two review-pass findings resolved during
implementation, both worth noting for anyone extending this feature: (1) **1-based spawn-point
keys, not 0-based** — an early implementation looked up `spawn_points["player_{next_slot}_start"]`
with `next_slot` 0-based (matching `PlayerIndex`/`SplitViewportSlot` numbering), but every existing
scene (`room6`, `room8`) and the docs use 1-based `player_1_start`.."player_4_start" keys — the
0-based lookup silently resolved to an *existing* player's own spawn point (alignment-reviewer
finding), so the executor now looks up `player_{next_slot + 1}_start`. The test helper that builds
synthetic Grid scenes for hot-join tests originally used 0-based keys too, which matched (and
masked) the bug — fixed alongside the executor. (2) **One-frame empty-quadrant transient is
expected, not a bug** (system-architect finding): `drain_spawn_queue_system` sets
`ActiveSplitSlotCount` synchronously, but the new camera entity is created via deferred `Commands`
and doesn't exist until the next sync point — `split_screen_viewport_system` has no explicit
ordering against the drain system, so for one frame it can compute an N-cell grid while only N-1
cameras exist, rendering one clear-color quadrant. Benign and self-correcting (strictly better than
the despawn-rebuild full-window flash the original draft's design rejected), not worth ordering
against.

**Joiner spawn position and identity**: reuses the existing `spawn_points: Map<String,
(f32,f32,f32)>` scene convention (already populated for `room6`'s `player_3_start`/`player_4_start`)
rather than introducing a new, parallel position mechanism — the join path looks up
`spawn_points["player_{slot}_start"]`, falling back to the live primary player's current `Transform`
plus a small fixed `(1.5 * slot_index, 0, 0)` nudge only when that key is absent. `PlayerIndex` is
assigned deterministically as the joiner's target slot index (0-based) — guaranteed unique and never
`0` for a 2nd+ joiner (avoiding the existing "2+ players sharing `player_index: 0`" primary-target
warning) — **overriding** whatever `player_index` the joined prefab itself declares; this two-
sources-of-truth interaction (scene-load uses the prefab's baked index, join always overrides it
with the slot) must be documented so a designer reordering `join_prefab_keys` isn't surprised.

### v2 — Hot Leave

**Investigation ahead of drafting this section** (2026-07-31, grounding every claim below against
current source rather than the open questions' original guesses):

- **`SplitViewportSlot`/`Grid` math does not require contiguous slot values to render correctly on
  any single frame** — `split_screen_viewport_system`'s `Grid` branch (`capabilities/camera.rs`)
  computes `cols`/`rows` purely from `ActiveSplitSlotCount` and `row`/`col` purely from each
  camera's own `slot.0`; a sparse set (e.g. `{0, 1, 3}` with `count == 4`) renders every present
  slot correctly, with the missing cell simply blank (matches the existing "count == 3 leaves one
  cell empty" documented behavior — a genuinely new gap, not a special case). **But this masks a
  real freeze bug across a *second* departure**: when `cols*rows` shrinks below a surviving
  camera's `slot.0` on a later leave, that camera's `continue` (line ~453) skips writing
  `Camera.viewport` *for that frame only* — the field isn't cleared, so the camera keeps
  rendering a stale, now-wrong-shaped rect from the previous layout, indefinitely (nothing ever
  revisits it once its slot no longer fits `cols*rows`). **Conclusion: renumbering surviving
  players' `SplitViewportSlot` into a contiguous `0..new_count` range on every leave is required
  for correctness**, not just tidiness — this resolves the v1 plan's deferred "verify
  `Added<SplitViewportSlot>`-on-reinsert" open question by making clear renumbering is mandatory,
  not optional.
- **`PlayerIndex` and `SplitViewportSlot` are independent values that only hot-join happens to set
  equal** — every other consumer of `PlayerIndex` (the "P{n}" HUD label text, `PLAYER_LABEL_COLORS`
  tinting for both the HUD label and the target ring, `ring_layer_for_player` for
  `own_viewport_only`'s reserved `RenderLayers`, `ActionBar.owner_player` matching, "primary
  player" resolution) reads `PlayerIndex`, never `SplitViewportSlot` — confirmed by reading every
  call site. **This means hot-leave's renumbering only ever needs to touch `SplitViewportSlot` +
  `Camera.order` (pure viewport-layout geometry) — a surviving player's `PlayerIndex`, ring
  tint/layer, HUD label text/color, and action-bar ownership all stay exactly as they were before
  the leave.** No player's identity/color visibly changes because someone else left — a real UX
  risk this investigation ruled out rather than merely hoped away.
- **The renumbering write must be an in-place mutation of the existing `SplitViewportSlot(u32)`'s
  inner value (`slot.0 = new_value`), never a remove-then-reinsert.** Bevy's `Added<T>` filter only
  fires when a component is newly inserted onto an entity that didn't have it before — mutating an
  existing component's field never re-triggers it. `split_viewport_player_label_spawn_system` and
  `target_hud_spawn_system` both gate their one-time spawn on `Added<SplitViewportSlot>`; an
  in-place mutation is what keeps them from wrongly re-spawning a duplicate HUD label/target-HUD
  for a surviving player whose slot number just changed.
- **Nothing auto-cleans up a leaving player's split-screen-specific side entities.** Traced each:
  the split camera is a sibling entity (`OrbitCamera.target: Entity`, never `ChildOf` the player),
  so despawning the player does not cascade to it; the camera's `LinkedPlayerLabel`/
  `LinkedTargetHud` UI `Text` entities are likewise unparented siblings linked only by a stored
  `Entity`, not despawned by anything if their camera goes away; a `TrackingTarget` ring only
  despawns when its *tracked* entity dies or its *owner* retargets — neither condition fires just
  because the owner itself leaves, so a leaving player's ring becomes a permanently orphaned
  world-space decal until the next full scene reload. All of these need explicit despawn in the
  leave action; there is no shortcut.
- **`WorldLabelRank`-based widget duplication (`stat_label`/`world_stat_bar`/`world_labels`/
  damage-popup ranks) self-adjusts with zero cleanup needed** — it re-derives the live active-camera
  list from scratch every frame (`world_label_screen_pos_system`), never from a stored count, so a
  despawned camera simply drops out of next frame's selection.
- **`Action::Despawn(spawn_id)` is necessary but not sufficient for the player entity itself.**
  Confirmed `spawn_player_entity_core` already tags every player with `SpawnId`/registry entry via
  the shared `tag_spawned_entity` helper (so a despawn-by-id lookup is possible), and Bevy's
  recursive despawn correctly cascades to the player's own real ECS children (body meshes, cosmetic
  children) — but it cannot reach the camera/label/HUD/ring siblings above (no parent-child
  relation exists), and the camera entity has no `SpawnId` at all to target it by. A dedicated
  `Action::LeavePlayer` needs its own lookup logic for each of those, `Action::Despawn` alone
  cannot be reused as-is.
- **Nothing in this codebase reacts to a gamepad disconnecting** (confirmed: no
  `GamepadConnectionEvent`/`Disconnected` reader anywhere in `ironhold_core`). Auto-triggering a
  leave when a player's bound pad unplugs would need new event-reader plumbing with no existing
  precedent to build on — **out of scope for v2**, same "don't conflate two pieces of new input
  infrastructure" reasoning v1 used to split gamepad-join into its own feature. v2 is a deliberate
  keypress only, exactly like v1's join.

**Scope, mirroring v1's own narrowing precedent:**
- **`Grid`-split scenes only** — same reasoning as v1 (`Vertical`/`Horizontal` hardcode exactly 2
  halves with no reflow logic; `dynamic` hardcodes exactly 2 targets; `party`'s target `Vec<Entity>`
  is never mutated by any system today). Leaving in any other mode: `warn!` + no-op.
  **`local_coop_hot_join_leave.md`'s own hot-*joined* players are Grid-only already, so this is not
  a new restriction, just a continuation of v1's.**
- **GLB players only** — primitive/capsule local co-op is unsupported per
  `player_model_source_unification.md`; not attempted here.
- **No player-state persistence** — leaving is a full, permanent despawn. If a player later
  rejoins that (or any) slot via hot-join, they start fresh with no continuity to their previous
  session — no save/restore mechanism exists anywhere in this engine for an individual player's
  `StatMap`/inventory/progress across a leave, and building one is a separate, much larger feature.
  This directly resolves v1's own deferred open question ("does a leaving player's state vanish or
  persist for rejoin?") — **vanish**, matching the "keep scope narrow" precedent every other
  local-coop feature in this batch has followed.
- **Minimum 1 remaining player** — leaving is refused (warn + no-op) if it would drop the scene to
  0 live players; an otherwise-empty split-screen scene is an unexplored engine state (no camera
  at all, `ActiveSplitSlotCount` at `0`) with no clear designer-facing meaning. Flagged as an open
  question for ux-gamedesigner-reviewer: should this cap be exactly 1, or should leaving down to 0
  be allowed (falling back to... nothing? a title-screen-style pause?) — v1's own docs never
  addressed a 0-player scene either, so this isn't a regression, just an explicit decision v2 needs
  to make rather than inherit silently.

**Revision (2026-08-01, after plan-review — system-architect + ux-gamedesigner-reviewer):** both
reviewers independently found the same critical gap (the seat/slot conflation below), and
system-architect's investigation resolved the trigger-mechanism open question definitively — the
two options this draft originally offered are **both non-viable**: a direct `Action::LeavePlayer`
push from an input-detection system would violate `crates/ironhold_core/src/CLAUDE.md`'s explicit
"never push to `ActionQueue` from a capability system" rule (confirmed: the one prior violator,
the action bar, was already refactored *away* from direct pushes into the intent layer — this
would be a new, reintroduced violation, not a defensible mirror of `interact`), and a
`rules.ron`-authored `Action::LeavePlayer("{self}")` is **structurally unimplementable** today:
`rules.ron`/`state_machine.ron` matching is exact-string equality with no wildcard/capture
(`message_interpreter.rs`), `{self}` substitution only exists in `entity_fsm_interpreter_system`/
`dialogue.rs` and resolves against a *behavior file's owning entity* — but no player entity can
carry a behavior file (`PlayerConfig` has no `behavior` field; `attach_prefab_features` is never
called from the player spawn path) — and even if it could, a hot-joined player's spawn id is
runtime-generated (`format!("{}_{}", prefab_key, registry.counter)`), so a designer could never
author a rule matching it. The resolution below is a **third shape**, mirroring
`gamepad_hot_join.md`'s own `PendingJoinGamepad` pattern exactly, which sidesteps all of the above.

**Trigger — per-player `InputMap` field feeding an id-free event + a frame-scoped resource, not a
direct action push.** New `InputMap.leave: Option<String>` (keyboard) and `InputMap.gamepad_leave:
Option<String>` (gamepad, parsed via the existing `parse_gamepad_button`) — each already-joined
player binds their own leave key/button, per-player exactly like `interact`/`target_next` already
are (not a shared scene-level key like join's, since leave's identity is known instantly, unlike
join's). A new system in `runtime/input.rs` (alongside `unclaimed_gamepad_trigger_system`, not in
`capabilities/` — this is input translation, not gameplay logic), `.before(message_interpreter_
system)`:
1. Resets a new `PendingLeaveRequest(Option<String>)` resource to `None` unconditionally at the
   top of every run (same discipline as `PendingJoinGamepad`, so a stale spawn id can never survive
   into a frame that didn't actually request a leave).
2. Loops live `CharacterController`s; on the **first** (lowest-`PlayerIndex`) `just_pressed` match
   on that player's own bound `leave`/`gamepad_leave`, writes their spawn id into
   `PendingLeaveRequest` and emits `GameEvent::Trigger("coop.leave_requested")` — deliberately
   **id-free**, so it's actually authorable in `rules.ron` (unlike a per-spawn-id event string).
   **Capped at one request per frame** — a second presser's request is simply not serviced this
   frame (mirrors `unclaimed_gamepad_trigger_system`'s own one-match-per-frame cap and its stated
   reasoning: nothing downstream can safely disambiguate which of several same-frame events a
   shared resource's value belongs to); they can press again next frame.
3. A `rules.ron` rule handles it: `( on: "coop.leave_requested", do_actions: [
   LeavePlayer("{requester}") ] )` — `{requester}` is a new interpreter substitution token,
   resolved in the **executor** (not the interpreter) against `PendingLeaveRequest.take()`, so
   `rewrite_target`/`substitute_self`/`substitute_self_in_action` need **no new match arms** (each
   is an exhaustive per-variant match with `other => other`, so an unhandled token would otherwise
   silently pass through un-substituted). This gives a designer the same interception ability
   join's rules.ron indirection provides (e.g. gate the rule with `when: "not_boss_fight"`) with no
   new interpreter primitive and no pipeline-rule violation. `LeavePlayer("player_02")` (a literal
   spawn id, not the sentinel) still works too, for a scripted/scene-authored removal.
   **Documented limitation**: the `rules.ron` rule itself cannot distinguish *which* player
   requested the leave (it only sees the id-free trigger) — accepted for v2.

**`Action::LeavePlayer(String)` executor arm** (new action; either the literal `"{requester}"`
sentinel, resolved via `PendingLeaveRequest.take()`, or a literal spawn id):

1. Resolve the leaving player's `Entity` via `SpawnRegistry` (same lookup `Action::Despawn` uses);
   `warn!` + no-op if not found or not a live player.
2. **Refuse if this is the last live player** — read `ActiveSplitSlotCount` (not a live entity
   count: despawns are deferred via `Commands`, so a live query would still see 2 players if the
   last two both requested leave the same frame, defeating the floor for the second one;
   `ActiveSplitSlotCount` is decremented **synchronously** in step 7, so it's correct for a
   same-frame second leave) — `warn!` + no-op if `Some(1)`.
3. Find that player's own split camera: `Query<(Entity, &OrbitCamera, &SplitViewportSlot)>`
   filtered by `orbit.target == leaving_entity` — capture its `SplitViewportSlot.0` as
   `leaving_slot` before despawning anything.
4. **Despawn**, in order: the camera entity; its `LinkedPlayerLabel` UI `Text` entity (if present);
   its `LinkedTargetHud` UI entity (if the scene authors `target_hud:`); any `TrackingTarget` ring
   whose `owner == leaving_entity`; **every rank of any `stat_label`/`world_stat_bar` widget
   tracking the leaving player** (these are plain `WorldLabel`-tracked entities that
   `world_label_screen_pos_system` only ever hides, never despawns, once their tracked entity is
   gone — confirmed no despawn path exists for them today, and in a split-screen scene they're
   rank-duplicated ×`MAX_SPLIT_PLAYERS` each); then the player entity itself
   (`commands.entity(leaving_entity).try_despawn()` — recursive, cascades to real ECS children;
   `try_despawn()` not `despawn()`, per this codebase's despawn-discipline convention).
5. **Clear stale references to the leaver**: remove the entry from `SpawnRegistry` (mirroring
   `Action::Despawn`'s own `registry.entities.remove(&target_id)` — without this,
   `target_auto_clear_system` keeps silently resolving a dangling `Entity` every frame); if the
   leaver was the primary player, clear `CurrentTarget` (nothing else clears it on despawn, so
   `{target}` substitution in every rule would otherwise keep resolving to the departed player's
   last target indefinitely).
6. **Renumber**: for every *other* live split camera whose `SplitViewportSlot.0 > leaving_slot`,
   mutate `slot.0 -= 1` **in place** (never remove-and-reinsert — see the investigation note above)
   and set `Camera.order` to match. **Also renumber any already-queued `is_hot_join` entry in
   `PendingEntitySpawns` whose frozen seat index is above `leaving_slot`** — otherwise a join
   queued the same frame (or, since `SPAWNS_PER_FRAME` caps drains at 2/frame, a later frame) can
   spawn at a slot the shrunk grid no longer has room for, hitting the exact stale-viewport freeze
   this whole renumbering step exists to prevent, just reached via the queue instead of a live
   camera.
7. Decrement `ActiveSplitSlotCount` by one.
8. Emit the triple `GameEvent::Trigger("player.left")` (bare, always matchable — the workhorse for
   a generic "someone left" reaction), `GameEvent::Trigger(format!("player.left:{}", spawn_id))`
   (works only for scene-authored players with a designer-chosen id), and
   `GameEvent::Trigger(format!("player.left.index:{}", player_index))` (the seat number, matchable
   regardless of how the player was spawned — **document that index `2` is the player whose HUD
   label reads "P3"**, since `camera.rs` renders `P{player_index + 1}`, a guaranteed 0-based/
   1-based authoring trap otherwise). Add the symmetric `player.joined`/`player.joined.index:{n}`
   to `Action::JoinPlayer` in the same pass — v1 shipped with no join-side event at all, an
   asymmetry a designer reacting to leaves would immediately notice missing on the join side.

**Seat index vs. viewport slot — the critical fix both reviews independently found.** v1's
`Action::JoinPlayer` derives `player_index` from `next_slot = ActiveSplitSlotCount + queued_hot_
joins` — i.e. from the live **count**. Once a leave can shrink that count without renumbering
survivors' `PlayerIndex` (the whole point of the "identity never shifts" design above), the count
and the *set of already-used indices* can diverge: 4 players (indices 0-3), the player at index 1
leaves → survivors keep {0,2,3}, count becomes 3 → a subsequent join computes `next_slot = 3`,
**colliding with the still-live index-3 player** (duplicate "P4" label, duplicate ring tint/layer,
two players sharing one `join_prefab_keys` control scheme and moving in lockstep). **Fix, now in
v2's scope**: `Action::JoinPlayer`'s slot computation splits into two independent numbers —
**seat index** (`PlayerIndex`, `join_prefab_keys[..]`, `player_{n+1}_start`, `ActionBar.owner_
player` — stable per player, join claims the **lowest free seat** among currently-live
`PlayerIndex` values, not the count) and **viewport slot** (`SplitViewportSlot`/`Camera.order` —
contiguous `0..count`, renumbered by leave exactly as steps 3/6 above already specify). This
directly resolves the v2 acceptance criterion that join-after-leave must never collide, which the
original single-`next_slot` design could not satisfy. `crates/ironhold_core/src/CLAUDE.md`'s
"hot-join can NOT diverge here, since `Action::JoinPlayer` sets both `player_index` and the spawn
slot to the same `next_slot` value" becomes false under this fix and must be corrected in the docs
task.

**Losing the primary player (`PlayerIndex(0)`).** If the seat-0 player leaves, the seat-index fix
above means nothing is auto-promoted to primary (`is_primary_player` checks `PlayerIndex(0)`, and
every live player already carries an explicit `PlayerIndex`). **Decision: allow it and document the
degradation, don't refuse it** — refusing would make the primary player uniquely unable to leave
for reasons invisible to them, breaking the "any player can choose to leave, it's their own
controller" symmetry the rest of this feature establishes. Consequence, documented rather than
silently accepted: `CurrentTarget` mirroring, primary-player `{target}` substitution, and any
`owner_player: None`/`Some(0)` action bar all go dormant (gracefully — each already degrades to a
no-op on a missing match, confirmed) until a future join happens to land on seat 0 again (which,
under the seat-reuse fix, it naturally will — the next join claims the lowest free seat).

**Executor param budget.** `SpawnParams`/`SceneStateParams`-style bundles are already at Bevy's
16-`SystemParam` ceiling (the same wall v1's own plan already accounts for). The `LeavePlayer` arm
needs several new queries (camera+slot+label+HUD lookup, the ring query, the widget-rank query) and
critically needs `ActiveSplitSlotCount` promoted from `Res` (its current, v1-established contract)
to `ResMut` — a new bundled `SystemParam` (e.g. `LeaveParams`) is required, not a bare-param
addition. Two existing doc comments become stale and need updating in the same change:
`ActiveSplitSlotCount`'s own doc comment in `runtime/scene_manager/mod.rs` (currently states
`drain_spawn_queue_system` is the sole `ResMut` owner), and the `JoinPlayer` arm's invariant
comment block in `action_executor.rs` (currently assumes the executor never writes this resource).

**Demo**: extend `local_coop_demo/room8` (the existing hot-join demo) with per-player leave
bindings on all four seats — **`"KeyC"` (P1), `"Delete"` (P2), `"Semicolon"` (P3), `"Numpad6"`
(P4)** (verified free of collision against every key room8's four prefabs already bind, adjacent to
each player's own cluster so it's reachable without looking away from their half of the screen) —
plus a `leave_hold_secs: f32` hold-to-confirm (see below) and an on-screen banner reacting to
`player.left`. Also author all four `join_prefab_keys` slots in `room8.scene.ron` (today only
slots 2/3 are set; under the seat-reuse fix, a P1/P2 departure must be re-joinable too, or that
seat is permanently dead for the rest of the demo).

**Accidental-press protection.** Leaving is an irreversible, state-destroying action with all four
`local_coop_demo` seats sharing one physical keyboard — an unconfirmed instant-leave on a stray
keypress is a real risk the original draft didn't address. New `InputMap.leave_hold_secs: f32`
(`#[serde(default)]` = `1.0`) requires the bound key/button to be **held**, not just pressed, before
the leave-request system (above) fires — this is new timer-accumulator infrastructure (no
hold-duration primitive exists elsewhere in this codebase today; a per-player `Local`/component
accumulator is the natural shape). While holding, emit `GameEvent::Trigger("player.leaving.index:
{n}")` so a designer can render "P3 leaving…" feedback; emit `"player.leave_cancelled.index:{n}"`
if released early. Without *some* feedback, a 1-second hold reads as a broken key, not a
confirmation gesture.

## Tasks
- [x] Extract `spawn_split_camera_for_player(config: &PlayerConfig, player_entity: Entity, slot:
      u32) -> Entity` from the existing `Grid`-branch loop body (`entity_spawner.rs:706-721`) —
      the only factoring this feature needs; `spawn_players_and_camera` itself is otherwise
      untouched
- [x] New scene-level `join_prefab_keys: [Option<String>; MAX_SPLIT_PLAYERS as usize]` (or an
      equivalent fixed-size, slot-indexed structure — accessed via `.get(slot)`/pattern-matching
      with a `warn!` + no-op fallback, never a direct panicking index) — reuse `local_coop_demo`'s
      existing IJKL/Numpad-scheme prefabs as the 3rd/4th join-slot entries
- [x] New `Action::JoinPlayer` (no payload) — schema + executor arm: checks the `MAX_SPLIT_PLAYERS`
      cap and the "current split state is `Grid`" scope guard (both `warn!` + no-op on failure),
      computes the next slot from live player count *plus* already-queued `is_hot_join` entries
      (same-frame double-join safety), reads `join_prefab_keys[slot]` via `SceneHandleV2`/
      `Res<Assets<GameSceneV2>>` (bundled into an existing `SystemParam` struct, not a bare `Res` —
      `action_executor_system` is at the 16-param ceiling), builds the joiner's `PlayerConfig` via
      `assemble_player_config`, overrides `PlayerIndex` to the slot index, resolves spawn position
      via `spawn_points["player_{slot}_start"]` (fallback: primary player's live position + a small
      offset), and pushes a `QueuedSpawn` with a new `is_hot_join: bool` flag onto
      `PendingEntitySpawns`. When the join brings the count to `MAX_SPLIT_PLAYERS`, also emit a
      `coop.lobby_full` event
- [x] `drain_spawn_queue_system`: new branch for `is_hot_join` — calls `spawn_player_entity_core`
      (camera-less — **not** `spawn_player_entity`, which spawns its own dedicated full-window
      camera), then `spawn_split_camera_for_player`, then increments `ActiveSplitSlotCount` by one
      (all using this system's existing `DynamicStatUiQueue`/`ActiveTonemapping` access — no
      executor param plumbing needed for spawn-time resources)
- [x] Keyboard join-press: reuse `global_input_system`'s existing `scene_key_bindings` → `UiEvent::
      ButtonPressed` → rules → `Action::JoinPlayer` path (no new input-system code)
- [x] Demo — wire a `local_coop_demo` `Grid` scene that **starts at 2 players** (not `room6`, which
      already starts at the 4-player cap and so can't demonstrate growth) with `join_prefab_keys`
      pointing at the existing `player_p3_grid`/`player_p4_grid` prefabs, `player_3_start`/
      `player_4_start` spawn points, a `"KeyJ": "join"` binding, and the matching `rules.ron` entry
      (`ui.button_pressed:join → Action::JoinPlayer`)
- [x] Demo — a static on-screen `Label` ("Press J to add a player"), hidden on `coop.lobby_full` via
      a `rules.ron` rule + `SetEntityVisible` — **verify during implementation that
      `SetEntityVisible` actually targets UI `Label` entities** (documented today against 3D
      entities); if it doesn't, fall back to an always-visible prompt for v1 rather than blocking on
      new UI-visibility plumbing
- [x] Tests — regression: both existing `spawn_players_and_camera` call sites are unaffected (this
      feature adds a new helper, it does not modify the existing function); new: joining a 2-player
      Grid scene up to 3 then 4 players produces the correct camera count/viewport layout each time,
      with existing players' cameras/`PlayerIndex`/`StatMap` completely untouched; new: joining at
      the `MAX_SPLIT_PLAYERS` cap warns and no-ops; new: a joined player's rank-`N` stat-bar/world-
      label/action-bar widgets (if `owner_player`-scoped) become visible with no additional wiring
      (validates the scope-cut thesis for both scene-load-gated and runtime-`DynamicStatUiQueue`-
      gated widgets); new: `Action::JoinPlayer` in a `dynamic`/`party`/single-camera scene warns and
      no-ops; new: the joiner gets a unique `PlayerIndex` and doesn't trip the "shared `player_index:
      0`" warning; new: two `Action::JoinPlayer`s processed in the same executor run assign distinct
      slots, not a collision; new: `Action::JoinPlayer` in a `Vertical`/`Horizontal` scene warns and
      no-ops (not just `dynamic`/`party`)
- [x] Docs — `docs/20_data_formats.md`: `Action::JoinPlayer`, `join_prefab_keys`, the join-key-
      binding pattern, the cap/`Grid`-only scope-guard behavior, the `spawn_points["player_{slot}_
      start"]` convention, and the `PlayerIndex`-override-beats-prefab-baked-index interaction
- [x] Docs — `crates/ironhold_core/src/CLAUDE.md`: update the "four player-construction sites"
      section (adds a fifth: the `is_hot_join` branch in `drain_spawn_queue_system`) and resolve
      `ActiveSplitSlotCount`'s own forward-reference to this feature

### v2 tasks
- [ ] New `InputMap.leave: Option<String>` + `InputMap.gamepad_leave: Option<String>` +
      `InputMap.leave_hold_secs: f32` (`#[serde(default)]` = `1.0`) fields (`schema/player.rs`);
      `gamepad_leave` parsed via the existing `parse_gamepad_button`. `InputMap::key()`/
      `gamepad_button()` need an `Option`-aware match arm for `"leave"`, not the bare-`String` arm
      every other field uses — call this out explicitly so it isn't discovered mid-implementation.
- [ ] New per-player hold-timer accumulator (new infrastructure — no hold-duration primitive
      exists elsewhere in this codebase) + leave-request system in `runtime/input.rs` (beside
      `unclaimed_gamepad_trigger_system`): resets a new `PendingLeaveRequest(Option<String>)` to
      `None` unconditionally every run; on a live player's `leave`/`gamepad_leave` binding held for
      `leave_hold_secs`, writes their spawn id into it and emits `GameEvent::Trigger("coop.leave_
      requested")` (id-free, capped at one request/frame); emits `player.leaving.index:{n}` while
      holding and `player.leave_cancelled.index:{n}` on early release
- [ ] New `{requester}` executor-side substitution token (resolved against
      `PendingLeaveRequest.take()` — deliberately NOT added to `rewrite_target`/`substitute_self`/
      `substitute_self_in_action`, which stay untouched)
- [ ] New `Action::LeavePlayer(String)` — schema + executor arm implementing the 8-step sequence in
      Approach: last-player refusal (via `ActiveSplitSlotCount`, not a live query), camera/label/
      HUD/ring/stat-widget lookup and despawn, `SpawnRegistry`/`CurrentTarget` clearing, in-place
      `SplitViewportSlot`/`Camera.order` renumbering (including any queued `is_hot_join` entry's
      frozen seat), `ActiveSplitSlotCount` decrement, the triple `player.left*` event emission
- [ ] New bundled `SystemParam` (e.g. `LeaveParams`) for the executor arm's new queries; promote
      `ActiveSplitSlotCount` from `Res` to `ResMut` and update its stale doc comment in
      `runtime/scene_manager/mod.rs` plus the `JoinPlayer` invariant comment block in
      `action_executor.rs` (both currently assume only `drain_spawn_queue_system` writes it)
- [ ] **Split `Action::JoinPlayer`'s slot computation into seat index vs. viewport slot** — join
      now claims the lowest free `PlayerIndex` among currently-live players (seat index; also used
      for `join_prefab_keys`/`player_{n+1}_start`/`ActionBar.owner_player`), independent of
      `ActiveSplitSlotCount` (viewport slot, contiguous, unaffected by which specific seats are
      occupied). This is a v1-code-touching fix, now in v2's scope — see Approach's "critical fix"
      note for why the original single-`next_slot` design collides after any leave.
- [ ] Symmetric `player.joined`/`player.joined.index:{n}` events on `Action::JoinPlayer` (v1 shipped
      with no join-side event; adding it now avoids a designer-visible asymmetry)
- [ ] Scope guards mirroring v1's join guards: `warn!` + no-op when the current split state isn't
      `Grid`, and when leaving would drop the scene below the minimum-1-player floor
- [ ] Demo — `local_coop_demo/room8`: per-player leave bindings on **all four** seats (`"KeyC"` P1,
      `"Delete"` P2, `"Semicolon"` P3, `"Numpad6"` P4 — verified collision-free against every key
      the room's four prefabs already bind), all four `join_prefab_keys` slots authored (today only
      2/3 are — under the seat-reuse fix, any seat must be re-joinable after a leave), an on-screen
      banner reacting to `player.left`/`player.leaving.index:{n}`, and updated per-player hint
      labels (`controls_hint_p3`/`p4` currently go stale if that player leaves — make them bound
      labels, blanked from a `player.left.index:{n}` rule)
- [ ] Tests — regression: v1's hot-join tests are unaffected by the leave action itself (though the
      seat/slot split above *does* touch `Action::JoinPlayer` — re-verify v1's existing join tests
      still pass against the split computation, not just "unmodified"); new: leaving a **middle**
      slot (not just the last) renumbers every higher surviving slot's `SplitViewportSlot`/
      `Camera.order` down by one, contiguous `0..new_count`; new: a surviving player's `PlayerIndex`,
      ring tint/layer (if `own_viewport_only`), HUD label text/color, and `ActionBar.owner_player`
      matching are all completely unchanged by a leave; new: **a join immediately after a middle-slot
      leave assigns the freed seat index, not a colliding one** (the critical-fix regression test);
      new: the leaving player's camera, `LinkedPlayerLabel`/`LinkedTargetHud` entities, any
      `TrackingTarget` ring, and any `stat_label`/`world_stat_bar` widget ranks they owned are all
      despawned with no orphans; new: `SpawnRegistry`/`CurrentTarget` no longer reference the
      leaver; new: `ActiveSplitSlotCount` decrements correctly; new: leaving the sole remaining
      player warns and no-ops (verified via a same-frame two-leave race, not just a single call);
      new: leaving in a non-`Grid` scene warns and no-ops; new: `Added<SplitViewportSlot>` does NOT
      re-fire for a renumbered survivor's camera; new: a queued `is_hot_join` entry's frozen seat
      is correctly renumbered when a leave lands before it drains
- [ ] Docs — `docs/20_data_formats.md`: `Action::LeavePlayer`, the new `InputMap` fields (including
      `leave_hold_secs`), the `coop.leave_requested`/`{requester}` mechanism, the `player.left*`/
      `player.joined*` event names, the minimum-1-player floor, the `Grid`-only scope guard, the
      seat-index-vs-viewport-slot distinction, and "leave keys must be distinct per player because
      the keyboard is shared" (mirroring the existing per-scheme key-collision guidance)
- [ ] Docs — `crates/ironhold_core/src/CLAUDE.md`: add a sixth player-construction-adjacent site
      (leave is a *de*-construction site with its own multi-entity cleanup contract); correct the
      now-false "hot-join can NOT diverge here... sets both `player_index` and the spawn slot to the
      same `next_slot` value" sentence in the `per_viewport_target_ring_visibility` bullet

## Open questions
- **(resolved, v1)** Join prefab: per-slot `join_prefab_keys: Vec<String>`, not one shared key — a
  single shared prefab would give every joiner an identical (colliding) control scheme.
- **(resolved, v1)** Join trigger scope: scene-wide (any press of the bound key claims the next open
  slot), auto-assigning the lowest free `PlayerIndex` — backed by the per-slot prefab list above so
  the correct scheme is always used for whichever slot opens next.
- **(resolved, v2)** Player-state disposal on leave: **vanish, no persistence** — no save/restore
  mechanism exists anywhere in this engine for per-player state across a leave, and building one is
  out of scope; a later rejoin of that (or any) seat starts fresh, matching hot-join's own "joiners
  always start fresh" behavior.
- **(resolved, v2)** The `Added<SplitViewportSlot>`-on-reinsert question v1 deferred: renumbering
  must be an **in-place mutation** of the component's inner `u32` (`slot.0 -= 1`), never a
  remove-then-reinsert — confirmed against Bevy's `Added<T>` semantics.
- **(resolved, v2, post-review)** Trigger mechanism: neither of the two options this draft
  originally posed is viable (a direct action push violates the pipeline rule; a `rules.ron`-
  authored `Action::LeavePlayer("{self}")` is structurally unimplementable — no wildcard rule
  matching, no behavior file on players, runtime-generated hot-join spawn ids). Resolved to a third
  shape mirroring `PendingJoinGamepad`: an id-free `coop.leave_requested` event + a frame-scoped
  `PendingLeaveRequest` resource + a `{requester}` token resolved in the executor. See Approach.
- **(resolved, v2, post-review)** Seat index vs. viewport slot: `Action::JoinPlayer` must derive
  `PlayerIndex` from the lowest **free seat**, not the live camera **count** — see Approach's
  "critical fix" note. Without this, join-after-leave can collide two players onto one `PlayerIndex`.
- **(resolved, v2, post-review)** Losing the primary player (seat 0 leaves): allow it, document the
  degradation (primary-player-scoped content goes dormant until a future join lands on seat 0
  again), don't refuse it — refusing would single out the primary player as uniquely unable to
  leave for no reason visible to them.
- **(resolved, v2, post-review)** Minimum-1-player floor: keep it (not "allow leaving to 0") — a
  0-player scene has no defined engine state (no camera at all under the Grid math), while 1 player
  is a clean, already-correct state (`cols=1,rows=1` renders a full-window single view). "Drop out
  of co-op" and "quit to a menu" are different, and this engine has no menu/lobby scene for the
  latter yet.
- **(resolved, v2, post-review)** Demo leave keys: `"KeyC"` (P1), `"Delete"` (P2), `"Semicolon"`
  (P3), `"Numpad6"` (P4) — verified collision-free against room8's four prefabs' existing bindings,
  each adjacent to that player's own key cluster.

## Acceptance criteria
- Given a `local_coop_demo` Grid scene starting with 2 players, when the join key is pressed, then
  a 3rd player spawns (using the correct `join_prefab_keys` scheme, e.g. IJKL) and the viewport
  layout recomputes to a 3-cell grid live, with no scene reload, and the existing 2 players' cameras/
  state are completely unaffected (**browser-observable**).
- Given the same scene at 4 players (the cap), when the join key is pressed again, then nothing
  spawns and a clear `warn!` is logged — no crash, no 5th camera.
- Given a freshly-joined 3rd/4th player, when they move into view, then their stat label/world stat
  bar/nameplate render correctly in their own viewport with no additional per-widget wiring beyond
  what v1 touches.
- Given a `Vertical`/`Horizontal`/`dynamic`-split or `party`-mode scene, when the join key is
  pressed, then nothing happens beyond a clear `warn!` — only `Grid` scenes support hot-join in v1.
- Given the on-screen join prompt, when 4 players are present, then the prompt hides (via
  `coop.lobby_full`) or, if `SetEntityVisible` can't target it, remains a documented always-visible
  fallback rather than misleadingly implying a 5th join is possible.

### v2 acceptance criteria
- Given a 4-player `Grid` scene, when the player at a **middle** slot (not the highest) holds
  their own leave key for `leave_hold_secs`, then that player, their camera, HUD label, target HUD
  (if authored), target ring, and stat widgets all disappear, and the layout recomputes to a 3-cell
  grid live with no scene reload (**browser-observable**).
- Given the same leave, when observing the two surviving players who were at higher slots than the
  one who left, then their HUD label text/color and target-ring tint are completely unchanged —
  no visible identity shift, even though their `SplitViewportSlot`/viewport position moved
  (**browser-observable**).
- Given a player releases their leave key/button before `leave_hold_secs` elapses, then no leave
  occurs and a `player.leave_cancelled.index:{n}` event fires — accidental taps don't remove a
  player (**browser-observable**).
- **Given a 4-player scene where the player at seat index 1 leaves, when a new player then joins,
  then the joiner is assigned `PlayerIndex(1)`** (the freed seat), not a value colliding with any
  still-live player's — the concrete regression test for the seat-index-vs-viewport-slot fix
  (**browser-observable**: the joiner appears with the same colour/HUD-label identity the departed
  P2 had, not a duplicate of an existing player's).
- Given the seat-0 (primary) player leaves a 2+-player scene, when the scene continues, then no
  crash occurs and primary-player-scoped content (the legacy `target_display` var, any `owner_
  player: None` action bar) goes dormant until a later join lands on seat 0 again — documented
  degradation, not a silent break.
- Given a 1-player-remaining scene, when that player holds their leave key, then nothing happens
  beyond a clear `warn!` — leaving the last player is refused, including when two players attempt
  to leave in the same frame (only one can succeed, per the `ActiveSplitSlotCount`-based check).
- Given a `Vertical`/`Horizontal`/`dynamic`-split or `party`-mode scene, when a leave key is
  pressed, then nothing happens beyond a clear `warn!` — only `Grid` scenes support hot-leave,
  matching v1's join scope.
- Given a scene combining hot-join and hot-leave in the same session, when a player leaves and a
  different player later joins, then the joiner's assigned seat index never collides with any
  still-live player's `PlayerIndex` — this is now a structural guarantee (seat index is derived
  from the live `PlayerIndex` set, not the camera count), not merely an emergent property.
