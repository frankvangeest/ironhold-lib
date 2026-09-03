# Feature: Local Co-op Split-Screen Player HUD Labels

_Status: Done_
_Planned at: `ef55f87` (2026-07-07)_
_Shipped at: `c84d878` (2026-07-08)_

## Playtest findings (2026-07-08)

- Dev and release builds both confirmed: P1-P4 corner labels render correctly top-right in
  room3/room4/room6, hide/show correctly across room5's dynamic split, no regressions.
- Post-implementation review (alignment-reviewer + wasm-perf-reviewer, both run on commit
  `6c3fa0f`): alignment ALIGNED (two non-blocking warnings — stale `PlayerIndex` doc comments,
  fixed by `c84d878`; no RON opt-out, deliberately out of scope per this plan's "Not in scope"
  section). wasm-perf found one real issue — `split_viewport_player_label_update_system` wrote
  `Node.left`/`top` unconditionally every frame, forcing a full UI relayout on every split-screen
  frame; fixed in `c84d878` by guarding the write like the adjacent `Visibility` write.
- Separately discovered (not part of this feature): the pre-existing portal room-name labels
  (`WorldLabel`/`EntityLabelDef`, added earlier this session) render mis-positioned in every
  split-screen room because `world_label_screen_pos_system` assumes exactly one `Camera3d`. This
  is the real-world trigger of a limitation already predicted in `claude_suggestions.md` ▸ Camera
  before any split-screen code existed. Logged as a bug in `planning/backlog.md` ▸ Bugs; a proper
  viewport-aware fix is planned as a separate follow-up feature per Frank's direction — not folded
  into this plan.

## What

A small colored "P1"/"P2"/"P3"/"P4" corner label automatically overlaid on each player's own
split-screen viewport in `local_coop_demo`, so anyone watching a shared screen can immediately
tell which quadrant/half belongs to which player. Derived entirely from the already-existing
`PlayerIndex` component and each split camera's live `Camera.viewport` — no per-scene manual RON
authoring, works automatically for any orientation (`Vertical`/`Horizontal`/`Grid`) and follows
window resize.

## Why

Backlog item, deliberately deferred from Stage 1 ("P1/P2 nameplate & HUD distinction") until
split-screen viewports existed to put per-player UI in. Stages 3-6 now all ship real per-player
split cameras (`SplitViewportSlot`), so this is the natural place for that HUD to land. It's also
the first real consumer of `PlayerIndex(u32)` — Stage 1 added the component and inserted it on
every GLB player entity, but no system has read it until now.

## Research findings (confirmed by reading the current code, verified by architecture + UX review)

- **Re-confirmed by architecture review (2026-07-07): zero new schema surface.** No RON fields, no
  new `Action`/event types — `SplitScreenPlayerLabel`/`LinkedPlayerLabel` are pure runtime
  components, not serializable `schema/` types, so schema stability is a non-issue for this
  feature. Crate boundaries are clean: everything lives in `capabilities/camera.rs` plus one
  `.chain()` registration in `lib.rs:290-299` — no platform-specific code, nothing leaks into
  `ironhold_native`/`ironhold_web`.
- `PlayerIndex(u32)` (`capabilities/player.rs:41`) is inserted on every GLB player entity at spawn
  (`spawn_player_entity_core`) but consumed by zero systems today — this feature is its first
  real use.
- `SplitViewportSlot` cameras are real `OrbitCamera`s, so `OrbitCamera.target` → `PlayerIndex` is a
  valid lookup. `Camera.viewport` is already computed correctly every frame by
  `split_screen_viewport_system` for all three orientations — this feature reads that value
  directly rather than recomputing cell rects itself.
- `Camera.viewport` is in **physical** pixels; Bevy UI `Node.left`/`top` are in **logical**
  pixels — needs `window.scale_factor()` to convert (the mirror image of
  `split_screen_viewport_system`'s own physical-pixel care).
- **Confirmed by architecture review: UI targets full-window logical space, not per-viewport
  space.** The persistent `Camera2d` (`lib.rs:404-413`) has `IsDefaultUiCamera` commented out, but
  every existing RON UI root (`scene_loader.rs:1228,1298`) is a full-window 100%×100% node, and
  room6's per-quadrant labels are already authored in full-window 1280×720 coordinates — so a
  standalone `Node` resolves against the same full-window UI camera the RON roots use, and the
  physical-viewport→logical-window math is correct as originally planned. This dependency is
  implicit today (nothing states it outright) — worth a one-line comment at the new system's spawn
  site pointing at this precedent, so a future refactor of the `Camera2d` setup doesn't silently
  break it.
- **Confirmed by architecture review: system ordering already eliminates any one-frame staleness
  risk.** `lib.rs`'s existing `.chain()` runs `dynamic_split_screen_system` → `split_screen_viewport_system`
  → (this new system, `.after` the latter) — so on the exact frame a merge/split transition flips
  `Camera.is_active` and recomputes `Camera.viewport`, this system reads both already-fresh values
  in the same frame. No stale position or visibility for even one frame.
- One persistent global `Camera2d` (`order: 1000`, `ClearColorConfig::None`, survives scene
  transitions) already renders every UI overlay across all viewports with zero coupling to any 3D
  camera's `viewport` field.
- Dynamic split (Stage 5) never spawns/despawns cameras — it toggles `Camera.is_active` on
  already-existing split cameras. This feature's labels must mirror that (hidden while their
  camera is inactive, i.e. during the merged/party state).
