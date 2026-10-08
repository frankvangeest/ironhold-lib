# Split-screen cameras, target rings and per-viewport widgets

Ring `RenderLayers`, the per-viewport target HUD and player labels, `WorldLabelRank` duplication and viewport-aware selection.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Target indicator ring, per-player, colour precedence

<!-- b:352 -->

**Target indicator** (`capabilities/target_indicator.rs`) — a ground-ring decal that tracks the
selected entity, now **one independent ring per player**. Activated via `target_indicator:` in
scene RON (references a `decals:` catalog key). `target_indicator_system` runs in `Update`,
watches `LoadedTargetIndicator` (mesh-cache rebuild) and each player's `Changed<PlayerTarget>`
(spawn/despawn that player's own ring only), and manages one `TrackingTarget` entity per player.
`TrackingTarget` now carries both `target: Entity` (the tracked world entity) and
`owner: Entity` (the player entity whose ring this is), instead of just the tracked entity — the
`owner` field is what lets one player's target change despawn/respawn only their own ring without
touching any other player's. The indicator is tagged `LevelEntity` and does NOT go through the
action pipeline (it is a pure cosmetic side-effect of the target state).

Ring colour is resolved per-target at target-switch time via three-tier precedence **only in a
single-player scene**:
1. `PrefabDef.indicator_color` — direct RGBA override on the prefab (highest priority)
2. `PrefabDef.indicator_category` — string key looked up in `TargetIndicatorDef.named_colors`
3. `TargetIndicatorDef.color` — scene-level fallback

**Whenever 2+ players are present, every ring is tinted by the fixed `PLAYER_LABEL_COLORS`
palette instead** (same palette the split-screen "P{n}" corner HUD label uses, see
`capabilities/camera.rs`) — the per-target precedence just described is overridden entirely, so it's
visually obvious whose ring belongs to whom. If two players target the same entity, both rings
render, coincident, each in its own player's colour; there is no deduplication. This is a
deliberate design decision (`planning/features/per_player_split_screen_targeting.md`), not an
oversight — a per-target colour would make it impossible to tell whose ring is whose once two
players can each select something different.

The system reads the target entity's `PrefabKey` component, looks up the prefab in `LoadedPrefabCatalog`,
and applies the precedence chain (single-player only). Material handles are memoised by resolved `[u32;4]` colour bits —
alternating between two targets/players of the same resolved colour creates no new `StandardMaterial`. The mesh handle
(radius-driven, colour-independent) is a single cached `Local`; both caches clear on scene change.

## Per-viewport ring visibility: RenderLayers intro

<!-- b:383 -->

**Per-viewport ring visibility** (`SplitScreenDef.own_viewport_only`, `docs/20_data_formats.md`) —
opt-in restriction so a ring is only visible in its owner's own split viewport, instead of every
ring rendering in every viewport (the tinting described earlier is unaffected either way; this only changes
*where* a ring renders). Built on Bevy `RenderLayers` — the first designer-facing feature to use
them; the only prior usage was `inspector.rs`'s feature-gated debug camera on reserved layer 31
(untouched by this feature). Layers 1–4 are reserved, one per split player, indexed identically to
`PLAYER_LABEL_COLORS`'s own scheme. `capabilities::camera::ring_layer_for_player(player_index)` and
`all_ring_layers()` are the sole owners of this arithmetic — every insertion site listed on this page calls one
of the two rather than re-deriving `1 + player_index % MAX_SPLIT_PLAYERS` by hand, so raising
`MAX_SPLIT_PLAYERS` can never desync one site from another. Components are only ever inserted when
`own_viewport_only == true` — zero `RenderLayers` footprint on any entity when it's `false` (the
default), verified by a regression test that default settings spawn zero `RenderLayers` components
anywhere.

## Orbit camera / ring layer assignment

<!-- b:396 -->

