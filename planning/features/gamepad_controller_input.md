# Feature: General Gamepad/Controller Input

_Status: Ready_
_Planned at: `77a6dc9` (2026-07-19)_

**Plan-review note (2026-07-19):** **system-architect** returned Needs-more-design-work, resolved
as follows: (1) **Major** — `interactable_system` uses `player_query.single()` (`interactable.rs:
36`), which fails and early-returns for *everyone* once 2+ `CharacterController`s exist in a scene;
adding a gamepad check "alongside" the existing keyboard read would not unblock local-coop interact
as the plan's Why originally implied, since the system never reaches the per-entity loop in
co-op at all. Resolved by explicitly scoping this feature's gamepad-interact to single-player
scenes (matching `interactable_system`'s existing, pre-existing limitation) and filing the
underlying local-coop interact bug separately (see `planning/backlog.md` ▸ Bugs — not this
feature's scope to fix, since it's a pre-existing bug independent of gamepad input entirely). (2)
The `resolve_gamepad` helper must preserve "sort connected gamepads once per system, resolve many
players/cameras against that one sorted slice" rather than re-sorting per caller — signature
corrected below. (3) Consistency fix: `gamepad_index`/`gamepad_deadzone` are now pre-resolved onto
`OrbitCamera` at spawn (mirroring the sibling camera-look plan's spawn-time-resolution pattern),
dropping the need for a live `Query<&CharacterController>` join in `camera_orbit_system` — only a
live `Query<(Entity, &Gamepad)>` remains, since the analog stick *value* genuinely can't be
resolved at spawn. (4) The `- stick_y` sign in the pitch formula is now flagged as unverified,
not assumed — must be checked against Bevy's actual axis convention and pinned by the same
direction-asserting test pattern the sibling plan already established, not presupposed. (5) Added:
an explicit hard ordering dependency on `per_player_camera_look_controls.md` merging first (shares
`OrbitCamera.pitch`/`min_pitch`/`max_pitch`/`look_speed`, its pitch-direction convention, and both
touch the same `InputMap`/`CameraConfig` struct-literal call sites); a load-time `warn!`+no-op task
for unrecognized `gamepad_*` button names, mirroring the existing key-name validation seam
(`project_loader.rs`/`scene_loader.rs`); a `cargo check -p ironhold_cli` task.
**ux-gamedesigner-reviewer** returned Needs-more-design-work, resolved as: (a) West/North defaults
are reframed as genre-conventional (Xbox X/Y, PlayStation Square/Triangle), not merely
non-colliding — docs must state the physical mapping, not just the abstract Bevy name; (b) demo
wiring was entirely missing — added a commented-out worked example task, scoped to a
single-player demo (consistent with the interactable-scope decision above); (c) the frozen-yaw
limitation note is reframed as an honest, permanent (until resolved) keyboard/gamepad parity gap,
not "the same limitation keyboard had before" (keyboard is getting fixed by the sibling plan;
gamepad is not, yet); (d) two stale `docs/20_data_formats.md` passages identified for correction;
(e) `look_speed`'s cross-input reuse is now documented explicitly rather than left implicit (see
the sibling plan's own amendment renaming it from `keyboard_look_speed`).

## What
Makes gamepad input designer-configurable and closes two real functional gaps: today a connected
gamepad can move/turn/jump/run (all **hardcoded** Rust constants, not RON-authorable), but has *no*
way to interact with the world or cycle targets, and no camera-pitch control at all. This feature:
(1) moves the existing hardcoded button/axis mapping into RON-configurable `InputMap` fields with
defaults that exactly match today's behavior, (2) adds a gamepad equivalent for `interact`
(single-player scenes only — see Approach) and `target_next` (works in local co-op; `tab_targeting_
system` is already per-player), and (3) adds right-stick-Y camera pitch, reusing the `OrbitCamera.
pitch`/`min_pitch`/`max_pitch` fields and the `look_speed` dial from `per_player_camera_look_
controls.md` (**hard dependency — must merge after that plan ships**, see Tasks).

## Why
Prior investigation (this session) confirmed gamepad support today is a narrow, single-purpose
convenience for local co-op pad *routing* (`InputMap.gamepad_index`), not general controller input:
left stick → move, right stick X → turn, South → jump, East → run are all hardcoded literals in
`input_translator_system` (`runtime/input.rs:97-143`), with **no** RON field, **no**
`parse_gamepad_button` helper, and **no** designer override path. Two consequences beyond "not
designer-configurable": a controller-only player in a **single-player** scene cannot `interact`
with anything at all (no gamepad path exists, and `interactable_system`'s existing `single()` call
means this feature's fix is correctly scoped to single-player only — see Approach), and no gamepad
player anywhere can Tab-cycle targets (`tab_targeting_system` is already per-player, so this half
of the fix *does* work in local co-op). This also establishes the button-name-parsing
infrastructure the dependent "gamepad-routed action-bar slots" backlog item will reuse.

## Approach

**New `InputMap` fields** (`schema/player.rs`), plain (non-`Option`) `String`/`f32` with
`#[serde(default = ...)]` matching today's hardcoded values exactly — same idiom as the existing
`run`/`interact`/`target_next` keyboard defaults, so every scene that doesn't author these gets
byte-for-byte identical gamepad behavior to today:
```rust
#[serde(default = "default_gamepad_jump")]        pub gamepad_jump: String,          // "South"
#[serde(default = "default_gamepad_run")]         pub gamepad_run: String,           // "East"
#[serde(default = "default_gamepad_interact")]    pub gamepad_interact: String,      // "West" (new)
#[serde(default = "default_gamepad_target_next")] pub gamepad_target_next: String,   // "North" (new)
#[serde(default = "default_gamepad_deadzone")]    pub gamepad_deadzone: f32,         // 0.15 (matches today's constant)
```
`West`/`North` are not arbitrary — they follow genre convention (Xbox X/Y, PlayStation
Square/Triangle: West = the standard "use/interact/action" face button, North = the standard
"cycle/secondary" button), not merely "whatever doesn't collide with South/East." Docs must state
the physical mapping for every button name, not just the abstract Bevy name (see Tasks) — the
existing `gamepad_index` doc prose already does this correctly and is the pattern to match.

**`InputMap::parse_gamepad_button(s: &str) -> Option<GamepadButton>`** — new helper mirroring
`parse_key`, covering `South`/`East`/`North`/`West`/`LeftTrigger`/`LeftTrigger2`/`RightTrigger`/
`RightTrigger2`/`Select`/`Start`/`LeftThumb`/`RightThumb`/`DPadUp`/`DPadDown`/`DPadLeft`/
`DPadRight`. An unrecognized name `warn!`s and no-ops at load time, mirroring the existing key-name
validation seam (`project_loader.rs`/`scene_loader.rs` already warn on bad keyboard key names) —
without this, a typo'd gamepad button name would silently and permanently disable that action with
no diagnostic, unlike a typo'd keyboard key.

**Shared gamepad-resolution helper** — `input_translator_system`'s existing "collect connected
gamepads, sort by `Entity::index()`, index by `gamepad_index`" logic (`input.rs:52-61`) is
refactored into `fn resolve_gamepad<'a>(sorted: &'a [(Entity, &'a Gamepad)], index: Option<usize>)
-> Option<&'a Gamepad>`, taking an **already-sorted slice** built once per system per frame (not
re-sorted per caller) — `input_translator_system` keeps its existing single sort-before-the-loop
shape, and `camera_orbit_system` (below) builds its own single sorted `Vec` once per frame and
calls the shared resolver per camera. `pub(crate)` in `runtime/input.rs`, reachable from
`capabilities/camera.rs`/`capabilities/interactable.rs`/`capabilities/targeting.rs`.

**`input_translator_system` changes**: replace the hardcoded `GamepadButton`/`GAMEPAD_DEADZONE`
literals with per-player lookups from the resolved player's `InputMap` (`gamepad_jump`/
`gamepad_run`/`gamepad_deadzone`, parsed via the new helper). Movement (left stick) and turn (right
stick X) are unchanged in mechanism, just no longer hardcoded to fixed button names for jump/run.

**`targeting.rs` (`tab_targeting_system`)** — already fully per-player (loops `&mut controllers`,
each cycling its own `PlayerTarget` off its own `InputMap.target_next`); this feature folds a
gamepad check into the same per-player boolean the keyboard check already produces (matching
`input_translator_system`'s existing `keyboard || gamepad` combining pattern for jump, not a
second, separately-gated block) — this half of the fix works correctly in local co-op today.

**`interactable.rs` (`interactable_system`) — scoped to single-player.** This system calls
`player_query.single()` and early-returns for *all* players the moment a scene has 2+
`CharacterController`s — a pre-existing limitation, confirmed independent of this feature (interact
already doesn't work for anyone, keyboard or gamepad, in any local-coop scene today). This feature
adds a gamepad check folded into the existing keyboard boolean (same combining shape as above), so
gamepad-interact works in every scene interact already works in today — i.e. single-player only.
Rewriting `interactable_system` into a per-player loop (mirroring `tab_targeting_system`'s shape) is
a real, separate fix, logged as its own backlog Bug rather than bundled into this feature's scope.

**Camera pitch via right-stick-Y** — mirrors `per_player_camera_look_controls.md`'s `look_up`/
`look_down` mechanism, reusing the *same* `OrbitCamera.pitch`/`min_pitch`/`max_pitch` fields and its
pinned pitch-direction convention. Right-stick-Y is currently **unused** by any system (only
right-stick-X drives `InputAction::Turn`) — a net-new axis, no conflict with existing gamepad
character-turning. `OrbitCamera` gains `gamepad_index: Option<usize>` and `gamepad_deadzone: f32`,
**pre-resolved at spawn** from the player's `InputMap` (consistent with the sibling plan's
spawn-time-resolution pattern — only the live per-frame stick *value* needs a live query, not the
index/deadzone, which are static per player). `camera_orbit_system` gains one live
`Query<(Entity, &Gamepad)>` (no `CharacterController` join needed), builds one sorted slice per
frame, resolves `orbit.gamepad_index` against it via the shared helper, and — past
`orbit.gamepad_deadzone` — applies pitch each frame:
```rust
orbit.pitch = (orbit.pitch + stick_y_sign * stick_y * orbit.look_speed * dt).clamp(orbit.min_pitch, orbit.max_pitch);
```
**The `stick_y_sign` factor is deliberately left unresolved here — do not presuppose `+1` or `-1`.**
Bevy's `GamepadAxis::RightStickY` sign convention must be verified empirically (or from Bevy's own
docs/source) against the sibling plan's pinned convention (`look_up` increases `pitch` toward
`max_pitch`) before writing the final expression; the same direction-asserting regression test
pattern that plan established must be reused here, not just a clamp-bounds test. `look_speed`
(added by the sibling plan, reused unmodified here) already scales correctly for analog magnitude:
multiplying by the raw `stick_y` value (not just a boolean past-deadzone gate) means a full stick
deflection reproduces exactly the keyboard-hold rate, and partial deflection scales proportionally
— no third speed field needed. Docs must state plainly that `look_speed` governs both keyboard-hold
and gamepad-stick pitch rate, so a gamepad-only player's `camera:` block referencing a `look_speed`
field isn't mistaken for a keyboard-only leftover.

**Explicitly deferred — right-stick-X camera yaw (permanent parity gap, not "the same as before").**
Right-stick-X already drives `InputAction::Turn` (character body rotation); reusing it to *also*
orbit the camera (full twin-stick free-look, character auto-facing movement instead) is a deeper
control-feel redesign, not a config addition — deliberately not attempted here. Stated plainly: once
this feature and the sibling camera-look plan both ship, a **keyboard** split-screen player can
freely yaw their camera (`look_left`/`look_right`), but a **gamepad** split-screen player cannot —
pitch-only. This is a real, standing keyboard/gamepad parity gap, not a temporary one, and must be
documented as such rather than glossed over. Still a strict improvement over today (zero camera
control), so shippable as-is; revisit only with a deliberate playtest-informed decision on
twin-stick vs. auto-face-on-move, not as a quiet follow-up.

**Explicitly out of scope — action bar.** Gamepad-routed action-bar slots is a separate, dependent
backlog item that reuses this feature's `parse_gamepad_button` helper and RON-field idiom.

## Tasks
- [ ] **Hard dependency**: do not start implementation before `per_player_camera_look_controls.md`
      merges to `integration` — this feature reuses its `OrbitCamera.look_speed`/`pitch`/
      `min_pitch`/`max_pitch` fields, its pinned pitch-direction test pattern, and both features
      touch the same `InputMap`/`CameraConfig` non-derived struct literals (whichever lands second
      rebases onto the first's field additions)
- [ ] `InputMap`: add `gamepad_jump`/`gamepad_run`/`gamepad_interact`/`gamepad_target_next: String`
      (defaults matching today's hardcoded values) and `gamepad_deadzone: f32` (default `0.15`)
- [ ] `InputMap::parse_gamepad_button` helper (schema/player.rs), mirroring `parse_key`; unrecognized
      names `warn!` + no-op at load time (new task, mirrors the existing key-name validation seam)
- [ ] Refactor `input_translator_system`'s gamepad sort-and-index logic into a shared
      `resolve_gamepad(sorted_slice, index)` helper — preserve "sort once per system per frame,
      resolve many" in both callers (see corrected signature in Approach)
- [ ] `input_translator_system`: replace hardcoded gamepad button/deadzone literals with per-player
      RON-resolved values
- [ ] `targeting.rs`: fold a gamepad-target-next check into `tab_targeting_system`'s existing
      per-player boolean (works in local co-op — this system is already per-player)
- [ ] `interactable.rs`: fold a gamepad-interact check into `interactable_system`'s existing
      keyboard boolean — **explicitly scoped to single-player scenes**, matching this system's
      pre-existing `single()`-based limitation; do not expand this system's scope in this feature
- [ ] File a new `planning/backlog.md` ▸ Bugs entry: `interactable_system` never fires for any
      player once 2+ `CharacterController`s exist (`single()` early-return) — pre-existing,
      independent of gamepad input, found during this plan's review
- [ ] `OrbitCamera`: add `gamepad_index: Option<usize>` and `gamepad_deadzone: f32`, pre-resolved
      at spawn from the player's `InputMap` (both spawn sites, alongside the sibling plan's
      `look_left_key`/etc. resolution)
- [ ] `camera_orbit_system`: add `Query<(Entity, &Gamepad)>`, resolve once per frame via the shared
      helper, apply right-stick-Y pitch — **verify the stick-Y sign empirically against the pinned
      pitch convention before committing to `stick_y_sign`'s value**; do not assume
- [ ] Update every non-deserialized `InputMap`/`OrbitCamera` struct literal (test fixtures,
      `default_input_map()` in `entity_spawner.rs`) for the new fields — same construction-site
      churn class as the sibling plan
- [ ] Demo wiring: add a commented-out worked `gamepad_*` binding block (mirroring the sibling
      plan's commented-out pitch pair and the existing `// gamepad_index: 0,` convention) to a
      **single-player** demo prefab with a nearby interactable (e.g. `entity_logic_demo` or
      `quick_scene` — confirm which has an `interactable: true` prop in reach of player spawn before
      picking one)
- [ ] Tests — regression: existing gamepad-routed fixtures behave identically with the new fields
      at their defaults; new: a custom `gamepad_interact` button triggers `entity.interacted:{id}`
      in a single-player scene; new: a custom `gamepad_target_next` button advances Tab-targeting in
      a **2-player** scene (proving the per-player path actually works in co-op, unlike interact);
      new: right-stick-Y moves camera pitch in the same direction as keyboard `look_up`/`look_down`
      (direction-asserting, not just clamp-asserting, per the sibling plan's pattern); new:
      `parse_gamepad_button` unit tests for the full supported set plus an unrecognized-name
      `warn!`+no-op case
- [ ] `cargo check -p ironhold_cli` — standard schema-change gate
- [ ] Docs — `docs/20_data_formats.md`: `InputMap` table gets the 5 new fields; a "valid gamepad
      button names" table with the **Xbox/PlayStation physical mapping** for every entry (not just
      the abstract Bevy name — matching the existing `gamepad_index` prose's own precedent); a full
      worked `inputs:` RON example showing all 5 gamepad fields; note that `look_speed` governs both
      keyboard-hold and gamepad-stick pitch rate
- [ ] Docs — fix two stale passages in `docs/20_data_formats.md`: the `gamepad_index` entry's
      button-mapping description (currently stated as fact — must read as "defaults, overridable via
      the `gamepad_*` fields below") and the co-op targeting prose (~line 498-500) that already
      claims Tab-cycle "can use a gamepad button" — verify this becomes true rather than staying a
      coincidentally-accurate stray claim
- [ ] Docs — explicitly document the permanent keyboard/gamepad camera-yaw parity gap (see Approach)
      so it reads as a known, deliberate limitation, not an oversight
- [ ] Docs — `crates/ironhold_core/src/CLAUDE.md`'s "Local co-op ... gamepad routing" section:
      update from "hardcoded" to "RON-configurable, defaults shown," cross-referencing docs/20

## Open questions
- Twin-stick camera yaw for gamepad (right-stick-X repurposed or dual-purposed) — deliberately
  deferred, see Approach. Needs a dedicated playtest/decision once both this feature and the
  camera-look plan are live in a real gamepad-in-split-screen scene.
- Should `gamepad_target_next` default to a shoulder/trigger/R3 button instead of a face button,
  matching how some action games home target-cycle away from the diamond? North (Y/Triangle) is
  defensible as a shipped default (reviewed and accepted), but the docs worked example should show
  how to rebind it to `RightThumb`/`RightTrigger` for designers who expect that convention.

## Acceptance criteria
- Given a scene with a gamepad-routed player and no gamepad-related `InputMap` fields authored
  beyond `gamepad_index`, when this ships, then movement/turn/jump/run feel identical to before
  (regression, not just passing tests).
- Given a **single-player** scene with a gamepad-routed player prefab with a custom
  `gamepad_interact` button, when that button is pressed near an interactable entity, then
  `entity.interacted:{id}` fires exactly as it would from the keyboard `interact` key
  (**browser-observable**).
- Given a **2-player local-coop** scene where one player is gamepad-routed, when that player's
  `gamepad_target_next` button is pressed, then Tab-cycle targeting advances for that player only,
  exactly as the keyboard binding does for the other player (**browser-observable** — proves the
  per-player targeting path works in co-op, in contrast to the single-player-only interact fix).
- Given a gamepad-routed player holding right-stick-Y past the deadzone, then camera pitch moves
  smoothly within `min_pitch`/`max_pitch`, in the verified (not assumed) direction matching keyboard
  `look_up`/`look_down` (**browser-observable**).
