# Feature: Viewport-aware `world_label_screen_pos_system` (fix split-screen `WorldLabel` positioning)

_Status: Ready_
_Architecture review: PASS (2026-07-08) — sound camera-selection algorithm, acceptable scope
boundary (see two amendments below), no crate/WASM concerns._
_Planned at: `9b94255` (2026-07-08)_

## What

Fix `world_label_screen_pos_system` (`crates/ironhold_core/src/lib.rs:497`) so every `WorldLabel`
entity — portal room-name labels, entity labels, floating stat labels, damage popups, nameplate
anchors — positions correctly in scenes with 2+ active `Camera3d` entities, instead of silently
no-oping every frame via `camera_q.single()`. This is the real-world trigger reported as: **"Portal
room-name labels render static and mis-positioned in every split-screen room"**
(`planning/backlog.md` ▸ Bugs).

## Why

Confirmed bug, reproduced in `local_coop_demo` rooms 3-6 (screenshots 2026-07-08). Root cause:
`world_label_screen_pos_system` calls `camera_q.single()` against `Query<(&Camera, &GlobalTransform),
With<Camera3d>>`. Every split-screen scene has 2+ real `Camera3d` entities alive simultaneously
(room5's dynamic split always has 3 — two `SplitViewportSlot` cameras plus the `PartyOrbitCamera`,
regardless of which are currently active via `Camera.is_active`; room6's grid split has 4). `.single()`
returns `Err` whenever more than one entity matches the query, regardless of `is_active` — so the
`let Ok(...) = camera_q.single() else { return }` guard fires every frame, and every `WorldLabel`'s
`Transform` is frozen at its default spawn value forever.

This was predicted in `planning/claude_suggestions.md` ▸ Camera (2026-07-05, before any split-screen
code existed) as one of four systems with an undocumented single-camera assumption
(`world_label_screen_pos_system`, `nameplate.rs:212`, `particle_renderer.rs:303`, `targeting.rs:122`).
Per Frank's direction (2026-07-08): fix this system properly rather than reverting the room-name
labels feature.

## Research findings

- **`WorldLabel` is shared infrastructure, not portal-specific.** One system
  (`world_label_screen_pos_system`) drives every consumer: `world_labels:` scene RON (portal
  room-name labels — the reported bug), per-entity `label:` RON, `stat_label`/`world_stat_bar`
  floating text, `ShowDamagePopup` popups, and every nameplate anchor
  (`nameplate.rs:107`). Fixing the system fixes all of these at once — nameplates in split-screen
  are a second, previously-undiscovered instance of the same bug (nameplates aren't enabled in
  `local_coop_demo` today, so it's latent there, but the fix removes the limitation for any future
  project that combines the two).
- **`Camera::world_to_viewport` already returns full-window logical coordinates, not
  viewport-relative ones.** Confirmed by reading `bevy_camera-0.18.0/src/camera.rs:505-533`:
  `world_to_viewport_core` maps NDC into `self.logical_viewport_rect()`, whose `min` is the
  viewport's window-relative offset — so for a camera with a sub-viewport, the returned `Vec2` is
  already positioned correctly in the full window's logical pixel space. The existing
  `vp.x - half_w` / `half_h - vp.y` math (window-relative → `Text2d`'s origin-centered 2-D space)
  needs **no change** once a per-camera result is obtained — only the camera **selection** is
  broken, not the projection math.
- **`Camera::logical_viewport_rect()`** returns `Some(Rect)` for the camera's own on-screen
  section (or the full window rect when no `Viewport` is set — i.e. single-camera scenes are
  handled by the same code path with zero special-casing). This is the natural "does this camera
  actually show this point on-screen" test: project via `world_to_viewport`, then check the result
  falls inside `logical_viewport_rect()`.
- **`Camera.is_active` already distinguishes live cameras.** `dynamic_split_screen_system`
  (`camera.rs:523-527`) toggles `is_active` on the split cameras vs. the party camera — inactive
  cameras are never rendered but keep updating their `Transform` (per `CLAUDE.md`'s note that
  `camera_orbit_system`/`party_camera_follow_system` don't gate on `is_active`). Filtering the query
  on `camera.is_active` collapses the merged case back to exactly one active camera — no ambiguity,
  same as today's single-player scenes.
- **A single `WorldLabel` is one `Text2d` entity with one `Transform`, rendered once per frame by
  the one persistent global `Camera2d`.** It cannot visually appear in two different screen
  positions in the same frame. This differs from the split-screen player HUD corner labels
  (`planning/features/done/local_coop_player_hud_labels.md`), which solved the analogous problem by
  spawning a **separate UI entity per `SplitViewportSlot` camera** (`Added<SplitViewportSlot>` →
  one label per camera). Duplicating every `WorldLabel` consumer (5 spawn call sites across
  `scene_loader.rs`, `nameplate.rs`, `damage_popup.rs`, `action_executor.rs`) the same way would be
  a much larger, cross-cutting rework — out of scope for fixing the reported bug (see "Not in
  scope" below).
- **No inspector interference.** `inspector.rs`'s debug camera is a `Camera2d` (`RenderLayers::layer(31)`),
  not a `Camera3d` — confirmed it's never a candidate in this query.
- Distance-based font scaling (`WorldLabel.depth_scale`) currently uses one global `cam_pos` for
  every label. Once camera selection is per-label, `cam_pos` naturally becomes "the distance from
  whichever camera is actually showing this label" — a correctness improvement, not just a bug fix,
  since a label seen from a closer split camera should scale differently than the same label seen
  from a farther one.

## Approach

Rewrite `world_label_screen_pos_system`'s camera handling:

1. Change the camera query to `Query<(&Camera, &GlobalTransform, Option<&SplitViewportSlot>), With<Camera3d>>`
   (no `.single()`), filtered to `camera.is_active` inside the loop (cheap — at most 4 iterations).
2. For each `WorldLabel`, resolve its `world_pos` (existing tracked-entity/fixed logic, unchanged),
   then iterate the active cameras: call `camera.world_to_viewport(cam_global, world_pos)`; on `Ok(vp)`,
   check `camera.logical_viewport_rect().is_some_and(|r| r.contains(vp))`. Collect the first
   qualifying camera in a **deterministic order** — sort candidates by `SplitViewportSlot(u32)` when
   present (a camera with no `SplitViewportSlot`, i.e. a single camera or the `PartyOrbitCamera`,
   sorts last; ties among `Some` slots broken by `Entity` order), so the same label always resolves
   to the same camera across frames when more than one qualifies. A `PartyOrbitCamera` sorting last
   is never actually ambiguous in practice — `dynamic_split_screen_system` never has it active at
   the same time as a split camera — but the comparator must still order it deterministically for
   robustness.
3. If no active camera's viewport contains the projected point, hide the label
   (`Visibility::Hidden`) — same failure-mode contract as today's `Err(_)` branch, just reached via
   a different condition.
4. Once a camera is selected, the existing per-label logic (depth-scale font sizing using that
   camera's distance, `Transform` write with the existing ≥0.5px change-detection guard, `Visibility`
   write) is unchanged — only the camera it's computed against changes from "the one global camera"
   to "the one camera (if any) that actually shows this point on-screen this frame."
5. `half_w`/`half_h` (window-based) stay as-is — `world_to_viewport` already returns full-window-relative
   coordinates (see research above), so no per-viewport offset math is needed beyond what
   `world_to_viewport` already does.

### Not in scope

- **Duplicating labels across simultaneously-visible split viewports.** If the same world point is
  on-screen in two active cameras' viewports at once (e.g. two players standing near the same
  portal in a grid split), only one deterministically-chosen camera's viewport shows the label this
  pass — it will not appear twice. Full per-camera duplication (mirroring the HUD corner label
  pattern) would require rearchitecting every `WorldLabel` spawn call site into a
  "logical label + N per-camera visual instances" model; a much larger change than this bug
  warrants. If a real project need for simultaneous multi-viewport labels surfaces, track it as a
  separate follow-up feature.
- **`nameplate.rs`'s separate `camera_q.single()` at `nameplate_visibility_system`
  (`nameplate.rs:212`)** — a distinct system with its own single-camera assumption for
  distance-culling. Not touched here; nameplates aren't enabled in any split-screen project today
  so this stays latent. Left as a documented limitation in `claude_suggestions.md` for a future pass.
- **`particle_renderer.rs:303` (billboard orientation) and `targeting.rs:122` (click-to-select)** —
  same class of bug, different systems, no shared code path with `WorldLabel`. Not touched here.
- **Extracting a shared "pick the active camera whose viewport shows this point" helper.** This
  fix is the first real implementation of that selection logic; `nameplate.rs`/`targeting.rs`/
  `particle_renderer.rs` each have different query shapes and result-usage (distance-cull vs.
  click-hit vs. billboard axis), so a shared helper would be premature until a second consumer
  actually needs the exact same shape. Worth revisiting if/when `nameplate_visibility_system` is
  fixed next.

## Tasks

- [ ] Update `world_label_screen_pos_system` camera query + selection logic per Approach above
- [ ] Update `depth_scale` font-size calculation to use the selected camera's distance, not a
      single global `cam_pos`
- [ ] Tests (`crates/ironhold_core/tests/`, likely a new or extended `local_coop_tests.rs` /
      dedicated file): a `WorldLabel` at a fixed `world_pos` resolves to the correct on-screen
      `Transform` when (a) exactly one active `Camera3d` exists (existing single-camera behavior
      unchanged — regression guard), (b) 2 active split cameras exist and the point is only visible
      in one of their viewport rects, (c) a merged/split transition (`Camera.is_active` toggling)
      changes which camera positions the label with no stale frame, (d) the point is outside every
      active camera's viewport rect (label hides, same as today's off-frustum case)
- [ ] Docs: no new RON surface (this is a runtime bugfix, not a schema change) — but add a short
      note to `crates/ironhold_core/src/CLAUDE.md`'s existing "Other known limitations, introduced
      by split-screen having 2 real `Camera3d` entities" paragraph, recording that
      `world_label_screen_pos_system` is now viewport-aware while `nameplate.rs`,
      `particle_renderer.rs`, and `targeting.rs` still are not — so the paragraph doesn't go stale
      and mislead the next person who checks whether `WorldLabel` is still affected.
- [ ] Update the fixed backlog bug entry and `claude_suggestions.md`'s Camera entry to reflect
      `world_label_screen_pos_system` moving from "affected" to "fixed" in the four-system list.
- [ ] Log a new `claude_suggestions.md` entry (per architecture review, 2026-07-08): a `WorldLabel`
      simultaneously visible in 2+ active split viewports at once (e.g. two players near the same
      portal in room5/room6) renders in only one deterministically-chosen viewport, not duplicated
      — reachable in shipping rooms, not purely hypothetical; deferred to a future per-camera
      duplication feature if it proves to matter in practice.

## Open questions

- None outstanding for the scoped fix. (Whether to later duplicate labels across simultaneously-visible
  viewports, and whether to fix `nameplate.rs`/`particle_renderer.rs`/`targeting.rs` the same way,
  are both deliberately deferred — see "Not in scope.")

## Acceptance criteria

- Given `local_coop_demo` room3, room4, room5, or room6, when the scene loads, then each room's
  floating "Room N" label renders positioned above its portal (not static/centered), and follows
  the portal's on-screen position as the active camera(s) move.
- Given room5's dynamic split, when the view merges (single `PartyOrbitCamera` active) or splits
  (two `SplitViewportSlot` cameras active), then the room-name label continues tracking correctly
  across the transition with no stale-frame flash.
- Given room6's 4-way grid split, when any one player's viewport shows the portal, then the label
  renders correctly positioned within that viewport (at least one active camera resolves it), with
  no crash or permanent hide.
- Given any existing single-camera (non-split) scene, when it loads, then `WorldLabel` positioning
  behavior is pixel-identical to before this fix (regression guard — this must not be a behavior
  change for the common case).
- Given a `WorldLabel`'s world position is off-frustum or off-viewport for every currently-active
  camera, when the system runs, then the label is hidden (`Visibility::Hidden`), matching today's
  existing off-frustum contract.
