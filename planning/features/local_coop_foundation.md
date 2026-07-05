# Feature: Local Co-op Foundation (2-player, shared camera, view-box clamp)

_Status: In Progress (Stage 1 Done, Stage 2 Done, Stage 3 Done, Stages 4–5 Queued)_
_Planned at: `c624c7b` (2026-07-03)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| Stage 1 | Two-player schema + shared framing camera + view-box clamp (this doc) | Done | `da81799` (2026-07-05) |
| Stage 2 | Portal/teleport action (moves both players together) | Done | `8181ccd` (2026-07-05) |
| Stage 3 | Vertical split-screen scene | Done | `b59a3e7` (2026-07-05) |
| Stage 4 | Horizontal split-screen scene | Queued | — |
| Stage 5 | Dynamic split-screen scene (viewport follows player positions) | Queued | — |

A fifth split style (diagonal) was scoped out during design discussion — Bevy's `Camera.viewport`
is rectangle-only, so a true diagonal cut needs a custom stencil/shader mask, which is untested
in this engine's WASM/WebGL2 target. Not in this project's plan.

## What

A new example project (proposed name: `local_coop_demo`) demonstrating same-machine local
multiplayer: two players on one keyboard (+ optional gamepad), moving through a sequence of
scenes linked by portals, each scene showcasing a different screen-sharing configuration. This
stage (Stage 1) builds the foundation everything else depends on: a scene that holds two player
entities instead of one, a single shared camera that frames both players and zooms based on how
far apart they are, and a hard clamp that stops either player from wandering outside the camera's
maximum framed area. No viewport splitting yet — this stage is single-camera, single-viewport, to
prove the two-player core before touching rendering.

This is **local co-op on one machine**, not networked play — it does not depend on and is
unrelated to the Beta 0.6 "LAN Co-op" networking milestone (`planning/features/networking_multiplayer.md`),
which requires deterministic ticks and state replication. No networking, determinism, or replay
concerns apply here.

## Why

Every later stage (portal, vertical/horizontal/dynamic split) needs more than one local player to
exist in a scene and a camera system that reacts to both of them. Landing that core first, as its
own playable single-viewport scene, gives a stable base for the viewport-splitting work in Stages
3-5 and de-risks it early. It also happens to unblock two long-standing icebox items:
- **"Camera/input configuration → scene layer"** — camera/input are currently `PrefabDef`
  singleton fields; this feature forces them to become instance-level so two players can each
  have their own input map, which is the direction that icebox item already wanted.
- **"Gamepad / controller input"** — needed here for the "keyboard + gamepad" input scheme Frank
  chose; this feature is the first real consumer of it rather than a standalone icebox add.

## Approach

### Schema: more than one player per scene

Scene player detection already exists via a tag, not a dedicated field —
`scene_loader.rs:178` checks `prefab.components.tags.contains("player")` per scene entity. There
are **four** sites that assemble or consume a player's spawn config today, not one, and each
currently assumes exactly one player:

1. **GLB player collector** — declared `player_config: Option<PlayerConfig>` at
   `scene_loader.rs:164`, assembled at `scene_loader.rs:626`. Unconditionally overwritten each time
   a `tags: ["player"]` GLB prefab is found, so a second GLB player today silently discards the
   first.
2. **Primitive player collector + inline spawn** — declared `primitive_player: Option<(tuple)>` at
   `scene_loader.rs:166`, set at `:244`, spawned inline at `:699`–`:862`. This path builds its own
   `CharacterController` + `OrbitCamera` directly and does **not** go through `PlayerConfig` at
   all — a capsule/primitive-shaped player is a fully separate spawn path from the GLB one.
3. **Dynamic / character-select spawn** — `action_executor.rs:148`–`173` (assembled at `:155`),
   for `Action::Spawn` on a `tags: ["player"]` prefab (the character-select flow). One player per
   action, so it doesn't break a *count*, but it duplicates the same literal `PlayerConfig`
   assembly as site 1.
4. **Shared GLB spawn function** — `entity_spawner.rs:491` `spawn_player_entity(...)`, consuming
   either `QueuedSpawn.player_config` (`scene_manager/mod.rs:173`) directly, or
   `PendingPlayerConfig` (**defined** at `scene_manager/mod.rs:346`, **constructed** at
   `scene_loader.rs:868`) when the terrain-async path defers the spawn by a frame.