- Deliberately **not** touching `nameplate_visibility_system`'s separate `camera_q.single()`
  assumption. Confirmed during this research: nameplates currently no-op entirely in any
  split-screen scene, since `.single()` fails whenever 2+ real cameras exist — `local_coop_demo`
  has never enabled nameplates, so this has been latent and invisible until now. This exact gap
  was already predicted and logged in `planning/claude_suggestions.md` ▸ Camera during Stage 3
  planning (2026-07-05) — no new entry needed. Fixing it is a separate, deeper
  multi-camera-projection problem (shared by `world_label_screen_pos_system`,
  `particle_renderer.rs`, `targeting.rs` too) and explicitly out of scope here.
- **UX review correction — top-left collides with the scene title, in EVERY room, not just
  room6.** Original draft assumed top-left avoided room6's bottom-anchored control hints, but
  missed that `room_hint` sits at `position: (20, 20)` in *every* split scene (`room3`/`room4`/
  `room5`/`room6`) — the top-left corner of P1's own cell. In room6 the title's `size.x: 900` even
  overflows past P1's ~640px-wide quadrant into P2's. **Corner changed to top-right of each cell**
  — dodges every room's title label, still a natural at-a-glance position.
- **UX review: only room6 has per-player material tints (`tint_blue`/`pink`/`dark_green`/`red`);
  rooms 3/4/5 use plain `character_male`/`character_female` with no `material` override at all.**
  Reading a "tint" for the label color would silently degrade to no color in three of four
  split-screen rooms. Resolution below uses a fixed engine-side palette instead.

## Approach

