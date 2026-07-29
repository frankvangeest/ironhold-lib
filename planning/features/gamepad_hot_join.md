# Feature: Gamepad-Triggered Hot Join

_Status: Ready_
_Planned at: `f60bd33` (2026-07-29)_

**Plan-review note (2026-07-29):** both reviewers returned Needs-more-design-work on the first
draft. **system-architect** found two blocking issues with the original `PendingJoinGamepad(
Option<Entity>)` design: (1) it can't correctly pair two gamepads pressed in the same frame with
their two resulting `Action::JoinPlayer`s — resolved by capping detection to **at most one**
gamepad-triggered join per frame (mirrors the existing `SPAWNS_PER_FRAME` one-thing-at-a-time
precedent), with the test restated as consecutive frames rather than a literal same-frame double
press; (2) the resource could go stale — a *non-join* gamepad binding (e.g. `"Start":
"toggle_pause"`) on an unclaimed pad would set it with nothing to consume it, so a later
*keyboard*-triggered join could silently inherit a wrong pad — resolved by having detection
unconditionally reset the resource to `None` at the start of every run, only setting it when a
qualifying press is found that same frame, with the detection system ordered `.before(
message_interpreter_system)` (matching `global_input_system`'s existing ordering) so consumption
is guaranteed same-frame. The architect also found the original "live-signal" filter was solving a
problem that doesn't need new logic: since a phantom/dead duplicate gamepad (the documented Xbox
360 dual-registration quirk) reports **zero for every button, forever**, requiring `just_pressed`
specifically on the *bound* button already excludes it — there is no separate "any nonzero signal"
prefilter to build, which also answers both original open questions (no N-frame confirmation
window needed; stick movement doesn't need to count as liveness, only the bound button's edge
matters). Also flagged and fixed: the exclusion set must also cover gamepads mid-flight through
`PendingEntitySpawns` (an `is_hot_join` entry not yet drained), mirroring the `queued_hot_joins`
same-frame-safety precedent already in `Action::JoinPlayer`; the schema-mirroring task under-named
its own dependencies (`ProjectGamepadBindings` sibling resource, and **three** merge sites — two in
`project_loader.rs`, one in `scene_loader.rs` — not one); and a documented, accepted limitation
that a runtime-assigned `gamepad_index` inherits the same positional-index fragility (a mid-session
disconnect/reconnect re-routes it) that RON-authored `gamepad_index` already has today.
**ux-gamedesigner-reviewer** found the RON surface itself is sound (authorable by analogy, no new
button vocabulary), but the plan had no player-facing discoverability task at all — added: the
demo's join prompt must show real controller button names (e.g. "A / Cross"), never the engine's
internal `"South"` string; the demo's P3/P4 keyboard control-hint labels become misleading for a
gamepad-driven joiner (their prefab's own `inputs:` keyboard fields, including camera-look, go
inert once `gamepad_index` is overridden — confirmed against the actual `InputMap` resolution code)
and need rewording; and "does the very first press count, or does discovery require a priming
press" needed to be a stated acceptance criterion, not left implicit, distinguishing it from the
known WASM/Chrome quirk where a brand-new pad's first interaction can double as the browser's own
activation gesture (a documented browser limitation, not an engine bug). Both reviewers'
docs-scope findings are folded into the Tasks/Docs item below.

## What
Adds a gamepad equivalent of the existing keyboard hot-join trigger: a new player can join an
already-`Grid`-split local co-op scene by pressing a button on an **unclaimed** physical gamepad,
with that specific gamepad immediately bound to the new player — no separate assignment step, no
existing player's controller affected.

## Why
`local_coop_hot_join_leave.md` v1 shipped keyboard-only by explicit scope cut; gamepad-join was
named there as deferred v2 work. Today's gamepad support (`gamepad_controller_input.md`) is 100%
per-already-joined-player — `InputMap.gamepad_index` is a static, RON-authored index resolved via
`resolve_gamepad`, and nothing emits a global trigger the way `scene_key_bindings` does for
keyboard. Realistic couch co-op needs "plug in a controller, press a button, you're in" — this is
the last piece blocking that.

