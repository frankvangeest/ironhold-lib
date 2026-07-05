# Feature: Local Co-op Foundation (2-player, shared camera, view-box clamp)

_Status: In Progress (Stage 1 Done, Stage 2 Ready/awaiting play-test, Stages 3–5 Queued)_
_Planned at: `c624c7b` (2026-07-03)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| Stage 1 | Two-player schema + shared framing camera + view-box clamp (this doc) | Done | `da81799` (2026-07-05) |
| Stage 2 | Portal/teleport action (moves both players together) | Ready (awaiting play-test) | — |
| Stage 3 | Vertical split-screen scene | Queued | — |
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
- [ ] Play-test checklist: walk player 1 through the portal alone → both players land in room2;
      walk back through the return portal → both land in room1 at their original spawn positions
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