Changes:
- **Scope decision: this stage supports two GLB-based players, not primitive/capsule players.**
  Site 2 (primitive inline spawn) is left single-player-only for now — extending it to N players
  would double this stage's surface area for no payoff, since `local_coop_demo` can reuse existing
  shared character GLBs (e.g. `character_male`/`character_female` from `3rd_person_game_demo`)
  instead of authoring new capsule assets. Revisit only if a future project specifically wants
  multi-player primitive shapes.
- `player_config: Option<PlayerConfig>` → `Vec<PlayerConfig>` at site 1, and `PendingPlayerConfig`
  (site 4) becomes `PendingPlayerConfig(Vec<PlayerConfig>)` — `spawn_player_entity` spawns one
  character controller + camera rig per entry instead of assuming exactly one.
- Site 3 (`action_executor.rs:155`) also needs a `player_index` value — dynamically-spawned
  players default to `player_index: 0` unless the acting rule specifies otherwise.
- Extract a shared `assemble_player_config(prefab, entity_def, translation) -> PlayerConfig`
  helper (beside `tag_spawned_entity`'s pattern) and route sites 1 and 3 through it, instead of
  adding `player_index` to two separate hand-written literals. Matches this codebase's existing
  "one source of truth per spawn concern" convention (`tag_spawned_entity`, `attach_prefab_features`).
- New typed field `player_index: u32` (default `0`) on `PrefabDef`, read alongside the existing
  `player` tag. Two player prefabs (e.g. `player_p1`, `player_p2`) each carry `tags: ["player"]`
  plus a distinct `player_index`, each with its own `components.inputs` (`InputMap`) — this is
  what lets input routing and camera targeting tell the two players apart without parsing prefab
  keys as strings.
- `PlayerConfig` gains `player_index: u32`, threaded through `assemble_player_config`, and
  forwarded onto the spawned entity as a queryable `PlayerIndex(u32)` component
  (`capabilities/player.rs`). **Not consumed by any system yet** — Stage 1's own input routing
  keys off `gamepad_index` and camera targeting keys off scene `entities` order, not this value.
  Reserved for a future consumer (e.g. per-player nameplate/HUD labeling — see the split-out
  backlog item).

### Input: per-player routing + gamepad

- `InputMap` (`schema/player.rs`) needs a `gamepad_index: Option<usize>` field so a player prefab
  can optionally bind to a specific connected gamepad instead of keyboard.
- `input_translator_system` (`runtime/input.rs:31`) currently emits `InputActionMessage` to every
  `CharacterController` from one shared keyboard read. It needs to read each controller's own
  `InputMap` (already the per-entity shape) and, when `gamepad_index` is set, read Bevy's
  `Gamepad`/`ButtonInput<GamepadButton>`/`Axis<GamepadAxis>` resources for that index instead of
  the keyboard. This is the "Gamepad / controller input" icebox item, scoped down to exactly what
  two-player local co-op needs (no rebinding UI, no axis-curve tuning).
- Default RON authoring for this demo: player 1 = WASD + Space, player 2 = arrow keys + Enter,
  with `gamepad_index: 0` / `gamepad_index: 1` as an optional override on either.

### Camera: shared framing + zoom-by-distance

`OrbitCamera` (`capabilities/camera.rs:18`) orbits a single `target: Entity` and already supports
scroll-driven zoom clamped to `[min_radius, max_radius]`. For this stage we need a camera that
tracks the midpoint of *two* players and auto-adjusts distance based on how far apart they are,
without breaking existing single-player scenes.

- New component `PartyOrbitCamera` (sibling to `OrbitCamera`, not a replacement): same
  orbit/pitch/yaw fields, but `targets: Vec<Entity>` instead of a single `target`, plus
  `zoom_margin: f32` (extra distance added beyond raw player separation).
- New system `party_camera_follow_system`: each frame, computes the midpoint and max pairwise
  distance across `targets`, sets `look_at` to the midpoint, and sets `radius = (max_distance +
  zoom_margin).clamp(min_radius, max_radius)`. **Correction post-implementation:** runs in
  `Update`, chained alongside `camera_orbit_system` — not `FixedUpdate` as originally planned
  here. `camera_orbit_system` itself already runs in `Update` (treated as render cadence, not
  physics, per `lib.rs`'s existing "Visual/animation pipeline stays in Update" grouping); this
  doc's original claim about its schedule was wrong, and the implementation correctly matches
  the real sibling system instead of the mistaken plan.
- `CameraConfig` (RON) gains an optional `party: PartyZoomDef { zoom_margin: f32, allow_manual_zoom:
  bool }` block (`allow_manual_zoom` defaults `false`) — whether scroll-zoom still nudges the
  derived radius is an authored choice, not a hardcoded Rust behavior.
- **`party` presence is the sole, explicit switch for `PartyOrbitCamera`** — not an inferred "2+
  players ⇒ party camera" rule. If a scene has 2+ players and no `party` block, the loader falls
  back to the first player's own `OrbitCamera` **and logs a warning** naming the scene, so a
  designer who forgot to author `party` gets a visible signal instead of two silently-competing
  camera rigs with no RON-visible symptom.

### View-box clamp

Simplest robust option, matching Frank's spec ("players can't move out of max view box") as a
hard position clamp rather than derived camera-frustum math:
- New optional field `max_view_box: Option<(f32, f32, f32, f32)>` (`min_x, min_z, max_x, max_z`)
  on `GameSceneV2`.
- New small system `player_view_box_clamp_system`, `FixedUpdate`, `.after(player_movement_system)`:
  for every `CharacterController` entity, clamps `translation.x`/`translation.z` into the box (Y
  untouched), **and zeroes the corresponding `Velocity.linvel` axis whenever that axis was
  clamped**. Both players spawn as `RigidBody::Dynamic` with a `Velocity` component
  (`player.rs:127`'s ground sphere-cast reads it too); without the velocity zero, Rapier keeps
  re-integrating the outward velocity every tick and the player visibly fights the clamp at the
  edge instead of stopping cleanly.

### Not in scope for this stage

- Portal/teleport action — Stage 2.
- Any viewport splitting, `Camera.viewport` usage, or `RenderTarget` — Stages 3-5.
- Gamepad rebinding UI, axis dead-zone tuning — out of scope entirely for this demo, default
  Bevy gamepad behavior is enough.
- Extending the primitive/capsule inline player path (site 2 above) to support multiple players —
  `local_coop_demo` uses GLB player prefabs only; primitive players remain single-player-only.

## Tasks
- [x] Extract `assemble_player_config(prefab, entity_def, translation) -> PlayerConfig` helper;
      route sites 1 (`scene_loader.rs:626`) and 3 (`action_executor.rs:155`) through it
- [x] `PrefabDef.player_index: u32` (`#[serde(default)]`) + `PlayerConfig.player_index`, populated
      by `assemble_player_config`
- [x] `player_config` → `Vec<PlayerConfig>` at site 1; `PendingPlayerConfig(Vec<PlayerConfig>)` at
      site 4 (`mod.rs:346` definition, `scene_loader.rs:868` construction); `spawn_player_entity`
      (`entity_spawner.rs:491`) loops over the vec
- [x] `InputMap.gamepad_index: Option<usize>` + gamepad read path in `input_translator_system`;
      verify Bevy gamepad input resources are available and behave correctly in the WASM build
      (browser gamepad API support varies) before relying on it for the demo
- [x] `PartyOrbitCamera` component + `party_camera_follow_system` + `CameraConfig.party:
      PartyZoomDef { zoom_margin, allow_manual_zoom }` RON field; warn-and-fallback when 2+
      players exist without a `party` block
- [x] `GameSceneV2.max_view_box` + `player_view_box_clamp_system` (position clamp + velocity zero
      on the clamped axis)
- [x] `local_coop_demo` project: `project.ron`, one scene, two player prefabs (`player_p1` on the
      `character_male` GLB, `player_p2` on `character_female`, both already in the shared asset
      library), `assets.ron`
- [x] Register the new project per `CLAUDE.md`'s "Adding a new asset project": add to
      `test_web.py`'s `PROJECTS` list, generate the baseline screenshot, add an `index.html` card
      (screenshot still outstanding — needs a GPU-capable machine, see Stage 1 play-test notes)
- [x] Tests: two players spawn from one scene; camera radius tracks separation within clamp
      bounds; view-box clamp stops position drift (and zeroes velocity) past the boundary on both
      axes; warning fires when 2+ players exist without `party`
- [x] Docs: `docs/20_data_formats.md` (new fields), `crates/ironhold_core/src/CLAUDE.md`
      (party camera + gamepad routing notes, and the four-site player-spawn inventory above so
      the next person doesn't have to re-derive it)
- [x] Schema/CLI check: `cargo check -p ironhold_cli` + spot-check `query scenes` on
      `local_coop_demo` once the new fields land

## Open questions
None outstanding — resolved 2026-07-04:
- **P1/P2 nameplate/UI distinction** — split out into its own feature, not part of this stage. See
  the new backlog entry under "Local Co-op Split-Screen Demo" (Queued).
- **Keyboard bindings** — as proposed: player 1 = WASD + Space, player 2 = arrow keys + Enter.
- **Character models** — `character_male` (player 1) and `character_female` (player 2), both
  already in the shared asset library; no new art needed.

## Acceptance criteria
- Given a scene with two player-tagged entities, when the scene loads, then both spawn with
  independent controllers, inputs, and (for gamepad-bound players) respond only to their assigned
  gamepad index.
- Given both players standing close together, when they move apart, then the shared camera radius
  increases smoothly up to `max_radius`, and decreases as they move back together, without ever
  exceeding the configured min/max bounds.
- Given `max_view_box` is set on the scene, when either player attempts to move past its edges,
  then that player's position is clamped at the boundary, that axis's velocity is zeroed so the
  player doesn't jitter against the edge, and movement along the other axis is unaffected.
- Given a scene with 2+ players and no `party` block authored, when the scene loads, then only
  one `OrbitCamera` is spawned (not two competing rigs) and a warning naming the scene is logged.

---

## Stage 2 — Portal/Teleport Action

### What

A portal in one scene, entered by either player, moves **both** players to a linked destination
scene. Adds a second scene (`scenes/room2.scene.ron`) to `local_coop_demo`, linked from
`scenes/main.scene.ron` by a portal, with a return portal back.

### Why

Confirms the "if one player touches the portal, everyone goes" requirement — and the
portal-linked-scene-sequence structure the whole project is built around — before Stages 3-5
build split-screen rendering on top of it. De-risking this early is cheap: it turns out to need
zero new engine code (see Research below), so proving it now costs almost nothing.

### Research findings (confirmed before writing this plan — see `crates/ironhold_core/src/capabilities/trigger_zone.rs`, `action_executor.rs`, `scene_loader.rs`)

- `trigger_zone_system` queries `With<CharacterController>` generically — it already fires for
  **any** player entity crossing a `TriggerZone` sensor, not a singular assumed player. Stage 1's
  2-player scenes need no change here.
- `Action::LoadScene` already tears down **all** `LevelEntity` entities (both players included)
  and respawns fresh from the destination scene's `entities:` list, via the `Vec<PlayerConfig>`
  path Stage 1 shipped. "Both players teleport" is automatic — the whole world reloads regardless
  of which player touched the trigger; "teleport" in this engine's model is a full scene reload,
  not a position change.
- Established RON pattern already exists in `particles_demo`: a composite `Primitive` prefab with
  `trigger_zone: (radius: ...)` whose spawn id is referenced by a `rules.ron` entry
  (`on: "entity.entered:{id}", do_actions: [LoadScene("scenes/...ron")]`). Stage 2 reuses this
  pattern exactly.
- **Known, accepted quirk**: if both players cross the same trigger zone in the same physics tick,
  two `entity.entered:{id}` events fire (one per player), so the matching rule's `LoadScene` runs
  twice in one frame. Harmless — the second call just re-triggers an already-in-flight scene load
  (same handle insert, same state transition) — but worth a RON comment so a future reader doesn't
  mistake it for a bug. Not worth debouncing for a demo project.

**Conclusion: no new schema, no new `Action` variant, no new Rust code.** This stage is purely a
RON-authoring addition to `local_coop_demo` on top of existing, already-multi-player-safe engine
mechanics.

### Approach

- New prefab `"portal_to_room2"` (and return counterpart `"portal_to_room1"` in `room2`) —
  `kind: Primitive`, `trigger_zone: (radius: 1.5)`, simple decorative geometry (e.g. two short
  pillars flanking a gap, distinct accent color) so it reads visually as a portal, following
  `particles_demo`'s pattern.
- New scene `scenes/room2.scene.ron` — same shape as `scenes/main.scene.ron` (ground + two player
  entities in the same order + `max_view_box`), so Stage 1's shared-camera/view-box mechanics
  continue to apply unchanged in the destination scene. Deliberately **not** a split-screen scene
  yet — that's Stages 3-5; Stage 2 only proves the portal mechanic in isolation.
- `logic/rules.ron` gains two entries: `entity.entered:{main_portal_id}` →
  `LoadScene("scenes/room2.scene.ron")`, and `entity.entered:{room2_portal_id}` →
  `LoadScene("scenes/main.scene.ron")`.
- `LoadScene` is a full reload, so no per-player runtime state carries across the portal (nothing
  exists yet beyond position/velocity, so nothing is lost). Flag as a future concern only if a
  later stage needs to preserve state (e.g. score) across a portal — out of scope here.

### Tasks
- [x] Add `portal_to_room2` / `portal_to_room1` prefabs to `local_coop_demo/prefabs/prefabs.ron`
      (plus a `ground_room2` variant so the destination is visually distinct at a glance)
- [x] Add `scenes/room2.scene.ron` (ground + two players + `max_view_box`, matching
      `main.scene.ron`'s shape; cooler lighting tone as an extra visual "you teleported" cue)
- [x] Wire both portal directions in `logic/rules.ron`; commented the same-tick double-fire quirk
- [x] RON validate (`ironhold_cli validate` — 7 files valid) + asset checker (511 refs, 0 missing)
- [x] Integration test coverage (`local_coop_tests.rs`, 3 new tests): fires for a single player,
      fires twice when both players enter the same tick, ignores non-player entities
- [x] `room2` picked up automatically by `test_web.py`'s `discover_scenes()` (globs
      `scenes/*.scene.ron`) — no manual registration needed, screenshot baseline still pending a
      GPU-capable run (same outstanding item as Stage 1's `main` baseline)
- [x] Play-test checklist: walk player 1 through the portal alone → both players land in room2;
      walk back through the return portal → both land in room1 at their original spawn positions.
      Confirmed by Frank; also surfaced and fixed a real UI label-overlap bug (see commit
      `8181ccd`) not anticipated by the plan — the fixed-size `Label` box wraps/overflows
      instead of clipping, and stacking two of them without accounting for that overlapped.
- [x] Re-confirmed "no Rust changes" — implementation only touched `assets/projects/local_coop_demo/`
      RON files, a new test file, and this doc; `cargo check -p ironhold_cli` clean. No Rust/schema
      change means no WASM rebuild either — the Stage 1 release binary already committed
      (58 MB) serves the new RON at runtime unchanged, so alignment/architecture/wasm-perf review
      triggers genuinely don't apply here.

### Open questions
- Should the portal require *both* players to be within some radius before firing (a
  "wait for your partner" mechanic), or is "first player through triggers it for everyone" (the
  spec's literal wording, and what the engine already does for free) the intended feel? Defaulting
  to the latter — it's what was asked for and costs nothing to build.
- Visual treatment for the portal — reuse `particles_demo`'s pillar-pair primitive look, or
  something more distinct? Low-stakes, decide during authoring.

### Acceptance criteria
- Given both players in `scenes/main.scene.ron`, when either player enters the portal's trigger
  zone, then the scene reloads to `scenes/room2.scene.ron` and both players spawn there — not just
  the one who entered.
- Given both players in `scenes/room2.scene.ron`, when either enters the return portal, then both
  players are back in `scenes/main.scene.ron`.
- No regression to Stage 1 behavior (shared camera, view-box clamp) in either scene.

---

## Stage 3 — Vertical Split-Screen Scene

### What

Replace the single shared `PartyOrbitCamera` (Stage 1's no-split camera) with two independent
cameras, each rendering to half the screen (left/right) and following just one player. This is
the first real viewport-splitting work in the project — everything through Stage 2 was schema and
RON, this stage touches actual multi-camera rendering.

### Why

Confirms the Bevy multi-camera-viewport mechanics actually work correctly in this engine and its
WASM/WebGPU target before Stage 4 (horizontal) and Stage 5 (dynamic) reuse the same underlying
mechanism with a different split calculation. Landing the harder, novel part (viewport math,
window-resize correctness, shared-input handling) once here means Stages 4-5 are mostly "swap the
split-rect formula."

### Research findings (confirmed before writing this plan — Explore + system-architect review)

- Zero existing `Viewport`/`RenderTarget` usage anywhere in `ironhold_core` — greenfield.
- Window size is already dynamically queryable on both native and WASM (no fixed-size assumption
  anywhere) — viewport rects can be recomputed every frame cheaply rather than needing to hook
  resize events specifically.
- `camera_orbit_system`'s query has no `.single()` — it already supports N independent
  `OrbitCamera` entities tracking different targets. Zero changes needed there for "two cameras,
  two players."
- **Real problem needing a design fix, not just documentation**: `camera_orbit_system` (and
  `party_camera_follow_system`, same shape) reads `mouse_wheel_events`/`mouse_motion_events`
  **once per system call**, applying the identical computed delta to every `OrbitCamera` in its
  loop. Two split cameras both mouse-orbit/scroll-zoom enabled would rotate/zoom **together** on
  one shared mouse — visibly wrong for split-screen. Fix: disable manual camera control entirely
  for split-screen player cameras (fixed-angle auto-follow only — the convention most split-screen
  console co-op games use) via two RON-authorable knobs, no new concepts needed:
  `zoom_speed: 0.0` (already a field — scroll × 0 has no effect) and a new `"None"` arm added to
  `parse_orbit_button` (currently `"Left"`/`"Right"`/`"Either"`; an unrecognized string
  warns-and-defaults to `"Either"`, the opposite of what's wanted here). system-architect confirmed
  this is the right tool — the coupling is in the global input-event stream, not the component, so
  per-camera input masking would be solving it at the wrong layer.
- Four **other** systems query `With<Camera3d>` and will see 2+ matches once split-screen ships:
  `world_label_screen_pos_system` (`lib.rs:501`), `nameplate.rs:212`, `particle_renderer.rs:303`,
  `targeting.rs:122`. All four already degrade gracefully (`.single()` returning a `Result` +
  `else { return }`, or `.iter().find(...)`) — none panic, they silently no-op or arbitrarily pick
  one camera. `local_coop_demo` doesn't use world labels, nameplates, particles, or
  click-targeting, so none of these degradations are visible in this demo. Documented as known
  limitations (matching Stage 1's `CameraShake`/`PartyOrbitCamera` precedent), not fixed now — but
  unlike that limitation, none of these four sites carry an in-code comment marking the
  assumption, so also logging them to `claude_suggestions.md` for whoever hits them next.
- **`CameraShake` behavior actually changes here, not just "stays broken"**: split-screen spawns
  two *real* `OrbitCamera` entities (not `PartyOrbitCamera`), so `Action::CameraShake`
  (`With<OrbitCamera>`) will fire on **both** cameras once this ships — a real behavior change from
  Stage 1's "shake no-ops with the party camera" story. Documenting this as an intentional
  consequence of using real `OrbitCamera`s for split-screen, not an accident to fix later.
- **Resolved without a runtime spike, by reading Bevy's actual source**: the one
  system-architect-flagged unverified behavior — does a second camera's clear wipe the first
  camera's already-rendered viewport half? — is answered definitively by
  `bevy_render::texture::texture_attachment::ColorAttachment` (`bevy_render-0.18.0/src/texture/texture_attachment.rs:12-32`).
  Cameras targeting the same window with matching HDR/MSAA settings share one cached texture
  (`bevy_render-0.18.0/src/view/mod.rs:1104-1105`'s `textures.entry((camera.target, texture_usage,
  hdr, msaa))`), and that shared `ColorAttachment` carries an `is_first_call: Arc<AtomicBool>`
  that `get_attachment()` flips to `false` on first use (`texture_attachment.rs:40-49`) — so
  **only the first camera to render each frame actually clears (`LoadOp::Clear`); every later
  camera targeting the same texture gets `LoadOp::Load` automatically, regardless of its own
  `ClearColorConfig`.** Combined with each camera's `viewport` restricting where it draws, this is
  exactly the correct split-screen behavior with zero extra code — no `ClearColorConfig::None`
  workaround needed on the second camera, no spike required. No open risk here anymore.

### Approach

- New RON type `SplitScreenDef { orientation: SplitOrientation }`, with `SplitOrientation::Vertical`
  the only variant Stage 3 implements. Introducing the enum now (rather than a Stage-3-only ad hoc
  field) is deliberate: Stage 4 adds `Horizontal` and Stage 5 adds `Dynamic` as new variants of
  this *same* enum, and both are concretely planned (not speculative), so the shape is predictable
  enough to get right the first time.
- `CameraConfig.split: Option<SplitScreenDef>` — authored on the FIRST player only, same convention
  as `party`. **`split` and `party` are mutually exclusive on player 0's `CameraConfig`** — if a
  designer sets both by mistake, log a warning and let `split` win (more specific/newer setting),
  rather than silently picking one with no signal.
- When `split` is set (2+ players): `spawn_players_and_camera` gains a third branch — spawn TWO
  real per-player cameras (reusing the existing `spawn_orbit_camera_for_player` path once per
  player, **not** `PartyOrbitCamera`), each tagged with a new marker `SplitViewportSlot(u32)`
  (which half it owns: `0` = left, `1` = right).
- **Orientation itself is NOT stored on `SplitViewportSlot`** — kept in a new resource
  `ActiveSplitScreen(Option<SplitOrientation>)`, mirroring `ActiveViewBox`/`LoadedTargetIndicator`'s
  exact pattern (populated on scene load, cleared on `LoadScene`). Keeping split-screen state off
  `OrbitCamera` itself means the planned `camera_modes` unification (`OrbitCamera` + `FlyCamera` →
  `ActiveCameraMode`) doesn't have to untangle it later — split-screen becomes a fourth thing that
  refactor eventually needs to account for, but doesn't gain a fourth *coupling point* to unwind.
- New system `split_screen_viewport_system`: reads `ActiveSplitScreen` + the primary window's size,
  and for each `SplitViewportSlot` camera, sets `Camera.viewport` to its half — recomputed every
  frame (cheap: 2 cameras, simple arithmetic) rather than hooking resize events specifically, so
  it's correct immediately after any resize with no missed-event risk. **Must convert logical
  window size to physical pixels via `window.scale_factor()`** — `Viewport`'s fields are
  physical-pixel `UVec2`, not logical; getting this wrong means correct-looking layout on
  non-HiDPI displays and wrong-sized/positioned splits on HiDPI ones.
- Disable manual camera control for split-screen player cameras in RON: `zoom_speed: 0.0` and
  `orbit_button: "None"` on both players' `camera` blocks in the split scene. Each camera then only
  ever sits at its configured fixed offset/angle behind its own player.
- UI (`Camera2d`, order 1000, `ClearColorConfig::None`) stays completely untouched — it already
  renders full-screen across both viewport halves with no coupling to either 3D camera's
  `viewport`.
- New scene `scenes/room3.scene.ron`, reachable via a new portal from `room2` (with a return
  portal back), carrying `split: (orientation: Vertical)` on player 1's `camera` block. `room1`
  and `room2` stay exactly as shipped in Stages 1-2 — each room continues to prove exactly one new
  mechanic in isolation.

### Tasks
- [x] Verify no clear-wipe issue between viewport halves — resolved by reading Bevy's
      `ColorAttachment` source (see Research findings above), no runtime spike needed
- [x] `SplitOrientation` enum (`Vertical` only for now) + `SplitScreenDef { orientation }` +
      `CameraConfig.split: Option<SplitScreenDef>`; warn-and-`split`-wins if `party` is also set
- [x] `SplitViewportSlot(u32)` component + `ActiveSplitScreen(Option<SplitOrientation>)` resource
      (populated by `spawn_players_and_camera`, not `spawn_scene_v2` as originally planned — see
      that function's own decision of party vs. split vs. fallback; cleared on `LoadScene`)
- [x] `spawn_players_and_camera`: third branch — when `split` is set, spawn two per-player cameras
      and tag each with `SplitViewportSlot`
- [x] `split_screen_viewport_system`: recompute `Camera.viewport` every frame from
      `Window::physical_size()` (already physical pixels — simpler than the planned
      `width()`/`height()` × `scale_factor()` multiplication) + `ActiveSplitScreen`'s orientation
- [x] `parse_orbit_button`: add `"None"` arm → `(false, false)`, no warning
- [x] `local_coop_demo`: new `scenes/room3.scene.ron` (vertical split, `zoom_speed: 0.0` +
      `orbit_button: "None"` on both player cameras via new `player_p1_split`/`player_p2_split`
      prefabs), portal `room2` → `room3` (new `portal_to_room3` prefab) + return portal (reuses
      the existing `portal_to_room2` prefab — same event name, no new prefab needed for the
      return trip)
- [x] Tests (7 new, `local_coop_tests.rs`): `split_screen_viewport_system` produces correct
      non-overlapping physical-pixel viewport rects (even width, odd width, and a regression test
      proving `scale_factor_override` doesn't perturb the result since `physical_size()` is read
      directly); no-op when no split active; `ActiveSplitScreen` lifecycle; `parse_orbit_button`
      `"None"`; `split`+`party`-both-set warns and `split` wins; split spawns exactly one
      `SplitViewportSlot(0)` + one `SplitViewportSlot(1)`
- [x] Document the `CameraShake`-now-fires-on-both-cameras behavior change (`camera.rs` +
      `crates/ironhold_core/src/CLAUDE.md`)
- [x] Log the four `With<Camera3d>` "silently picks/ignores one camera" sites in
      `claude_suggestions.md` (no in-code marker today, unlike `CameraShake`'s documented one) —
      done during planning, before implementation
- [x] Update the `camera_modes` Icebox/Queued backlog note — split-screen's `SplitViewportSlot`/
      `ActiveSplitScreen` noted as a fourth thing that unification will eventually need to
      account for (after `OrbitCamera`, `FlyCamera`, `PartyOrbitCamera`)
- [x] Full review gate applies this time (unlike Stage 2): alignment, architecture, and
      wasm-perf-reviewer, since this touches Rust/schema and a new per-frame system. Alignment:
      ALIGNED, no blockers. Wasm-perf: OK, no regressions (one explicitly-non-blocking nit on
      guarding the per-frame `Camera.viewport` write, declined as premature optimization for a
      2-camera demo). Architecture: no Critical/Major issues; two Minor nits — CameraShake's
      fires-on-both-cameras behavior now documented at the `camera_shake_system` code site (not
      just CLAUDE.md), and `SplitViewportSlot`'s implicit binary (0/1) assumption flagged as a
      latent constraint for Stage 4/5's N-way split, not Stage 3
- [x] `docs/20_data_formats.md` + `crates/ironhold_core/src/CLAUDE.md` updates for the new
      fields/resource/system
- [x] WASM dev + release build (this stage DOES need a rebuild, unlike Stage 2), playtest
      checklist, Frank confirmation. Dev playtest surfaced a real bug (Bevy's camera-order-
      ambiguity warning spamming the console, since both split cameras defaulted to
      `Camera.order = 0`) — fixed by giving each split slot a distinct explicit order
      (`entity_spawner.rs`), re-verified via alignment review + full test suite + a second dev
      playtest, then the release build was confirmed clean (58 MB, no console errors)

### Open questions
- Is "fully fixed camera, no manual control at all" the right UX for Stage 3, or should
  gamepad-bound players eventually get their own right-stick orbit? No gamepad-driven camera
  control exists anywhere in `camera_orbit_system` today (mouse only), so adding it is out of scope
  here — defaulting to fully fixed for both players regardless of input device. Revisit only if
  this becomes a real ask.
- Should `room3` keep the same `max_view_box` size as `room1`/`room2`, or does halving each
  player's horizontal screen space change what a sensible box looks like? Decide during authoring
  and playtesting rather than guessing now.

### Acceptance criteria
- Given a scene with `split: (orientation: Vertical)` on the first player and 2 players present,
  when the scene loads, then two cameras spawn (not a `PartyOrbitCamera`), each rendering to its
  own half of the window with no visual bleed or clear-wipe between halves.
- Given the browser window is resized, when the next frame renders, then both viewport halves
  resize to match, correctly accounting for the display's scale factor.
- Given a split-screen scene, when either player scrolls the mouse wheel or drags to orbit, then
  neither camera's zoom or angle changes — manual control is disabled, only the fixed configured
  offset applies.
- Given `split` and `party` are both set on the first player by mistake, when the scene loads,
  then a warning is logged and `split` takes effect, not two conflicting camera setups.