## Approach
**Reuse the existing global/scene key-bindings pattern, don't invent a new merge mechanism.** Add
`global_gamepad_bindings: HashMap<String, String>` (`ProjectConfig`) + `scene_gamepad_bindings:
HashMap<String, String>` (`GameSceneV2`, override) — a new `ProjectGamepadBindings` (mirrors
`ProjectKeyBindings`) + `LoadedGamepadBindings` resource pair, merged into the loaded resource
exactly the way `project_loader.rs`/`scene_loader.rs` already merge `global_key_bindings`/
`scene_key_bindings` into `LoadedKeyBindings`: the scene layer **overlays per-key** on top of the
project base (`effective = project.clone(); for (k, v) in scene { effective.insert(k, v); }` — a
scene entry with the same button name replaces just that entry, every other project-level binding
still applies), not a whole-map replace. This needs all **three** existing merge sites, not one:
`project_loader.rs`'s two insert sites (inline-config path, external-files path) plus
`scene_loader.rs`'s per-scene-load rebuild — miss one and gamepad bindings bleed across scenes on
only one of the two loading paths. Button names parse via the existing `InputMap::
parse_gamepad_button` (`schema/player.rs`) — same vocabulary already used for per-player `InputMap`
fields, no new parser, no new button-name table to document (the existing "valid gamepad button
names" table just needs a note that it's no longer `InputMap`-only, see Docs below).

**Detection system** (sibling to `global_input_system` in `runtime/input.rs`, same `AppState::
InGame` gate, ordered `.before(message_interpreter_system)` for guaranteed same-frame consumption):
each frame, build the sorted-by-`Entity::index()` gamepad slice (same convention `resolve_gamepad`
already uses), exclude any pad already claimed by a live player's `InputMap.gamepad_index` **or** by
an `is_hot_join` entry still sitting undrained in `PendingEntitySpawns` this frame (mirrors the
`queued_hot_joins` same-frame double-join guard `Action::JoinPlayer`'s executor already has — a pad
mid-flight through the deferred spawn queue must not look "still unclaimed"). Among the remaining
eligible pads, find one whose bound button (per `LoadedGamepadBindings`) is `just_pressed` this
frame. **No separate "live signal" prefilter is needed**: a phantom/dead duplicate pad (the
documented Xbox 360 dual-registration quirk) reports zero for every button forever, so it can never
produce a `just_pressed` edge on anything — requiring the edge on the specifically-bound button
already excludes it, the same way `scene_key_bindings` already only triggers off key edges, not
passive presence. **At most one pad's press is serviced per frame** (deterministic: lowest sorted
index among qualifying presses) — if two eligible pads happen to press in the exact same frame,
the second one's press is simply not serviced that frame (an accepted, rare edge — the player just
presses again; `just_pressed` only lasts one frame, so this is a dropped edge, not a delay, matching
the `SPAWNS_PER_FRAME` precedent of "at most N of a thing per frame, nothing queued beyond that").
On a match, emit `UiEvent::ButtonPressed(trigger)` as usual, and additionally write a new
`PendingJoinGamepad(Option<Entity>)` resource. **The resource is unconditionally reset to `None` at
the top of every run of this system, before any new match is considered** — so it never carries a
value across frames, closing the staleness gap where a non-join gamepad binding (e.g. a pause
button) could otherwise leave a stale pad identity for a later, unrelated keyboard-triggered join to
inherit.

**Carrying gamepad identity to the join.** A bare `UiEvent::ButtonPressed(trigger)` (what keyboard
join uses) loses *which pad* pressed it — `Action::JoinPlayer` needs that identity to bind the new
player's `InputMap.gamepad_index` correctly. `Action::JoinPlayer`'s executor (unchanged
otherwise — same `join_prefab_keys[slot]` resolution, same cap/scope guards, same same-frame
double-join safety as v1) checks `PendingJoinGamepad` after resolving the joiner's `PlayerConfig`:
if set, overrides `gamepad_index` to that specific pad (translated to the same sorted-index
convention `resolve_gamepad` expects) instead of whatever the join prefab statically authored. A
keyboard-triggered join sees the resource already reset to `None` (per the frame-scoped reset above)
and behaves exactly as v1 does today — no `gamepad_index` override, zero behavior change to the
existing path.

**Accepted limitation, not solved here**: a runtime-assigned `gamepad_index` is still a *positional*
index into the sorted-by-connection-order slice, not a stable hardware identity — a mid-session
disconnect/reconnect of a different pad can shift indices and re-route which physical controller a
given index refers to. This hazard already exists for RON-authored `gamepad_index` today; making it
runtime-assigned doesn't introduce a new failure mode, just a new way to reach the same one. An
`Entity`-based (rather than positional-index-based) binding is logged as a future follow-up, not
attempted in this feature.

**Out of scope**: hot-*leave* (still its own deferred v2 item on `local_coop_hot_join_leave.md`,
unrelated to this), and any settings-UI style gamepad rebinding (that's the separate, still-Icebox
"Input remapping" backlog item).

## Tasks
- [ ] `global_gamepad_bindings` (`ProjectConfig`) + `scene_gamepad_bindings` (`GameSceneV2`) schema
      fields, `#[serde(default)]`
- [ ] `ProjectGamepadBindings` + `LoadedGamepadBindings` resources + merge logic at all three
      existing key-bindings merge sites (`project_loader.rs`'s inline-config insert, its
      external-files insert, and `scene_loader.rs`'s per-scene-load rebuild) — mirrors
      `ProjectKeyBindings`/`LoadedKeyBindings` exactly, per-key overlay semantics, not whole-map
      replace
- [ ] New detection system (`runtime/input.rs`, `AppState::InGame` gate, `.before(
      message_interpreter_system)`): claimed-pad exclusion (live players' `gamepad_index` **and**
      undrained `is_hot_join` `PendingEntitySpawns` entries), at-most-one-match-per-frame,
      `just_pressed`-only edge detection (no separate live-signal prefilter), frame-scoped
      unconditional `PendingJoinGamepad` reset, `UiEvent::ButtonPressed` emission on match
- [ ] `Action::JoinPlayer` executor: consume `PendingJoinGamepad` when present, override the
      joiner's `InputMap.gamepad_index` (index translated via the same sorted slice convention
      `resolve_gamepad` uses)
