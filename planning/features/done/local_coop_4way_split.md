# Feature: Local Co-op 4-Way Split-Screen Scene

_Status: Done_
_Planned at: `ac90078` (2026-07-06)_
_Shipped at: `dca09ef` (2026-07-07)_

## What

A new `local_coop_demo` scene (`room6.scene.ron`, portal-linked from `room5`) demonstrating a
static 4-way split screen: four players, each with an independent camera occupying one quadrant
of the window, controlled by four distinct keyboard schemes on one physical keyboard (WASD,
IJKL, Arrow keys, Numpad). This generalizes the split-screen system built in Stages 3-5 from a
hardcoded 2-way split to an N-way grid split, while keeping this scene's actual RON content fixed
at exactly 4 players.

## Why

Local co-op currently caps out at 2 players everywhere — schema, camera spawning, and viewport
math are all written assuming exactly 2. A 4-way split is the natural next showcase for the
split-screen mechanics Stages 3-5 already proved, and building the underlying split math
generically (grid layout driven by player count, not a hardcoded "half the screen") means a
future 3-player room is nearly free instead of requiring its own bespoke system. This is also the
necessary foundation for the (separately tracked, not-yet-drafted) hot join/leave follow-up —
that feature needs a split system that already tolerates a variable player count before it can
layer runtime add/remove on top.

## Research findings (confirmed by reading the current code)

- `SplitOrientation` (`schema/player.rs`) has exactly two variants, `Vertical`/`Horizontal`, and
  `split_screen_viewport_system` (`capabilities/camera.rs`) computes viewport rects with a binary
  `if slot.0 == 0 { .. } else { .. }` — flagged as a latent constraint for N-way splits back in
  the Stage 3/4 architecture reviews, never addressed since neither stage needed more than 2.
- `spawn_players_and_camera`'s `split` branch (`entity_spawner.rs:612-633`) explicitly does
  `.take(2)` on the zipped player/entity iterator, with a comment stating "a hypothetical 3rd+
  player isn't part of this stage's scope." This is the hard cap that needs removing.
- `InputMap::parse_key` (`schema/player.rs`) has no Numpad key parsing at all — only letters,
  top-row digits, function keys, modifiers, and arrow keys. Bevy's `KeyCode` enum already has
  `Numpad0`-`Numpad9` (physical-key variants, unaffected by NumLock state) — this is a pure
  additive whitelist change, no new concept. Spot-check the exact variant names against the
  vendored `bevy_input` crate at implementation time — not independently re-verified against
  source for this plan.
- `SplitViewportSlot(u32)` is already a bare index with no assumption baked into its own type —
  only the *system reading it* assumes binary. Good: no schema/component change needed there,
  just the system's math.
- `PartyOrbitCamera`/dynamic split (Stage 5) are untouched by this feature — this scene is
  static-only, no merge/split-by-distance behavior, so `DynamicSplitConfig` stays `None` for this
  scene exactly like Stages 3/4.

## Approach

### Generalizing the viewport split to N-way (`Grid` orientation)

- New `SplitOrientation::Grid` variant, additive alongside `Vertical`/`Horizontal` (both stay
  exactly as they are today — 2-way only, unchanged behavior, still used by `room3`/`room4`).
- **Slot count is stored authoritatively, not derived from a live query.** Original draft of this
  plan proposed reading the count of `SplitViewportSlot` cameras present each frame — architecture
  review flagged this as the wrong seam: since this feature is explicitly the foundation for a
  future hot join/leave feature, deriving layout from live entity count means any mid-transition
  frame (a camera despawned while `ActiveSplitScreen` is still `Some`) silently reflows the grid,
  and a stale `slot.0 >= cols*rows` degrades to a clamped 1×1 black viewport with no visible error.
  Fix: new resource `ActiveSplitSlotCount(Option<u32>)`, mirroring `DynamicSplitConfig`'s exact
  lifecycle (populated once by `spawn_players_and_camera` at scene load, cleared to `None` on
  `LoadScene`) rather than widening `ActiveSplitScreen` itself (which Stage 3/4/5 code already
  reads/writes as a bare `Option<SplitOrientation>` in many places — adding a parallel resource is
  additive and lower-risk than reshaping an existing one). A future join/leave feature would update
  `ActiveSplitSlotCount` as an explicit write when a player joins/leaves, not an implicit
  derivation.
