# Feature: Viewport-aware `world_label_screen_pos_system` (fix split-screen `WorldLabel` positioning)

_Status: Done_
_Architecture review: PASS (2026-07-08), re-confirmed after amendment (2026-07-10) — sound
camera-selection algorithm, acceptable scope boundary, no crate/WASM concerns._
_Planned at: `9b94255` (2026-07-08)_
_Shipped at: (pending commit, 2026-07-10)_

## Playtest findings (2026-07-10)

Dev build playtested by Frank: labels correctly track their portal in both player viewports —
the core positioning bug is fixed. But: **when player 1 approaches the portal where player 2 is
standing, the label in player 2's viewport disappears.** Frank's call: this is correct/expected
for `room5`'s *dynamic* split (merging naturally leaves only one camera active — nothing to
duplicate into), but wrong for a *fixed* split screen (`room3`/`room4`/`room6`, and `room5` while
actually split) where both viewports are simultaneously, persistently rendered — a portal visible
in both should show the label in both, not just the higher-priority one.

This is exactly the gap the original plan's "Not in scope" section called out and deliberately
deferred (see below) — but the deferral assumed it was a rare, minor cosmetic edge case. Frank's
playtest shows it reads as a real, visible bug (a label just vanishing from an otherwise fully-
rendered, currently-active viewport), not a minor gap. Scope is revised to fix it — see the
amended Approach below. The fix is deliberately scoped to **only** the scene-level `world_labels:`
spawn path (portal room-name labels — the actual reported case), not nameplates/damage
popups/stat/entity labels, to avoid the "rearchitect all 5 spawn call sites" cost the original
plan flagged as out of scope. Those other consumers keep today's single-instance,
highest-priority-camera-only behavior, unchanged.

**Correction (still 2026-07-10, same playtest round): wrong spawn site fixed first.** The
`WorldLabelRank` amendment above was implemented against `scene_loader.rs`'s `scene.world_labels:`
loop (fixed-world-position labels) — but `local_coop_demo`'s actual room-name labels are authored
via each portal entity's `label:` field (`EntityLabelDef`, `tracked_entity: Some(portal)`), a
**separate** spawn loop (`for (tracked, label_def) in pending_labels`) that the first amendment
never touched. Confirmed by reading `assets/projects/local_coop_demo/scenes/room3.scene.ron`:
`label: (text: "Room 4", offset: (0.0, 4.0, 0.0))` on the portal entity, not a scene-level
`world_labels:` list. Re-tested by Frank after the first amendment — bug still reproduced exactly
as before, because the fix literally never ran for this project. The `pending_labels` loop now
gets the identical rank-duplication treatment (same `WorldLabelRank(0..MAX_SPLIT_PLAYERS)` pattern,
same rank>0-starts-`Hidden` fix) as the `world_labels:` loop — both scene-level and per-entity
`WorldLabel`-producing spawn sites are now covered.

**Final confirmation (2026-07-10).** Dev build re-tested by Frank after the spawn-site correction:
"play test confirmed" — both viewports keep showing the label when a portal is simultaneously
visible in 2 active split viewports. Release build (`cargo clean && wasm-pack ... --features
webgpu`) completed cleanly, 59 MB (well under the 95 MB warning threshold), smoke-tested via
`python serve.py` with no console errors — Frank confirmed: "test confirmed". Full
`ironhold_core` test suite passes (all 16 binaries, run one-at-a-time per this session's low-disk
machine constraints — see `claude_suggestions.md` if that pattern recurs).

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

### Amendment (2026-07-10, post-playtest): scene-level `world_labels:` now duplicate per rank

Frank's playtest showed the deferral below was wrong for portal room-name labels specifically —
see "Playtest findings" above. Revised approach, scoped to `scene_loader.rs`'s `world_labels:`
spawn loop **only**:

- New component `WorldLabelRank(pub u8)` (`runtime/scene_manager/mod.rs`, next to `WorldLabel`).
  A `WorldLabel` with no `WorldLabelRank` behaves exactly as `WorldLabelRank(0)` — fully additive,
  zero changes needed at any other `WorldLabel` spawn site (nameplate anchors, damage popups, stat
  labels, entity labels all keep today's behavior unchanged).
- `world_label_screen_pos_system`'s selection changes from `.find_map(...)` (first qualifying
  camera) to `.filter_map(...).nth(rank)` — rank 0 is identical to the old "first qualifying"
  behavior; rank N picks the (N+1)-th qualifying camera in the same deterministic order. A rank
  with no qualifying camera this frame simply hides, same failure contract as before.
- `scene_loader.rs`'s `world_labels:` loop now spawns `MAX_SPLIT_PLAYERS` (4) sibling entities per
  authored label — one at each rank 0..3 — instead of one. Each independently binds to a different
  active-camera priority, so up to 4 simultaneously-visible split viewports each get their own
  correctly-positioned copy. Cost: up to 4x entities for room-name labels only (still a handful per
  scene, not per-frame — spawned once at scene load), and up to 4x the projection math per label in
  `world_label_screen_pos_system` (already bounded — see wasm-perf review below).
- This uniformly fixes both cases Frank distinguished: room3/4/6's always-2+-active-camera fixed
  splits, and room5's dynamic split *while actually split* (2 cameras simultaneously active) — the
  "only one camera active" merged case still naturally reduces to a single non-ambiguous match, no
  special-casing needed.

### Not in scope (original, before the 2026-07-10 amendment)