- Each split `ActiveCameraMode::Orbit` — both the static `Grid`/`Vertical`/`Horizontal` loop and the
  `dynamic`-split loop (`entity_spawner.rs`'s `spawn_players_and_camera` and
  `spawn_split_camera_for_player`) — gets `RenderLayers::layer(0).with(ring_layer_for_player(
  player_index))`, keyed on `PlayerConfig.player_index`, not spawn/loop order — they can diverge
  when a scene authors player entities out of `player_index` order (see the reversed-order test);
  hot-join can NOT diverge here, since `Action::JoinPlayer` sets both `player_index` and the spawn
  slot to the same `next_slot` value.
- Each ring entity (`target_indicator_system`) gets `RenderLayers::layer(ring_layer_for_player(
  owner_player_index))` only — no layer 0, since a ring never needs to be "ordinary scene
  geometry."

## `TargetRingVisibilityMode` lifecycle

<!-- b:414 -->

- `TargetRingVisibilityMode` (`AllViewports` default / `OwnViewportOnly`,
  `runtime/scene_manager/mod.rs`) is the resolved runtime state `target_indicator_system` reads —
  `init_resource`'d in `lib.rs` so it's never missing, resolved by `spawn_players_and_camera` for
  every scene (including single-player/party-only), and reset to `AllViewports` on a full
  `Action::LoadScene` (`action_executor.rs`). `RenderLayers` is applied at ring-spawn time only,
  never re-applied to already-live rings — safe only because every write site to this resource is
  paired with a full `LevelEntity` teardown (rings carry `LevelEntity`), so no live ring can ever
  outlive the mode it was spawned under. A future mid-scene toggle (e.g. a settings menu) would
  need to re-tag every live `TrackingTarget` entity on an `is_changed()` branch — this resource does
  not do that today.

## Per-viewport target HUD readout

<!-- b:452.ref -->

**Per-viewport target HUD readout** (`capabilities/camera.rs`'s `target_hud_spawn_system`/
`target_hud_update_system`) — opt-in via the new `GameSceneV2.target_hud: Option<TargetHudDef>`
scene field (`docs/20_data_formats.md`). Mirrors the existing `split_viewport_player_label_spawn_
system`/`_update_system` pattern exactly: one `Text` entity per `SplitViewportSlot` camera
(`Added<SplitViewportSlot>`-triggered spawn, only when the scene authors a `target_hud:` block),
kept in sync every frame with that camera's owning player's `PlayerTarget` (via
`CameraTargets`) and the camera's live `Camera.viewport`/`is_active`. Anchored bottom-left
(the corner label is top-right) so the two never collide. `target_hud_update_system` is chained
`.after(split_screen_viewport_system)` in `lib.rs`, same ordering guarantee as the corner-label
update system, so there's no stale-frame risk across a `dynamic` split's merge/split transition.
`TargetHudDisplay` (`Full`/`NameOnly`/`IdOnly`) controls which of prefab/id/name the readout shows.

## Local co-op: Party, split, Grid, dynamic split, fallback

<!-- b:1261 -->

### Local co-op: shared camera, split-screen, gamepad routing, view-box clamp

