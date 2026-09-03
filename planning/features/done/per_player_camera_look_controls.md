# Feature: Per-Player Keyboard Camera Look Controls

_Status: Done_
_Planned at: `3dc2451` (2026-07-19)_

**Shipped (2026-07-19):** implemented on `feature/camera-look-controls`. All 5 reviews
(alignment, system-architect, debug-detective, ux-gamedesigner-reviewer, wasm-perf-reviewer) came
back clean or with only non-blocking findings, all folded in: a stale doc-comment overclaiming
gamepad support (this feature doesn't touch gamepad — fixed), a test-coverage gap where the
RON-string-to-`OrbitCamera`-field spawn-site resolution was never exercised end-to-end (added
`test_scene_load_resolves_look_keys_and_look_speed_onto_spawned_split_orbit_camera`), and a
weaker-than-claimed independence test (strengthened to bind the sibling camera to a *different*
key rather than leaving it unbound). A latent `InputMap`-key-validation gap (no `ActionBar`-style
duplicate-key/cross-player collision check exists for `look_left`/etc.) was logged to
`claude_suggestions.md` as out-of-scope follow-up work, not fixed here. Full test suite green
(97 tests in `local_coop_tests.rs` alone), `cargo check -p ironhold_core -p ironhold_cli` clean,
WASM dev build succeeded. Playtest confirmed by Frank across the vertical-split, 4-way-grid,
horizontal-split, and dynamic-split `local_coop_demo` rooms plus a single-player regression check
— per-player camera look worked correctly with no cross-talk between viewports. Playtest also
surfaced one **unrelated, confirmed pre-existing** console warning (a double-despawn race in
`target_indicator_system`, nothing to do with this feature — this branch never touches
`target_indicator.rs`/`targeting.rs`) — logged to `planning/backlog.md` ▸ Bugs with repro steps
and a suggested fix, not addressed here to keep this feature's scope intact.

**Plan-review note (2026-07-19):** **system-architect** returned Ready with two minor corrections,
both folded in below: (1) the `PartyOrbitCamera` out-of-scope rationale was wrong (`party_camera_
follow_system` never fights a per-player write — it just has no single per-player owner to receive
one; corrected in Approach); (2) the Tasks list understated construction-site churn since neither
`CameraConfig` nor `InputMap` derives `Default` — `default_camera_config()`/`default_input_map()`
(`entity_spawner.rs:953/972`) and the test-fixture builders in `local_coop_tests.rs`/`action_tests.rs`/
`npc_tests.rs`/`entity_logic_tests.rs`/`scene_lifecycle_tests.rs` all need the new fields too (all
compile-time catches, not correctness risk, but worth listing). **ux-gamedesigner-reviewer**
returned Needs-more-design-work, resolved as follows: (a) P2's open question is resolved as option
(a) — extend `parse_key` with a coherent punctuation set, not just Comma/Period in isolation; (b)
docs tasks expanded to fix ~5 now-stale "split-screen camera is fully fixed, no manual control"
passages in `docs/20_data_formats.md` and to move the designer-facing worked example there (not
only the developer `CLAUDE.md`); (c) pitch discoverability closed via a commented-out `look_up`/
`look_down` pair in one demo prefab, per the project's existing commented-optional-field
convention; (d) the demo-wiring task is now enumerated exactly (10 named prefabs, confirmed against
`local_coop_demo`'s actual scheme assignments) instead of a vague pointer, plus a "why no look
keys" comment on the party-mode prefabs; (e) `look_speed`'s doc entry must state its unit
(rad/s) and a feel anchor, cross-referenced against `orbit_speed` so two "speed" fields on one
`camera:` block don't read as redundant.

**Amendment (2026-07-19, ahead of `gamepad_controller_input.md`'s plan review):** renamed
`keyboard_look_speed` → `look_speed` throughout (field, default fn, doc references) before any
code exists to change. Both this feature's own reviewers and the gamepad-input plan's reviewers
(which reuses this same dial for right-stick-Y analog pitch) independently flagged that a field
named "keyboard" governing a gamepad player's stick speed would read as a naming bug, not a
deliberate shared dial — cheaper to fix now than after either feature ships.

## What
Adds keyboard-bound camera yaw/pitch turning, per player, to `OrbitCamera` — so a player in a
split-screen scene can look around even though split-screen scenes deliberately disable mouse-
orbit (`orbit_button: "None"`, since one shared mouse can't drive 2+ independently-active
`OrbitCamera`s). New optional `InputMap` fields (`look_left`, `look_right`, `look_up`,
`look_down`) let each player's own control scheme bind its own keys, fully independent of every
other player's camera.

## Why
Today, once a scene goes split-screen, no player can rotate their own camera at all — yaw/pitch
are frozen at whatever `initial_yaw`/`initial_pitch` the camera spawned with, because the only
existing way to change them (`camera_orbit_system`'s mouse-orbit branch) is deliberately disabled
per split-screen camera. Character movement doesn't rotate the camera either (confirmed: `camera_
orbit_system` never reads `CharacterController`/`InputAction` state). This was surfaced 2026-07-11
during the split-screen particle-billboard-orientation playtest, when manually orbiting the camera
turned out to be impossible in a 2-viewport scene — and it's the single largest usability gap
between single-player and local co-op today (see the parity review this plan follows from).

## Approach

**New `InputMap` fields** (`schema/player.rs`), all `Option<String>` defaulting to `None` (unbound
— zero behavior change for every existing scene, no RON migration):
```rust
#[serde(default)] pub look_left: Option<String>,
#[serde(default)] pub look_right: Option<String>,
#[serde(default)] pub look_up: Option<String>,
#[serde(default)] pub look_down: Option<String>,
```

**Extend `InputMap::parse_key`** with a coherent punctuation set — not just the two keys the arrow
scheme needs, so the "valid key names" surface reads as a deliberate, complete set rather than an
arbitrary pair added for one demo: `Comma`, `Period`, `Semicolon`, `Quote`, `Slash`,
`BracketLeft`, `BracketRight`, `Minus`, `Equal` (all real `bevy::input::keyboard::KeyCode`
variants — a mechanical, purely-additive widening of the match, same pattern as the existing
letter/digit/Numpad/F-key/Arrow/modifier arms).

**New `CameraConfig` field**: `look_speed: f32` (default `2.0`, i.e. ~2 rad/s — a full yaw
revolution in ~3.1s held). **Deliberately not reusing the existing `orbit_speed` field**: `orbit_
speed` is tuned as a mouse-pixel-delta multiplier (`mouse_delta.x * orbit_speed * dt`); the existing
split-screen demo prefabs already set it to `0.4` for that purpose. Reusing `0.4` directly as a
keyboard rad/s rate would make keyboard-look sluggish (~23°/s, ~15.7s per revolution) — the two
inputs have different natural units and shouldn't share a dial.

**`OrbitCamera` gains** (parsed once at spawn time, mirroring how `orbit_lmb`/`orbit_rmb`/
`character_rotate_rmb`/`character_rotate_lmb` are already pre-resolved from RON strings into bools
at spawn rather than re-parsed every frame — same pattern, applied to the 4 new optional keys):
```rust
pub look_left_key: Option<KeyCode>,
pub look_right_key: Option<KeyCode>,
pub look_up_key: Option<KeyCode>,
pub look_down_key: Option<KeyCode>,
pub look_speed: f32,
```
Populated in `scene_loader.rs`/`entity_spawner.rs` alongside the existing `parse_orbit_button` call
site.

**`camera_orbit_system` gains one new param**, `Res<ButtonInput<KeyCode>>` — no query join to
`CharacterController`/`InputMap` needed, since the keys are already resolved onto `OrbitCamera`
itself. After the existing mouse `orbit_active` block (untouched), a new, unconditional block:
```rust
if let Some(key) = orbit.look_left_key  { if keyboard_input.pressed(key) { orbit.yaw += orbit.look_speed * dt; } }
if let Some(key) = orbit.look_right_key { if keyboard_input.pressed(key) { orbit.yaw -= orbit.look_speed * dt; } }
if let Some(key) = orbit.look_up_key    { if keyboard_input.pressed(key) { orbit.pitch = (orbit.pitch + orbit.look_speed * dt).min(orbit.max_pitch); } }
if let Some(key) = orbit.look_down_key  { if keyboard_input.pressed(key) { orbit.pitch = (orbit.pitch - orbit.look_speed * dt).max(orbit.min_pitch); } }
```
This runs additively and independently of the mouse-orbit gate — a scene could in principle bind
both mouse and keyboard look for the same camera (harmless; last-writer-per-frame wins, same as any
two inputs touching the same field). Per-camera cost is 4 `Option<KeyCode>` comparisons + at most 4
`ButtonInput::pressed` lookups (already O(1)); negligible even at 4-way split.

**Pitch direction convention (pinned explicitly, per system-architect's finding):** `max_pitch`
(default `0.9`) is the more-overhead camera angle, `min_pitch` (default `0.1`) is near-horizontal —
confirmed against the existing mouse convention (`pitch -= mouse_delta.y * orbit_speed * dt`, i.e.
moving the mouse up *decreases* `pitch`, tilting the camera down toward horizontal — the opposite
of `look_up`'s naive reading). To keep `look_up`/`look_down` matching *this scene's actual mouse
feel* rather than a literal "up = sky" reading: `look_up` increases `pitch` toward `max_pitch`
(more overhead), `look_down` decreases it toward `min_pitch` (more horizontal) — i.e. `look_up`
mirrors "mouse down" in this codebase's pitch convention, not "mouse up." This must be verified by
feel in playtest before any project binds pitch keys by default; the regression test asserts the
direction explicitly, not just the clamp bounds, so a future refactor can't silently flip it.

**`PartyOrbitCamera` is explicitly out of scope** — not because per-player input would "fight" its
per-frame framing logic (it wouldn't; `party_camera_follow_system` only ever *reads* yaw/pitch to
derive translation, it doesn't overwrite them, which is exactly why mouse-orbit already works on a
party camera without conflict), but because a party camera is shared by every player at once and
has no single per-player owner to attribute a `look_left`/`look_right` binding to. Per-player look
only makes sense for the real per-player `OrbitCamera`s used in `split`/`dynamic` split-screen —
which is also the actual gap being closed here.

**Demo wiring** — bind yaw keys (`look_left`/`look_right`) into the 10 split/dynamic/grid player
prefabs in `assets/projects/local_coop_demo/prefabs/prefabs.ron` (verified against the file's
actual scheme assignments — every `_p1_*` prefab uses WASD, every `_p2_*` prefab uses Arrows,
`player_p3_grid` uses IJKL, `player_p4_grid` uses Numpad):

| Prefab(s) | Scheme | look_left / look_right |
|---|---|---|
| `player_p1_split`, `player_p1_split_h`, `player_p1_dynamic`, `player_p1_grid` | WASD | `KeyZ` / `KeyX` |
| `player_p2_split`, `player_p2_split_h`, `player_p2_dynamic`, `player_p2_grid` | Arrows | `Comma` / `Period` |
| `player_p3_grid` | IJKL | `KeyH` / `KeyP` |
| `player_p4_grid` | Numpad | `Numpad7` / `Numpad9` |

All four look-key pairs are mutually disjoint from each other and from every existing movement/
action key across all 4 schemes simultaneously (verified — this matters most in the 4-way grid
room where all four schemes are live on one keyboard at once).

The party-mode prefabs (`player_p1`, `player_p2`, the shared-camera, non-split path) get **no**
look keys and a one-line RON comment explaining why (`PartyOrbitCamera` is shared across all
players — no single per-player owner for a look binding — see Approach), matching this codebase's
existing convention of a short "IMPORTANT" comment wherever a field is deliberately absent rather
than merely forgotten (e.g. the existing "party on a non-first player is silently ignored" comments
in the same file).

**Pitch discoverability**: pitch (`look_up`/`look_down`) is implemented in the system (symmetric
cost, no reason to withhold it) but not *bound* in any of the 10 demo prefabs above, to keep the
new-key count at 2 per scheme rather than 4. To avoid the present-but-invisible-field trap this
codebase has hit before (`ActionSlotDef.label`, `animation_sources`, depth-scale overrides), add a
commented-out `look_up`/`look_down` pair with a one-line note to exactly one demo prefab
(`player_p1_split`), mirroring the existing commented-optional-field convention already used in
this file (e.g. `// gamepad_index: 0,`). The docs RON example shows all four fields regardless of
which are demo-bound.

## Tasks
- [x] `InputMap`: add `look_left`/`look_right`/`look_up`/`look_down: Option<String>` (schema/player.rs)
- [x] `InputMap::parse_key`: add `Comma`, `Period`, `Semicolon`, `Quote`, `Slash`, `BracketLeft`,
      `BracketRight`, `Minus`, `Equal` arms
- [x] `CameraConfig`: add `look_speed: f32` with `default_look_speed() -> f32 { 2.0 }`
- [x] `OrbitCamera`: add `look_left_key`/`look_right_key`/`look_up_key`/`look_down_key: Option<KeyCode>`
      and `look_speed: f32`; populate at both spawn sites (`scene_loader.rs`,
      `entity_spawner.rs`) alongside the existing `parse_orbit_button` call
- [x] `camera_orbit_system`: add `Res<ButtonInput<KeyCode>>` param and the keyboard-look block
      (independent of the existing mouse `orbit_active` gate); pin the pitch direction convention
      described above
- [x] Update every `CameraConfig`/`InputMap` struct literal that doesn't go through RON deserialization
      (neither type derives `Default`): `default_camera_config()`/`default_input_map()`
      (`entity_spawner.rs:953`/`972`) and the test-fixture builders in `local_coop_tests.rs`,
      `action_tests.rs`, `npc_tests.rs`, `entity_logic_tests.rs`, `scene_lifecycle_tests.rs`
- [x] Wire the 10-prefab demo table above into `local_coop_demo/prefabs/prefabs.ron`; add the
      commented-out pitch pair to `player_p1_split`; add the "why no look keys" comment to
      `player_p1`/`player_p2`
- [x] Tests — regression: an unmodified pre-existing scene (`3rd_person_game_demo` or `quick_scene`)
      has identical mouse-orbit yaw/pitch behavior after this change (no `look_*` fields authored →
      `Option<KeyCode>` fields are `None` → the new block never fires); new: a player with
      `look_left`/`look_right` bound rotates *only* its own camera's yaw when its key is held across
      several `update()` ticks, while a second camera in the same world (different `OrbitCamera`
      entity, no keys bound) is unaffected; pitch test asserts *direction* (`look_up` increases
      `pitch` toward `max_pitch`) in addition to clamping to `min_pitch`/`max_pitch` under sustained
      hold
- [x] Docs — `docs/20_data_formats.md`: `InputMap` and `CameraConfig` tables get the new fields
      (the `look_speed` entry states its unit, rad/s, a feel anchor — "~3s per full turn
      at the default" — and cross-references `orbit_speed` as the separate mouse-sensitivity dial);
      the "Valid key name strings" table gets the 9 new punctuation entries; one RON example shows
      all four `look_*` fields even though the shipped demo only binds yaw
- [x] Docs — fix the ~5 now-stale "split-screen camera is fully fixed / no manual control" passages
      in `docs/20_data_formats.md` (the `orbit_button: "None"` note and the split/dynamic/grid
      section blockquotes) to read "no manual *mouse* control; keyboard look can still be bound per
      player" — and add the designer-facing worked scheme-table example there (not only in the
      developer-facing `CLAUDE.md`)
- [x] Docs — `crates/ironhold_core/src/CLAUDE.md`'s Local co-op section: note that split-screen's
      mouse-orbit-disabled limitation is now addressed via per-player keyboard look, cross-referencing
      the docs/20 worked example rather than duplicating the full table

## Open questions
None outstanding — the arrow-scheme key question and all other reviewer findings are resolved above.

## Acceptance criteria
- Given `local_coop_demo`'s split-screen room, when P1 holds their bound look-left/look-right key,
  then P1's own viewport camera visibly orbits around P1's character while P2's viewport is
  completely unaffected (**browser-observable**).
- Given the same scene, when P2 holds P2's own look key instead, then only P2's camera orbits and
  P1's is unaffected.
- Given the 4-way grid room, when any one of P1–P4 holds their own look key, only that player's
  camera orbits — no cross-scheme key collision (**browser-observable**).
- Given any pre-existing non-split-screen scene with no `look_*` fields authored (e.g.
  `3rd_person_game_demo`, `quick_scene`), when this ships, then mouse-orbit camera feel is
  unchanged — regression, not just a passing test.
- Given a player's `look_up`/`look_down` keys held continuously, then pitch moves in the direction
  pinned above and never exceeds `min_pitch`/`max_pitch` (no clipping through the ground or
  flipping past the character).