- `split_screen_viewport_system` gains a `Grid` match arm: reads `ActiveSplitSlotCount` (falls back
  to a no-op if `None`), computes `cols = ceil(sqrt(count))`, `rows = ceil(count / cols)` (for
  `count == 4`: `cols = 2`, `rows = 2` — a clean quadrant grid; the formula is written generically
  but only `count == 4` is authored/tested by this feature — see the explicitly-documented
  `count == 3` dead-quadrant behavior below), then for each camera's `slot.0`: `row = slot.0 /
  cols`, `col = slot.0 % cols`, and computes that cell's physical-pixel rect using the same
  remainder-absorbing pattern `Vertical`/`Horizontal` already use for odd window sizes (last
  row/column absorbs the remainder so all cells sum exactly to the window size).
- `MAX_SPLIT_PLAYERS: u32 = 4` constant (`capabilities/camera.rs`) — a hard ceiling on `Grid`
  mode, both to match this feature's own scope and to avoid degenerate slivers if a scene is
  misconfigured with an absurd player count, and to bound WebGPU render-pass count (4 cameras ×
  4 render passes is the worst case this feature accepts; wasm-perf review must confirm this is
  fine before shipping).
- Two behaviors must be explicitly documented (`docs/20_data_formats.md`), per architecture review,
  since they're easy to get wrong silently: **(a)** a `Grid` scene with `count == 3` leaves one grid
  cell empty (dead/clear-color quadrant) — no special-cased 3-way layout exists; **(b)** a `Grid`
  scene with more than `MAX_SPLIT_PLAYERS` players present spawns the extra players cameraless
  (consistent with today's existing `Vertical`/`Horizontal` behavior when a 3rd player exists in a
  2-way scene — not a new failure mode, just extended to `Grid`'s higher cap).
- Update the two stale doc comments this feature invalidates: `SplitOrientation`'s doc comment
  (`schema/player.rs`) currently asserts only `Vertical`/`Horizontal` are implemented, and
  `split_screen_viewport_system`'s doc comment (`capabilities/camera.rs`) currently assumes at most
  2 cameras.

### Removing the 2-player cap

- `spawn_players_and_camera`'s `split` branch: the `.take(2)` becomes `.take(slot_count)`, where
  `slot_count: u32 = if split.orientation == Grid { (entities.len() as u32).min(MAX_SPLIT_PLAYERS) }
  else { 2 }` (explicit `usize`→`u32` cast, per architecture review) — `Vertical`/`Horizontal` keep
  their exact current 2-way behavior (including whatever happens today if more than 2 players are
  present in a scene not authored for it — not changing that), only `Grid` unlocks N-way. Also
  writes `ActiveSplitSlotCount(Some(slot_count))` for `Grid` (and `None` for `Vertical`/`Horizontal`,
  matching `DynamicSplitConfig`'s existing per-branch clear pattern).
- `Camera.order` assignment stays `i as isize` per slot (0-3 for a 4-way grid) — no clash risk
  since `Grid` scenes never spawn a `PartyOrbitCamera` (static-only, no dynamic branch involved).
- Camera manual-control-off convention (`zoom_speed: 0.0`, `orbit_button: "None"`) applies to all
  4 players' camera configs, same as every prior split stage.

### Input: Numpad key support

- `InputMap::parse_key` gains `"Numpad0"`..`"Numpad9"` → `KeyCode::Numpad0`..`KeyCode::Numpad9`.
  Purely additive to the existing match — no change to any existing key name's behavior.
- RON bindings for the 4 schemes (all mirroring the existing P1/P2 pattern of
  `strafe_left`/`strafe_right` duplicating `left`/`right`):

| | forward | backward | left | right | jump | run |
|---|---|---|---|---|---|---|
| P1 (WASD) | KeyW | KeyS | KeyA | KeyD | Space | ShiftLeft |
| P2 (Arrows) | ArrowUp | ArrowDown | ArrowLeft | ArrowRight | Enter | ShiftRight |
| P3 (IJKL) | KeyI | KeyK | KeyJ | KeyL | KeyU | KeyO |
| P4 (Numpad) | Numpad5 | Numpad2 | Numpad1 | Numpad3 | Numpad0 | Numpad4 |

  P1/P2 bindings match the existing `local_coop_demo` prefabs exactly (no change). P4's bindings
  are exactly what Frank specified (5/2/1/3/0/4 — deliberately the lower half of the numpad, not
  the more common 8/4/6/2 cluster, since 8-and-2-with-5-in-the-middle was called out as
  uncomfortable to play). P3's jump/run (`U`/`O`) are a proposed default, chosen for being
  adjacent to IJKL and not colliding with any other scheme's keys — confirm during
  authoring/playtesting like every prior stage's tentative key choice.

### Playtest findings (2026-07-07) — two fixes made post-implementation

- **Material tint didn't apply to players — real gap, now fixed.** The plan's claim that
  `PrefabDef.material` was "zero new engine capability" was wrong for players specifically:
  `spawn_prefab_instance` (the generic Actor/Prop path) reads `prefab.material` and inserts
  `PendingMaterialOverride`, but players are spawned through a completely separate path
  (`spawn_player_entity_core`) that never read it — and `PlayerConfig` didn't even carry a
  `material` field. Fixed: added `PlayerConfig.material: Option<String>` (`schema/player.rs`),
  forwarded it in `assemble_player_config` (the single shared helper all three player-assembly
  sites already route through — Stage 1's four-site inventory paid off here, one edit fixed all
  three), and `spawn_player_entity_core` now inserts `PendingMaterialOverride` exactly like the
  generic path does. No schema RON change needed — `material:` on a player prefab already parsed
  correctly, it just silently did nothing before this fix.
- **UI relayout — per-quadrant control hints instead of a stacked global block.** Frank's playtest
  feedback: two stacked "controls_hint" lines at the top reads worse than putting each player's
  own control hint at the bottom of their own grid cell, and the scene title should stay the sole
  top label (matching every other room) instead of becoming a 3rd stacked line. Reworked
  `room6.scene.ron`'s `ui:` block: one `room_hint` label at the top (unchanged position/role from
  every other room), plus 4 new labels (`controls_hint_p1`-`p4`) positioned near the bottom edge of
  each 1280x720-baseline quadrant. Same fixed-pixel convention every other Label in this project
  already uses (no Label in this engine is resize-aware — only the 3D camera viewports are), so
  this is a content-only change, no new schema/capability.
- **Floating room-destination labels above every portal (all 6 `local_coop_demo` scenes, not just
  room6).** Frank asked whether this was possible RON-only — it is: `SceneEntityDef.label:
  Option<EntityLabelDef>` already exists (`schema/scene_v2.rs`) as a per-entity floating
  world-space text annotation, independent of the nameplate system, and already wired into the
  composite-prefab spawn branch every portal here uses (`scene_loader.rs`). Added `label: (text:
  "Room N", offset: (0.0, 4.0, 0.0))` to all 10 portal entities across `main`/`room2`-`room6`, one
  edit per portal, zero Rust/schema changes.

### Scene content

- New prefabs `player_p3_grid` (IJKL) and `player_p4_grid` (Numpad), alongside new
  `player_p1_grid`/`player_p2_grid` variants carrying `split: (orientation: Grid)` on the first
  player's camera block (matching Stage 4's precedent of a dedicated prefab pair per split mode
  rather than overriding an existing one, since there's no scene-level camera-field override
  mechanism).
- **Visual distinction — solid-color material tint per player (revised per Frank's direction,
  2026-07-06).** UX review flagged that only two humanoid GLBs (`character_male`,
  `character_female`) are used across every prior room, and repeating them for 4 players means two
  visually-identical pairs. Rather than sourcing a 3rd/4th distinct model, reuse
  `character_male`/`character_female` for all 4 players and give each a distinct solid-color
  material tint: **blue, pink, dark green, red**. This needs **zero new engine capability** — an
  existing, already-shipped mechanism already does exactly this:
  `PrefabDef.material: Option<String>` (a material-catalog key that overrides a spawned model's
  material) is applied by `spawn_prefab_instance` via `PendingMaterialOverride`
  (`material_factory.rs`), and `apply_material_overrides` walks every mesh descendant of the
  spawned GLB once its scene children appear, replacing all of them with the same built material
  handle — precisely a "flat solid-color character" effect, already used elsewhere (e.g.
  `custom_materials`'s `mat_unlit_pink`/`mat_unlit_teal`). `local_coop_demo/assets.ron` currently
  has an empty `materials: {}` block; this feature adds 4 entries:
  ```ron
  "tint_blue":       (kind: Standard((base_color: (r:0.15, g:0.35, b:0.95, a:1.0)))),
  "tint_pink":       (kind: Standard((base_color: (r:0.95, g:0.35, b:0.70, a:1.0)))),
  "tint_dark_green": (kind: Standard((base_color: (r:0.10, g:0.40, b:0.15, a:1.0)))),
  "tint_red":        (kind: Standard((base_color: (r:0.85, g:0.15, b:0.15, a:1.0)))),
  ```
  (`Standard`, not `Custom`/unlit, so the tinted characters still respond to each room's existing
  lighting instead of rendering flat — matches the project's stylized-but-lit look elsewhere.)
  Each of the 4 new player prefabs sets `material: "tint_<color>"` alongside its existing `model:`
  field (`character_male` or `character_female`) — this is the only per-prefab change needed for
  visual distinction; no Rust code, no new schema field.
- New scene `room6.scene.ron`: `ground_room6` (new color identity, continuing each room's
  one-color-per-room convention), `max_view_box` sized the same way `room3`/`room4`/`room5` were
  (reuse the established box unless playtesting says otherwise), 4 player entities in prefab
  order (P1 first, since `split`/`party` config is only read from the first player), a portal
  back to `room5`.
- **Control-scheme hint label needs a second line, not a longer one.** UX review: the shipped
  `controls_hint` label is already at its known-good ceiling (59 chars / 900px, for P1+P2 only) —
  4 schemes cannot fit on one line, and this project has hit label-overflow bugs twice before
  (Stage 2, and flagged again at Stage 4). Fix: stack a second label instead of widening the first
  — `controls_hint` (P1/P2, `position: (20.0, 20.0)`, unchanged text), new `controls_hint_2` (P3/P4,
  `position: (20.0, 64.0)`, proposed text `"P3: IJKL + U + O | P4: Numpad 5/2/1/3 + 0 + 4"` — ~44
  chars, comfortably under budget), `room_hint` (portal text) moves down to `position: (20.0,
  108.0)`. Explicitly verify no wrap/overlap in-browser during the playtest, same as every prior
  stage's label change.

### Not in scope for this feature

- Dynamic merge/split for 4 players (hysteresis-by-distance) — static-only, matching the agreed
  scoping.
- Runtime player join/leave — tracked separately as a follow-up backlog item, not drafted yet.
- 3-way split (an actual authored/tested scene) — the `Grid` formula is written to generalize,
  but only `count == 4` ships and is verified here.
- Gamepad bindings for P3/P4 — this feature is keyboard-only for all 4 players (unlike Stage 1's
  P1/P2 gamepad option), since 4 simultaneous gamepads is a bigger ask than this feature's scope.

## Tasks

- [x] `SplitOrientation::Grid` variant + `ActiveSplitSlotCount(Option<u32>)` resource (mirroring
      `DynamicSplitConfig`'s populate-at-load/clear-on-`LoadScene` lifecycle) + `Grid` match arm in
      `split_screen_viewport_system` reading it (row/col math, remainder-absorbing cell sizing) —
      NOT deriving count from a live camera query (see architecture review note in Approach)
- [x] `MAX_SPLIT_PLAYERS` constant; `spawn_players_and_camera`'s `split` branch takes
      `slot_count` instead of a hardcoded `.take(2)` (`Vertical`/`Horizontal` keep taking exactly 2),
      writes `ActiveSplitSlotCount` accordingly
- [x] Update stale doc comments: `SplitOrientation` (`schema/player.rs`) and
      `split_screen_viewport_system` (`capabilities/camera.rs`) both currently assert only 2-way
      splits are implemented
- [x] `InputMap::parse_key`: add `Numpad0`-`Numpad9`
- [x] `local_coop_demo/assets.ron`: 4 new `materials` entries (`tint_blue`, `tint_pink`,
      `tint_dark_green`, `tint_red`, all `MaterialKind::Standard`)
- [x] `local_coop_demo`: `player_p1_grid`/`player_p2_grid`/`player_p3_grid`/`player_p4_grid`
      prefabs (reusing `character_male`/`character_female` models, each with a distinct
      `material: "tint_<color>"` override), `ground_room6`, `room6.scene.ron` — final UI layout
      (post-playtest) is one top title label + 4 per-quadrant control-hint labels, not the
      originally-planned stacked pair; portal wiring both directions
- [x] Tests: `Grid` viewport math for `count == 4` (even + odd window dimensions, non-overlap,
      sums-to-full-size); `count == 3` leaves the documented dead quadrant (no panic); a `Grid`
      scene with 5 players spawns the 5th cameraless without panicking; `spawn_players_and_camera`
      spawns exactly 4 `SplitViewportSlot`s + `ActiveSplitSlotCount(Some(4))` for a `Grid` scene;
      `Vertical`/`Horizontal` scenes unaffected (regression, `ActiveSplitSlotCount` stays `None`);
      Numpad key parsing. 38/38 pass; full `ironhold_core` suite passes.
- [x] Docs: `docs/20_data_formats.md` (`Grid` variant, `MAX_SPLIT_PLAYERS`, `ActiveSplitSlotCount`,
      Numpad key names, the `count == 3`/`>4 players` documented behaviors, quadrant-order-equals-
      entity-order), `crates/ironhold_core/src/CLAUDE.md` (also documents the `PlayerConfig.material`
      playtest-fix gap for future readers)
- [x] Schema/CLI check: `cargo check -p ironhold_cli` — clean
- [x] Full review gate: alignment-reviewer (ALIGNED), system-architect (ALIGNED, 2 minor — the
      Grid+dynamic-combo warning was added), wasm-perf-reviewer (OK, no regression),
      ux-gamedesigner-reviewer (ALIGNED, found the room6 room_hint color-name bug, fixed)
- [ ] Register `room6` per `CLAUDE.md`'s "Adding a new asset project" steps — **deferred**: the
      screenshot baseline (`test_web.py --project local_coop_demo --update-baselines`) was not
      generated for `room6` due to this session's build-time/disk constraints. `main`/`room2`-`5`
      already have baselines; `room6`'s is missing until a future baseline refresh. Not a blocker
      per Frank's explicit call — revisit next time baselines are regenerated for this project.
- [x] Playtest checklist explicitly includes: confirm no label wrap/overlap in-browser; note the
      IJKL-scheme's keyboard-position-to-screen-quadrant mismatch (IJKL sits center-right on the
      keyboard but P3 renders bottom-left) as an accepted, non-blocking quirk — confirmed by Frank
- [x] WASM dev + release build, playtest checklist, Frank confirmation — dev (111 MB) and release
      (59 MB) both confirmed working in-browser, no console errors

## Open questions

None outstanding — resolved during system-architect and ux-gamedesigner-reviewer plan review
(2026-07-06):
- **Slot count derivation** — stored in a new `ActiveSplitSlotCount` resource, not derived live
  from a query (architecture review).
- **P3's jump/run keys** — `U`/`O`, confirmed as a good, discoverable, collision-free choice (UX
  review): both sit directly above `I`/`L`, keeping the whole P3 scheme one contiguous cluster.
- **Visual distinction across 4 players** — resolved via a per-prefab solid-color material tint
  (blue/pink/dark green/red) using the existing `PrefabDef.material` override mechanism, per
  Frank's direction — supersedes the plan's earlier third-GLB-model proposal (UX review + Frank).
- **Quadrant assignment order** — P1 top-left, P2 top-right, P3 bottom-left, P4 bottom-right
  (row-major by `slot.0`) confirmed as the intuitive default (UX review).

## Acceptance criteria

- Given `room6.scene.ron` loads with 4 players present and `split: (orientation: Grid)` on the
  first player, when the scene is ready, then 4 independent cameras spawn, each rendering to its
  own quadrant of the window with no visual bleed between quadrants, and `ActiveSplitSlotCount`
  holds `Some(4)`.
- Given the browser window is resized (including to odd width/height), when the next frame
  renders, then all 4 quadrants resize to stay correctly proportioned with no gap or overlap.
- Given all 4 players are present on one keyboard, when each player uses their assigned scheme
  (WASD / Arrows / IJKL / Numpad) to move, jump, and run, then only that player's character
  responds — no cross-talk between schemes.
- Given `room3`/`room4` (existing `Vertical`/`Horizontal` 2-way scenes), when this feature ships,
  then their behavior is byte-for-byte unchanged (regression tests pass), and
  `ActiveSplitSlotCount` stays `None` for both.
- Given all 4 players, when viewed on screen, then each renders as a distinct solid color (blue,
  pink, dark green, red) regardless of which of the two base models (`character_male`/
  `character_female`) it uses underneath.