**`ActiveCameraMode::Party`** (`capabilities/camera.rs`) is a sibling to `ActiveCameraMode::Orbit`, not a
replacement — single-player scenes are untouched. When a scene has 2+ `tags: ["player"]`
entities, `spawn_players_and_camera` reads the **first** player's `CameraConfig.party:
Option<PartyZoomDef>` and `CameraConfig.split: Option<SplitScreenDef>` as the explicit switches
(mutually exclusive — if both are set, `split` wins and a warning is logged):
- `party` set → spawns one `ActiveCameraMode::Party` framing the midpoint of all players; radius is
  `clamp(max_pairwise_separation + zoom_margin, min_radius, max_radius)`, recomputed every frame
  by `party_camera_follow_system`. `PartyZoomDef.allow_manual_zoom` (default `false`) controls
  whether scroll-wheel still nudges the derived radius via an accumulated offset.
- `split` set → spawns one **real `ActiveCameraMode::Orbit` per player** (not `ActiveCameraMode::Party`), each
  tagged `SplitViewportSlot(u32)` (which cell it owns — slot index = spawn order, i.e. entity
  order in the scene's `entities:` list). `split_screen_viewport_system` recomputes every
  `SplitViewportSlot` camera's `Camera.viewport` every frame from `Window::physical_size()`
  (physical pixels already — no manual `scale_factor()` multiplication needed, unlike a naive
  `width()`/`height()` read) and `ActiveSplitScreen`'s orientation (`SplitOrientation::Vertical`
  splits left/right, `Horizontal` splits top/bottom, `Grid` computes an N-cell grid, described later on this page).
  Split-screen orientation lives in the `ActiveSplitScreen` resource (mirrors `ActiveViewBox`/
  `LoadedTargetIndicator` — populated by `spawn_players_and_camera`, cleared on `LoadScene`),
  **not** on `ActiveCameraMode` or `SplitViewportSlot` — this kept split-screen state out of the
  camera components, which is exactly why the `camera_modes.md` v1 unification didn't have to
  untangle it.
  `Vertical`/`Horizontal` are always exactly 2-way (`.take(2)` in `spawn_players_and_camera`'s
  `split` branch); only `Grid` (Stage 6) unlocks N-way, `.take(slot_count)` where
  `slot_count = entities.len().min(MAX_SPLIT_PLAYERS)` (`MAX_SPLIT_PLAYERS = 4`, `camera.rs`) —
  a `Grid` scene with more players than the cap spawns the extras cameraless, same as what
  already happened pre-Stage-6 if a 3rd player existed in a `Vertical`/`Horizontal` scene.
- `Grid` orientation (Stage 6) → `split_screen_viewport_system` reads a separate resource,
  **`ActiveSplitSlotCount(Option<u32>)`** (populated once by `spawn_players_and_camera` at scene
  load, cleared on `LoadScene` — mirrors `DynamicSplitConfig`'s exact write-once lifecycle), for
  its player count, rather than counting `SplitViewportSlot` cameras live in the query each frame.
  This was a deliberate architecture-review fix during planning: `Grid` was built as the
  foundation for hot-join (`local_coop_hot_join_leave.md`, now implemented) — deriving the count
  live would silently reflow the grid on any mid-transition entity churn, whereas a stored,
  explicitly-written count doesn't. `drain_spawn_queue_system`'s `Action::JoinPlayer` branch is now
  the second writer of this resource (alongside `spawn_players_and_camera` at scene load) —
  incrementing it by one per successful hot-join rather than recomputing from a live query is what
  makes that safe. Layout:
  `cols = ceil(sqrt(count))`, `rows = ceil(count / cols)`, cell assigned row-major by `slot.0`
  (`row = slot.0 / cols`, `col = slot.0 % cols`); the last row/column absorbs the remainder on an
  odd window dimension, same pattern as `Vertical`/`Horizontal`. `count == 3` leaves one grid cell
  (slot `3` of a 2×2 grid) with no camera — renders as clear color, not a special-cased 3-pane
  layout. `Grid` does **not** support `split.dynamic` — dynamic merge/split stays `Vertical`/
  `Horizontal`-only.
- `split.dynamic: Option<DynamicSplitDef>` set (Stage 5) → the view starts **merged** (its own
  internal `ActiveCameraMode::Party`, tuned by `DynamicSplitDef.merged_zoom_margin`/
  `merged_allow_manual_zoom` — mirrors `PartyZoomDef`'s two fields, self-contained specifically so
  dynamic split doesn't also require authoring a `party:` block alongside `split:`) and
  auto-splits into the two per-player Orbit-mode cameras once `split_distance` is exceeded, merging
  back below `merge_distance` (hysteresis — the gap prevents flicker right at one boundary). <!-- audit:ok -->
  `dynamic_split_screen_system` (`capabilities/camera.rs`) runs every frame, `.after(
  party_camera_follow_system)` and `.before(split_screen_viewport_system)` (see the `.chain()` in
  `lib.rs`), and decides merged-vs-split purely by toggling `Camera.is_active` on the
  already-spawned party/split cameras — **it never spawns or despawns cameras**; all three exist
  for the scene's lifetime, so there is no pop/snap on transition since inactive cameras keep
  tracking their targets the whole time (`camera_orbit_system`/`party_camera_follow_system` don't
  gate on `is_active`). The split axis (`Vertical` vs `Horizontal`) is chosen automatically from
  `abs(dx)` vs `abs(dz)` between the two players only at the merged→split transition instant, then
  held fixed for that whole split period — `SplitScreenDef.orientation` becomes a rare tie-break
  hint (used only when dx/dz are exactly equal) rather than the authored axis. **Unlike the
  fixed-orientation case, `ActiveSplitScreen` is continuously rewritten while dynamic mode is
  active** — every merge/split transition updates it — rather than being write-once at scene load;
  `DynamicSplitConfig` (the static per-scene tuning resource, populated once at scene load like
  `ActiveSplitScreen` itself) is what stays write-once.
- Neither set → logs a warning and falls back to a single `ActiveCameraMode::Orbit` targeting only the
  first player. Never silently spawns one `ActiveCameraMode::Orbit` per player without split-screen viewports
  — that would mean two cameras fighting for the same full-window viewport with no RON-visible
  symptom.

## `WorldLabelRank` viewport-aware labels; per-rank widget duplication

<!-- b:1406 -->

**`world_label_screen_pos_system` (`lib.rs`) is viewport-aware** (fixed — this was the root cause
of "Portal room-name labels render static and mis-positioned in every split-screen room"; see
`planning/features/world_label_split_screen_positioning.md`). It queries every active `Camera3d`
(`camera.is_active`, not `.single()`) and, per `WorldLabel`, picks the `WorldLabelRank`-th
(default 0 when the component is absent) active camera whose own `logical_viewport_rect()`
actually contains the point's `world_to_viewport()` projection — deterministic order:
`SplitViewportSlot` index first (cameras with no slot, e.g. `ActiveCameraMode::Party`, sort last),
tie-broken by `Entity`. This fixes positioning for every `WorldLabel` consumer at once (room
labels, entity labels, stat labels, damage popups, nameplate anchors), since they all share this
one system.

**Scene-level `world_labels:` (portal room-name labels) duplicate across simultaneously-visible
split viewports** (2026-07-10 playtest amendment — Frank found the single-camera-per-label
behavior described earlier showed a label vanishing from a fixed split screen's *other*, still-fully-rendered
viewport whenever a portal became visible in both at once). `scene_loader.rs`'s `world_labels:`
spawn loop now spawns `MAX_SPLIT_PLAYERS` (4) sibling entities per authored label — ranks 0..3,
via the `WorldLabelRank(u8)` component — instead of just one. Each sibling independently binds to
a different active-camera priority in `world_label_screen_pos_system`'s selection described earlier, so up to
4 simultaneously-visible active split viewports each get their own correctly-positioned,
independently-hideable copy. **Extended to `stat_label` and `Ascii`-style `world_stat_bar` in
Phase 4** (`planning/features/split_screen_camera_followups.md`) — same rank-duplication
pattern, at both spawn sites (`scene_loader.rs`'s scene-load loops and
`drain_dynamic_stat_ui_system`'s `Action::Spawn`/wave-spawn path), but gated on the loading scene
actually being split-screen (`player_configs.first().camera.split.is_some()` at scene-load time,
or `ActiveSplitScreen`/`DynamicSplitConfig` at runtime) — unlike `world_labels:`/`label:`, which
duplicate unconditionally. The gate exists because these widgets are rewritten every frame by
`stat_label_update_system`/`world_stat_bar_update_system` regardless of `Visibility`, so
unconditional duplication would be pure per-frame overhead in every ordinary (non-split) scene;
ordinary scenes get exactly 1 entity per widget, unchanged. **Extended to `ShowDamagePopup`/
`ShowFloatingText` in Phase 2 of `per_player_split_screen_targeting.md`** — same gate
(`action_executor.rs`'s `Action::ShowDamagePopup`/`ShowFloatingText` handlers read
`SceneStateParams.active_split`/`dynamic_split`), needed so a damage popup or floating text shows
in whichever viewport the target is actually visible in, not just the single highest-priority
active camera regardless of which player's action triggered it — surfaced during that phase's
playtest (a damage popup consistently appeared in player 1's viewport even when player 2 was the
one dealing the hit). **Extended to `Pixel`-style `world_stat_bar` in
`pixel_world_stat_bar_split_screen_duplication.md`** — `spawn_world_stat_bar_widget`'s `Pixel`
arm now duplicates its whole anchor+children hierarchy per rank exactly like the `Ascii` arm
already did (border/background mesh+material handles are registered once and cloned across
ranks; the fill is created fresh per rank). **`Icon`-style `world_stat_bar` built in with
day-one split-screen support** (`world_icon_stat_bar.md`) — its arm uses the same per-rank anchor
pattern from the start (texture + `TextureAtlasLayout` registered once and cloned across
ranks/cells; each `Sprite` cell created fresh, matching Pixel's fill-sharing precedent).
**`Textured`-style `world_stat_bar` also built in with day-one split-screen support**
(`world_textured_stat_bar.md`) — a 9-sliced continuous fill bar cropped from one shared
`texture_sheet` via a static `Sprite.rect` per layer (no `TextureAtlasLayout` needed, unlike
`Icon`, since each layer only ever draws one fixed sub-rect). The one `Handle<Image>` and the
`TextureSlicer`/`SpriteImageMode` are registered once and cloned across both layers and every
rank; the empty/track layer is static (`bg_color`-tinted once at spawn), only the fill layer's
`custom_size`/`color` update per frame via `world_textured_bar_update_system`
(`WorldTexturedBarFillMarker`), mirroring `world_pixel_bar_update_system`'s translation math and
change-detection guards exactly. Replaced the `Icon` hearts bar on `3rd_person_game_demo`'s
`player_male`/`player_female` as its playtest demo. **Damage popups and nameplate anchors remain
single-instance** (no `WorldLabelRank`, implicit rank 0 = highest-priority camera only) — the same
multi-viewport gap still applies to them; extend the same pattern to a given consumer's spawn site
only if a real project need surfaces.

## `particle_renderer` billboard viewport-aware (Phase 1)

<!-- b:1463 -->

**`particle_renderer.rs`'s billboard orientation is now viewport-aware** (fixed — Phase 1 of
`planning/features/split_screen_camera_followups.md`). `rebuild_pool_meshes_system` used to call
`camera_q.single()` with no `is_active` filter at all, so it fell back to unconditional world-axis
billboarding (`Vec3::X`/`Vec3::Y`) in *every* split-screen project, not just when 2 cameras were
simultaneously active — the widest-reaching of these sites. It now filters `is_active` and
picks the highest-priority active camera via the new shared
`capabilities::camera::camera_priority_key(entity, slot)` helper (same `SplitViewportSlot`-then-
`Entity` deterministic order as `world_label_screen_pos_system`, which was refactored to call the
same helper instead of inlining its own copy). **Known, accepted limitation**: with 2
simultaneously active split cameras at different angles, particles still only billboard correctly
toward the one picked camera — true per-viewport-correct billboarding would need duplicate
particle meshes per viewport, out of scope for this fix.

## Targeting `click_select` viewport-aware (Phase 2)

<!-- b:1476 -->

**`targeting.rs`'s click-to-select is now viewport-aware** (fixed — Phase 2 of
`planning/features/split_screen_camera_followups.md`). `click_select_system` used to pick the
first active `Camera3d` via `.find(|c| c.is_active)`, ignoring where the cursor actually was — a
click in player 2's viewport could silently be evaluated against player 1's camera. It now filters
to active cameras whose `logical_viewport_rect()` contains the cursor position before running the
nearest-entity search, using the same shared `camera_priority_key` comparator as Phase 1 to break
ties (cursor exactly on a shared viewport boundary) deterministically. **Known, minor behavior
change**: a click in a screen region no active camera's viewport covers (e.g. a dead grid quadrant
per Stage 6's 3-player 2×2 case) now does nothing, whereas the old arbitrary-camera pick used to
fall through to "clicked empty space" and clear `CurrentTarget`. Invisible in ordinary
single-camera scenes (a full-window viewport covers every in-window cursor position).

## Nameplate distance culling via stored camera distance (Phase 3)

<!-- b:1488 -->

**`nameplate.rs`'s distance-culling is now viewport-aware via store-and-read** (fixed — Phase 3 of
`planning/features/split_screen_camera_followups.md`). `nameplate_visibility_system` used to call
`camera_q.single()` with no `is_active` filter at all, so it silently no-op'd whenever 2+
`Camera3d` entities existed *at all* — not just when 2+ were simultaneously active (a merged
dynamic split with one inactive sibling camera hit this too). It no longer queries cameras
directly: `world_label_screen_pos_system` (which already selects one active camera per
`WorldLabel` each frame, containment-tested) now also stashes that camera's distance onto a new
`NameplateCameraDistance(Option<f32>)` component on the nameplate anchor — `None` on every
early-return path (tracked entity gone/hidden, or no qualifying camera this frame),
`Some(distance)` on the success path. `nameplate_visibility_system` reads that stashed value
instead of independently re-selecting a camera, guaranteeing the two systems can never disagree on
which camera is authoritative for a given anchor's position and visibility. An anchor with no
stashed distance (off every active viewport) is treated as out-of-range (hidden), matching the
prior no-op contract for "no qualifying camera." **Known, minor behavior change**: the culling
distance is now measured from the anchor's actual world position (tracked entity origin +
`NameplateOptionsDef.offset`, default `(0, 2.4, 0)` — the point that's actually drawn on screen)
against the viewport-selected camera, instead of the old `.single()` path's entity-origin-to-only-camera
distance. Arguably more correct (culling the point that's actually rendered), and the difference is
sub-metre at normal `max_distance` scales, but it is a real change near the boundary. **Accepted
limitation** (unchanged from before this fix): nameplate anchors remain single-instance (Phase 4
does not extend `WorldLabelRank` to them), so an entity's nameplate still shows in **at most one**
simultaneously-visible split viewport, never duplicated across two.

## Split-screen player HUD labels (P{n})

<!-- b:1511 -->

**Split-screen player HUD labels** (`capabilities/camera.rs`) — the first real consumer of
`PlayerIndex` (see `docs/dev/player-spawn-sites.md`). `split_viewport_player_label_spawn_system` reacts to
`Added<SplitViewportSlot>` (mirroring `nameplate_setup_system`'s `Added<NameplateTag>` idiom, so
no per-frame "does a label already exist" scan is needed) and spawns a standalone (unparented) UI
`Text` node — a corner "P{n}" label — for any split camera whose `CameraTargets` carries a
`PlayerIndex`. The camera entity gets tagged `SplitScreenPlayerLabel` + `LinkedPlayerLabel(Entity)`
pointing at the UI entity. `split_viewport_player_label_update_system` (`.after(
split_screen_viewport_system)` in `lib.rs`'s `.chain()`) keeps the label's `Node.left`/`top`
synced to that camera's live `Camera.viewport` (physical pixels ÷ `window.scale_factor()` →
logical, top-right anchored so it never collides with a room's top-left `room_hint` title) and its
`Visibility` synced to `Camera.is_active` — the ordering guarantees no stale frame across a
`dynamic` split's merge/split transition. Label text reads `PlayerIndex`, not scene entity/spawn
order; label color comes from a fixed `PLAYER_LABEL_COLORS` palette, not from `PrefabDef.material`
— see `docs/20_data_formats.md`'s "Split-screen player HUD labels" section for the designer-facing
version of both notes. The label is a valid standalone UI root because it resolves against the
same full-window `Camera2d` every RON UI label already uses (see the "Adding a new asset
project"/UI conventions elsewhere in this file) — `IsDefaultUiCamera` being commented out on that
camera does not change this in practice, but a future refactor of that setup should re-verify it.