- New component `SplitScreenPlayerLabel` (marker, no fields — spawned as a child/attachment
  pattern mirroring `nameplate_setup_system`'s `Added<NameplateTag>` convention) placed on the
  **camera** entity once its label has been spawned, so the spawn system can use `Added<>`
  filtering instead of a per-frame "does a label already exist for this camera" scan (per
  architecture review's idiom note). The actual UI label entity's id is looked up via a second
  component `LinkedPlayerLabel(pub Entity)` stored on the camera, pointing at the UI entity.
- New system `split_viewport_player_label_spawn_system` (`Added<SplitViewportSlot>` filter,
  mirroring `nameplate_setup_system`): for each newly-added split camera whose `OrbitCamera.target`
  has a `PlayerIndex`, spawns one UI `Text` node with:
  - `position_type: PositionType::Absolute` (explicit, matching `scene_loader.rs:1319`'s existing
    positioned-label convention — per architecture review, don't rely on the default).
  - Explicit `Visibility::Visible` (don't rely on `Inherited` on an unparented entity — per
    architecture review).
  - Text `"P{player_index + 1}"`, computed once at spawn (player index never changes at runtime).
  - Text color from a **fixed 4-entry engine palette** (`[Color; 4]` constant, e.g. in
    `capabilities/camera.rs` next to `MAX_SPLIT_PLAYERS`) indexed by `player_index`, chosen to
    visually match Stage 6's `tint_blue`/`pink`/`dark_green`/`red` RGB values — but is its own
    independent constant, NOT a read of the prefab's `material` field. This keeps the label
    color consistent across all 4 split rooms (3/4/5 included, which have no tint) while still
    matching room6's tints there. Must be documented clearly (see Docs task) that re-tinting a
    player's `material` in RON does **not** move the label's color — they're independently
    sourced by design, not synced.
  - A text outline/shadow (Bevy's `TextShadow` or a duplicate offset-behind text node) so the
    label stays legible against every room's differently-toned ground (UX review: golden room6,
    amber room3, etc. — a flat colored glyph risks low contrast on some).
  - Tagged `LevelEntity` for automatic scene-teardown cleanup (matching every other
    scene-scoped UI element — no new resource or explicit clear logic needed).
- New system `split_viewport_player_label_update_system`, `.after(split_screen_viewport_system)`
  in the existing `lib.rs` `.chain()`: for every camera with a `LinkedPlayerLabel`, updates the
  linked UI entity's `Node.left`/`top` from `camera.viewport.physical_position /
  window.scale_factor()` plus a small fixed margin (e.g. 8px, top-right anchored — i.e. `left =
  viewport_right_edge_logical - label_width - 8`, `top = viewport_top_edge_logical + 8`), and
  syncs `Visibility` to `camera.is_active`.
- Only ever spawns for scenes with real split cameras — party/single-camera/no-split scenes never
  have a `SplitViewportSlot` entity, so `Added<SplitViewportSlot>` never fires there. No new
  scene-level RON opt-in flag needed; this is fully automatic wherever `split:` is configured.

### Not in scope for this feature

- Fixing `nameplate_visibility_system`'s single-camera assumption or enabling nameplates in
  `local_coop_demo` — a separate, deeper multi-camera-projection problem already tracked in
  `claude_suggestions.md`, not folded into this feature.
- Per-player HUD beyond an identity label (health bars, stats, etc.) — `local_coop_demo` has no
  stats system wired in.
- A controller-icon variant for gamepad-bound players — `local_coop_demo` doesn't use
  `gamepad_index` in any of its rooms today.
- Syncing the label's color to a scene's actual `material:` tint (if any) — deliberately a fixed
  independent palette, see Approach above.

## Tasks

- [x] `SplitScreenPlayerLabel` marker + `LinkedPlayerLabel(pub Entity)` components; fixed 4-entry
      `PLAYER_LABEL_COLORS` palette constant (`capabilities/camera.rs`)
- [x] `split_viewport_player_label_spawn_system` (`Added<SplitViewportSlot>` filter, mirroring
      `nameplate_setup_system`'s idiom) — spawns the UI `Text` node with explicit
      `PositionType::Absolute`, explicit `Visibility::Visible`, text outline/shadow, `LevelEntity`
      tag
- [x] `split_viewport_player_label_update_system`, `.after(split_screen_viewport_system)` in
      `lib.rs`'s `.chain()` — updates position (top-right anchored) + visibility every frame.
      Guarded against unconditional change-detection writes (`c84d878`, wasm-perf follow-up).
- [x] One-line comment at the spawn site documenting the "UI targets the full-window `Camera2d`,
      not per-viewport space" dependency (per architecture review), so a future `Camera2d`/
      `IsDefaultUiCamera` refactor doesn't silently break this
- [x] Tests: label spawns exactly once per `SplitViewportSlot` camera whose target has a
      `PlayerIndex`; position correctly converts physical `Camera.viewport` to **window-logical**
      `Node` coordinates (architecture review: assert against window-logical, not
      viewport-logical, coords) including a HiDPI/scale-factor-override case (mirroring
      `test_split_screen_viewport_unaffected_by_scale_factor_override`); visibility mirrors
      `Camera.is_active` across a dynamic-split merge/split transition with no stale frame; no
      label spawns for a party or single-player scene (no `SplitViewportSlot` exists there);
      label color matches the fixed palette by `player_index`, independent of any `material` field.
      7 new tests, all passing (`local_coop_tests.rs`).
- [x] Docs: `docs/20_data_formats.md` — engine-automatic behavior note (no new RON field), must
      state explicitly: (a) label text is driven by `player_index`, NOT by scene entity/spawn
      order — a designer must give each player prefab a distinct `player_index` that matches its
      intended quadrant, or two players could show the same "P" number or a mismatched one; (b)
      label color is a fixed engine palette, independent of and NOT synced to a player's
      `material:` tint. `crates/ironhold_core/src/CLAUDE.md` gets the same two notes plus the
      `Added<SplitViewportSlot>`/`LinkedPlayerLabel` pattern summary.
- [x] Full review gate: alignment (ALIGNED, `6c3fa0f`/`c84d878` — confirmed zero-RON-surface is
      correct; stale `PlayerIndex` doc comments fixed), architecture (reviewed at plan stage,
      implementation matches), wasm-perf (found and fixed an unguarded `Node` write causing
      per-frame UI relayout on split-screen scenes, `c84d878`)
- [x] WASM dev + release build, playtest checklist (confirm labels appear top-right in `room3`/
      `room4`/`room6` without colliding with any room's title label; correctly hide/show across
      `room5`'s dynamic merge/split; legible against each room's ground tone), Frank confirmation.
      Confirmed twice — once for the initial feature (`6c3fa0f`) and once for the wasm-perf
      follow-up fix (`c84d878`).

## Open questions

None outstanding — resolved during architecture and ux-gamedesigner-reviewer plan review
(2026-07-07):
- **Corner** — top-right (not top-left as originally drafted), to dodge every room's title label.
- **Color** — a fixed engine palette matching Stage 6's tint RGB values, not a read of the actual
  `material:` field (only room6 has one).
- **Staleness across merge/split transitions** — confirmed non-issue by the existing `.chain()`
  ordering.
- **UI coordinate space** — confirmed full-window logical, matching every existing RON label's
  coordinate convention.

## Acceptance criteria

- Given a `Vertical`/`Horizontal`/`Grid` split-screen scene, when it loads, then each player's own
  viewport shows a colored "P{n}" label (matching their `PlayerIndex`) in the top-right of their
  own cell, with no label overlapping another player's cell or any room's title label.
- Given `room5`'s dynamic split, when the view is merged (single shared camera), then no
  per-player corner labels are visible; when it splits, the correct two labels appear in the
  correct halves with no stale position or visibility from before the split.
- Given the browser window is resized, when the next frame renders, then every visible corner
  label's position updates to stay correctly anchored to its (possibly resized) quadrant.
- Given a party-mode or single-player scene, when it loads, then no corner labels are spawned at
  all (no `SplitViewportSlot` camera exists to attach one to).
- Given a player's `material:` tint is changed in RON, when the scene reloads, then the label
  color is unaffected (fixed palette, independent of `material`).
