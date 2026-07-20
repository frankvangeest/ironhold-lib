# Feature: Local Co-op Hot Join/Leave

_Status: Done (v1 shipped 2026-07-20)_
_Planned at: `a59815c` (2026-07-19)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Hot **join** (keyboard only) into an already-`Grid`-split scene, up to `MAX_SPLIT_PLAYERS`, incremental single-camera-add | Done | 2026-07-20 |
| v2 | Hot **leave** (despawn + slot renumber + cleanup) and gamepad-join | Queued | — |

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
unclaimed pad; deferred to v2 rather than conflated with v1's keyboard path.

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

## Open questions
- **(resolved)** Join prefab: per-slot `join_prefab_keys: Vec<String>`, not one shared key — a
  single shared prefab would give every joiner an identical (colliding) control scheme.
- **(resolved)** Join trigger scope: scene-wide (any press of the bound key claims the next open
  slot), auto-assigning the lowest free `PlayerIndex` — backed by the per-slot prefab list above so
  the correct scheme is always used for whichever slot opens next.
- v2 (leave) needs its own decision on player-state disposal (does a leaving player's `StatMap`/
  future inventory/quest progress vanish or persist for rejoin?) — explicitly deferred to v2's own
  plan-review, not decided here.
- v2 must also empirically verify the `Added<SplitViewportSlot>`-on-reinsert behavior before relying
  on any in-place slot-renumbering strategy for leave (the mutate-vs-reinsert distinction the
  architect flagged) — v1 doesn't need this since it never renumbers.

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