- [ ] `ironhold_cli validate`: warn on an unrecognized button name in `scene_gamepad_bindings`/
      `global_gamepad_bindings` (mirrors the existing `scene_key_bindings` unrecognized-key
      warning — check-only, since `parse_gamepad_button` already no-ops safely at load)
- [ ] Demo: `local_coop_demo/room8` gets `"South": "join"` in `scene_gamepad_bindings` alongside its
      existing `"KeyG": "join"` keyboard binding; **reword the on-screen join prompt** to name real
      controller buttons (e.g. "Press G, or A / Cross on a controller, to join") — never the
      engine's internal `"South"` string; **reword the P3/P4 control-hint labels** to note that a
      gamepad-driven join replaces that player's keyboard scheme entirely (including camera-look,
      which currently has no gamepad equivalent for a non-primary control scheme — verify exact
      behavior against `input_translator_system`/`camera_orbit_system` during implementation and
      word the hint accordingly)
- [ ] Tests: unclaimed-pad detection excludes a pad already claimed by a live player; excludes a pad
      already mid-flight via an undrained `is_hot_join` `PendingEntitySpawns` entry; a phantom/dead
      duplicate pad (all-zero every button) never triggers a join even when "present"; a
      gamepad-triggered join binds the correct `gamepad_index` to the new player; a
      keyboard-triggered join leaves `gamepad_index` untouched even when a gamepad-bound
      *non-join* trigger (e.g. pause) fired on a prior frame (regression for the staleness fix);
      two gamepads pressing join on **consecutive** frames each join distinct slots with distinct
      pads bound correctly; two gamepads pressing join in the exact **same** frame result in only
      one join that frame, deterministically the lowest-sorted-index pad
- [ ] Docs — `docs/20_data_formats.md`: `global_gamepad_bindings`/`scene_gamepad_bindings` rows
      (ProjectConfig and GameSceneV2 tables), a "gamepad hot-join" subsection alongside the existing
      keyboard one (including the `gamepad_index`-override interaction, mirroring the existing
      `PlayerIndex`-override bullet, and noting that a keyboard-triggered join never touches it),
      a note that the existing keyboard-binding collision warning does **not** apply to gamepad
      bindings (claimed-pad exclusion already makes e.g. `"South"` safe to bind despite being
      `gamepad_jump`'s default), and a note on the existing "valid gamepad button names" table that
      it now also governs `*_gamepad_bindings`, not just `InputMap` fields
- [ ] Docs — `crates/ironhold_core/src/CLAUDE.md`: extend the existing "Gamepad routing" section
      with the new bindings resource pair and the `PendingJoinGamepad` mechanism

## Open questions
- (resolved) Live-signal filter: no separate mechanism needed — `just_pressed` on the specifically
  bound button already excludes phantom/dead duplicate pads, since they never produce that edge.
- (resolved) Simultaneous-press pairing: cap detection to one match per frame rather than build
  multi-pad-per-frame plumbing; a true same-frame double press drops the second edge (documented,
  accepted — the player presses again).
- Real-hardware verification still needed: confirm in practice (per `gamepad_controller_input.md`'s
  precedent of finding real quirks only visible on actual hardware) that the very first press on a
  freshly connected, never-before-touched pad does register as a join with no priming press
  required — distinct from the known WASM/Chrome "first interaction may double as the pad's
  browser-level activation gesture" caveat, which is a browser limitation to document, not a bug to
  fix.

## Acceptance criteria
- Given a `Grid`-split scene with `scene_gamepad_bindings: {"South": "join"}`, when an unclaimed,
  connected gamepad's South button is pressed for the first time (no prior priming press, no hold),
  then a new player joins via `join_prefab_keys[slot]` and that specific gamepad immediately
  controls them (browser-observable with real hardware).
- Given a gamepad already driving an active player, when its South button is pressed, then nothing
  happens — a claimed pad is never eligible to trigger a join.
- Given a phantom/dead duplicate gamepad entry (the documented Xbox 360 dual-registration quirk),
  it can never trigger a join, since it never produces a `just_pressed` edge on any button.
- Given both keyboard and gamepad join bindings configured on the same scene, when either is
  pressed, joins behave identically except the gamepad path additionally binds `gamepad_index` to
  the pressing pad; a keyboard join never inherits a stale pad identity from an earlier,
  unrelated gamepad button press (e.g. pause) on a prior frame.
- Given the `local_coop_demo/room8` on-screen join prompt and P3/P4 control hints, a player reading
  them sees real controller button names and an accurate description of what changes (keyboard
  scheme fully replaced) when a gamepad joins that slot — never the engine's internal button-name
  string.