- ~~**Duplicating labels across simultaneously-visible split viewports.**~~ Reversed by the
  amendment above, for scene-level `world_labels:` only. Original reasoning, still true for the
  consumers this does NOT cover: if the same world point is on-screen in two active cameras'
  viewports at once, only one deterministically-chosen camera's viewport shows a rank-0-only label
  (nameplates, damage popups, stat/entity labels) — it will not appear twice for those. Full
  per-camera duplication for all 5 `WorldLabel` spawn call sites remains a much larger change than
  this bug warrants; if a real project need for simultaneous multi-viewport nameplates/damage
  popups/stat labels surfaces, track it as a separate follow-up feature, using this fix's
  `WorldLabelRank` pattern as the reference.
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

- [x] Update `world_label_screen_pos_system` camera query + selection logic per Approach above
- [x] Update `depth_scale` font-size calculation to use the selected camera's distance, not a
      single global `cam_pos`
- [x] Tests (`local_coop_tests.rs`): a `WorldLabel` at a fixed `world_pos` resolves to the correct
      on-screen `Transform` when (a) exactly one active `Camera3d` exists (existing single-camera
      behavior unchanged — regression guard), (b) 2 active split cameras exist and the point is
      only visible in one of their viewport rects, (c) a merged/split transition
      (`Camera.is_active` toggling) changes which camera positions the label with no stale frame,
      (d) the point is outside every active camera's viewport rect (label hides, same as today's
      off-frustum case). All 4 passing.
- [x] Docs: added a note to `crates/ironhold_core/src/CLAUDE.md`'s "Other known limitations"
      paragraph, recording `world_label_screen_pos_system` as fixed while `nameplate.rs`,
      `particle_renderer.rs`, and `targeting.rs` still aren't.
- [x] Updated `claude_suggestions.md`'s Camera entry to reflect `world_label_screen_pos_system`
      moving from "affected" to "fixed" in the four-system list; logged the (now-superseded by the
      2026-07-10 amendment, for `world_labels:` specifically) multi-viewport-duplication gap as a
      separate entry.
- [x] **(2026-07-10 amendment)** Add `WorldLabelRank(pub u8)` component (`runtime/scene_manager/mod.rs`)
- [x] **(amendment)** Change `world_label_screen_pos_system`'s selection from `find_map` (first
      match) to `filter_map(...).nth(rank)`
- [x] **(amendment)** `scene_loader.rs`'s `world_labels:` loop spawns `MAX_SPLIT_PLAYERS` (4)
      ranked sibling entities per authored label instead of 1; rank>0 siblings spawn
      `Visibility::Hidden` (architecture re-review flag: avoids a one-frame flash of stacked
      duplicate labels at screen center before the first system tick hides unqualified ranks)
- [x] **(amendment)** Tests: a `WorldLabel`/`WorldLabelRank` pair simultaneously visible in 2 active
      split viewports resolves BOTH rank-0 and rank-1 siblings to their respective correct on-screen
      positions in the same frame (not just one); a rank with no qualifying camera this frame hides
      independently of its siblings; existing rank-0-implicit (untagged) tests remain a pure
      regression guard, unchanged. 2 new tests, all 51 in `local_coop_tests.rs` passing.
- [x] **(amendment)** Re-ran alignment review (ALIGNED WITH NOTES — flagged `CLAUDE.md`'s "known
      remaining gap" paragraph as stale, since fixed), architecture re-confirmation (READY, one
      minor visibility-flash flag, fixed above), wasm-perf review (OK, negligible cost) — full
      `ironhold_core` test suite passing
- [x] **(amendment)** Updated `crates/ironhold_core/src/CLAUDE.md`'s "Known remaining gap"
      paragraph to reflect `world_labels:` now duplicating across viewports, while nameplates/
      damage popups/stat/entity labels still don't
- [x] **(amendment)** New WASM dev build + updated playtest checklist, Frank re-confirmation —
      bug still reproduced (see "Correction" above: wrong spawn site fixed first)
- [x] **(correction)** Applied identical rank-duplication to `pending_labels`/`EntityLabelDef`
      loop (the spawn site `local_coop_demo`'s portals actually use); added a scene-load-level
      regression test (`test_entity_label_ranks_spawn_for_tracked_entity_labels_not_just_world_labels`)
      driving a real `spawn_scene_v2` with a `label:`-bearing entity, to catch this exact class of
      mistake going forward. Full test suite passing (16/16 binaries)
- [x] New WASM dev build + updated playtest checklist — Frank confirmed
- [x] WASM release build (59 MB) + smoke-test — Frank confirmed

## Open questions

- None outstanding. (Whether to extend the same rank-duplication pattern to
  `nameplate.rs`/`particle_renderer.rs`/`targeting.rs`, or to nameplates/damage popups/stat/entity
  `WorldLabel` consumers, is deliberately deferred — see "Not in scope" — unless a real project
  need surfaces.)

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
- **(2026-07-10 amendment)** Given a portal simultaneously visible in 2+ active split viewports at
  once (e.g. player 1 approaches the portal where player 2 is already standing, in a fixed split
  room or room5 while actually split), when the frame updates, then the room-name label appears
  correctly positioned in EVERY viewport that can see it, not just one — no viewport's label
  disappears just because another player can also see the same portal.
- Given any existing single-camera (non-split) scene, when it loads, then `WorldLabel` positioning
  behavior is pixel-identical to before this fix (regression guard — this must not be a behavior
  change for the common case).
- Given a `WorldLabel`'s world position is off-frustum or off-viewport for every currently-active
  camera, when the system runs, then the label is hidden (`Visibility::Hidden`), matching today's
  existing off-frustum contract.
