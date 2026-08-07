# Feature: Camera Modes

_Status: In Progress (v1 Done, v2 Queued)_
_Post-implementation review (2026-08-07): all 5 reviews (alignment, system-architect,
debug-detective, ux-gamedesigner-reviewer, wasm-perf-reviewer) independently converged on the same
3 critical bugs, giving unusually strong confidence they were real: (1) `follow_camera_system`/
`first_person_camera_system`/`fixed_camera_system` were written but never registered in `lib.rs`'s
Update chain, making `Follow`/`FirstPerson`/`Fixed` fully inert; (2) the local-coop split/party
spawn dispatch read `player_config.camera` directly instead of resolving through `camera_mode`,
which had already silently regressed the shipped `local_coop_demo` room4 migration (its
`orbit_button: "None"`/`zoom_speed: 0.0` split-screen mouse-decoupling was reverted to engine
defaults the moment the prefab dropped its legacy `camera:` block); (3) the new `fov` field
defaulted to 60° while Bevy's actual pre-existing default is 45°, silently widening every existing
project's camera the moment `insert_fov` started running unconditionally. All three fixed: the
three systems are now registered; the split/party/dynamic dispatch resolves through a new
`resolve_orbit_config_for_multiplayer` helper (Orbit payload wins, non-Orbit warns and falls back);
`default_fov()` is `45.0`. Also fixed along the way: `click_select_system` was mis-attributing
clicks in a `party:` scene to whichever player spawned first rather than the primary player
(`CameraTargets` on a party camera holds every player, not zero, so a bare `.first()` was wrong);
`spawn_party_orbit_camera` now applies the same `fov` as its sibling split cameras (previously
FOV would pop on every `split.dynamic` merge/split transition); `FirstPersonState`'s pitch clamp
used raw `min`/`max` (panics if authored backwards); `Fixed` now warns if authored with both or
neither of `look_at`/`look_at_entity`; nested `split:`/`party:` inside an `Orbit(...)` payload now
warns (parses fine, was silently inert). A new regression test
(`test_split_screen_honors_camera_mode_orbit_not_just_legacy_camera_field`) pins fix (2) directly.
Not fixed, logged to `planning/claude_suggestions.md` instead as non-blocking: `CameraModeDef::Party`
remains dead schema (no code path constructs it from a directly-authored payload); several
designer-facing doc gaps (the `PrefabDef` component field index table doesn't list
`camera_mode`/`split`/`party`; `docs/20_data_formats.md`'s two "Special tag" sections weren't
updated to mention `camera_mode`; a stale example pointer at room10); `ironhold_cli` has zero
camera-aware validation rules (nested-split-in-Orbit, `camera:`+`camera_mode:` both present,
`look_at_entity` referencing a non-existent id)._
_Planned at: `ece80c1` (2026-05-05)_
_Updated for local-coop/split-screen compatibility: `1fcef14` (2026-07-31); revised again after
plan-review at `2026-08-01` — 4 blocking questions resolved (see "Local co-op / split-screen
compatibility"): `ActiveCameraMode` as a per-camera component vs. a resource (the plan's own
internal contradiction), the authored-vs-runtime type split, a new `CameraTargets` component for
camera-ownership, and where `split:` is authored under the new schema. Also resolved: the
backward-compat mechanism (serde aliasing is impossible for this reshape; loader-side detection is
the only option), `SetCameraMode`'s multi-camera targeting (`owner_player: Option<u32>`), the
`Party` RON example, and `Party`+`split`/dynamic-split-vs-SetCameraMode precedence rules._
_Confirmation pass (2026-08-07, system-architect + ux-gamedesigner-reviewer, ahead of v1
implementation): both reviewers independently caught that the backward-compat mechanism as written
(detect the old `camera:`/`flycam:` **fields**) is wrong — both fields are optional tuning blocks,
and the actual mode switch is the `"flycam"`/`"player"` **tag**; field-presence detection would
leave the majority of player/flycam prefabs (8 of 10 player-bearing projects, 2 of 3 flycam
projects) cameraless. Corrected below to tag-driven detection. system-architect also found that
Blocker 3's `CameraTargets` component does not, by itself, make `dynamic_split_screen_system` or
`CameraShake`'s query expressible — both need to distinguish camera *kind* by Bevy query filter,
which `CameraTargets` (present on every camera) cannot do; resolved by adding zero-sized per-mode
marker components alongside `ActiveCameraMode` (see new "Blocker 5" below). ux-gamedesigner-reviewer
additionally flagged that the v1 migration story (only `3rd_person_game_demo`, a non-co-op project)
leaves the new `split:`-as-sibling-field syntax with zero shipped co-op examples; one
`local_coop_demo` room pair is now added to the v1 migration task. Both reviews confirmed no other
drift since 2026-08-01 — the four original Blockers' resolutions all still hold against current
source, and `player_model_source_unification` v2 (`7340eaf`, merged since the last revision) touched
no camera code._

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Camera mode unification — `ActiveCameraMode` component, backward-compat mapping | Done | `8bcecb5` (2026-08-07) |
| v2 | New modes (`Follow`, `Fixed`, `FirstPerson`) + `SetCameraMode` + transitions | Queued | — |

## What

A unified, data-driven camera system that lets game designers pick from a set of named camera presets — and switch between them at runtime via logic rules — without touching Rust. All camera behaviour is authored in RON: the mode, its tuning parameters, and when to transition between modes.

Currently the engine has two siloed cameras (`OrbitCamera`, `FlyCamera`) that are selected at scene load time based on entity tags and cannot switch mid-session. This feature replaces that with a single `CameraMode` enum in the scene/prefab RON, an optional transition config, and an `Action::SetCameraMode` that designers can fire from any rule or FSM state.

---

## Why

Camera feel is one of the highest-impact variables in game design. Most non-trivial games need more than one camera style (e.g. third-person gameplay → fixed cinematic on cutscene → back to third-person), and many need the ability to tune distance, angle, and smoothing per-scene without a Rust rebuild. This feature unblocks:

- Multi-scene projects that need different cameras per scene (e.g. top-down for a menu, orbit for gameplay)
- Cutscene / cinematic sequences triggered by logic rules
- Quick prototyping of feel: tweak follow distance, smoothing, FOV from RON without recompiling

---

## Modes (proposed)

| Mode | What it does | Replaces |
|------|-------------|---------|
| `Orbit` | Follows a target entity; player can orbit and zoom with mouse | `OrbitCamera` |
| `Follow` | Follows a target at a fixed offset; no free orbit — good for top-down/side-scrollers | New |
| `FirstPerson` | Camera locked to target's head position, looks where the character looks | New |
| `Fixed` | Static camera at a world position, looking at a fixed point or entity | New |
| `Flycam` | Free-flying, keyboard + mouse look; no target | `FlyCamera` |
| `Cinematic` | Follows a spline or lerps between named keyframes | Phase 2 — see backlog |

---

## Approach

### Schema changes

#### `CameraModeDef` (new, in `schema/scene_v2.rs` or `schema/player.rs`)

**RON syntax note (confirmed 2026-08-07 by actually running `ironhold_cli validate` against the
migrated `3rd_person_game_demo`, not just read from the schema): every fence below needs a
*double* layer of parens — `Orbit((field: value, ...))`, not `Orbit(field: value, ...)`.** A
newtype enum variant wrapping a plain named-field struct deserializes the struct as one complete
RON value; a struct's own RON representation is itself `(field: value, ...)`, so the full form
nests one inside the other. The single-paren form throws `Expected struct CameraConfig but found
"offset"` — a real error caught only by the migration, not by prose review. All fences corrected
below.

```ron
// In a prefab's components block, or at scene level
camera_mode: Orbit((
    // target_entity: "player",  // optional; defaults to the player entity
    offset: (0.0, 5.0, 10.0),
    look_at_offset: (0.0, 2.0, 0.0),
    orbit_speed: 0.5,
    zoom_speed: 10.0,
    min_radius: 2.0,
    max_radius: 20.0,
    min_pitch: 0.1,
    max_pitch: 1.5,
    orbit_button: "Either",
    character_rotate_button: "Right",
    initial_pitch: 0.5,
    initial_yaw: 0.0,
    fov: 60.0,   // optional, degrees
    transition: (
        duration_secs: 0.4,
        ease: "EaseInOut",
    ),
)),
```

```ron
camera_mode: Fixed((
    position: (20.0, 10.0, 0.0),
    // Either a static world point OR a named entity (not both):
    look_at: (0.0, 0.0, 0.0),
    // look_at_entity: "boss",   // tracks a moving entity each frame
    fov: 50.0,   // optional; narrower FOV suits cinematic fixed shots
    transition: (duration_secs: 0.6, ease: "EaseIn"),
)),
```

```ron
camera_mode: Follow((
    offset: (0.0, 4.0, 8.0),
    look_at_offset: (0.0, 1.5, 0.0),
    smoothing: 8.0,         // position lerp speed — higher = snappier, 0 = instant
    rotation_smoothing: 6.0, // separate smoothing for look-at rotation
    fov: 75.0,               // optional, degrees; default 60
    transition: (duration_secs: 0.3, ease: "Linear"),
)),
```

```ron
camera_mode: FirstPerson((
    eye_offset: (0.0, 1.7, 0.0),
    sensitivity: 0.002,
    min_pitch: -1.4,
    max_pitch: 1.4,
    fov: 90.0,   // optional, degrees; FPS games typically use 80–100
)),
```

```ron
camera_mode: Flycam((
    speed: 20.0,
    fast_speed: 60.0,
    sensitivity: 0.002,
    look_button: "Right",
)),
```

```ron
// Shared camera framing every "player"-tagged entity in the scene — authored on the FIRST
// player only (same convention as split: below). zoom_margin/min_radius/max_radius now live
// together in one struct (today they're split across CameraConfig and PartyZoomDef).
camera_mode: Party((
    look_at_offset: (0.0, 1.2, 0.0),
    // radius = clamp(max pairwise player separation + zoom_margin, min_radius, max_radius)
    zoom_margin:  6.0,
    min_radius:   8.0,
    max_radius:  28.0,
    orbit_speed:  0.4,
    zoom_speed:   8.0,
    orbit_button: "Right",
    allow_manual_zoom: false,  // true = scroll wheel nudges the auto-derived distance
)),
```

**`fov:` scope, decided during the 2026-08-07 confirmation pass**: every mode fence above shows a
static `fov:` field — this ships in **v1** (applied once at spawn/mode-resolve time, exactly like
`offset:`/`look_at:`). Only *interpolating* `fov` smoothly during a `SetCameraMode` transition is
**v2** (it requires `CameraBlendState`, which doesn't exist until then). Calling this out
explicitly avoids a silent no-op field in v1 — the same footgun class as `ActionSlotDef.label` and
`depth_scale` elsewhere in this codebase.

The existing `CameraConfig` and `FlyCamDef` structs become the inner payloads of `Orbit` and `Flycam` variants respectively — this is a backwards-compatible rename at the RON level if we add serde aliases. **Correction (2026-08-01, ux-gamedesigner-reviewer): the serde-alias approach is not viable** — `camera: (offset: ..., ...)` is a named-field struct; `camera_mode: Orbit(...)` is an enum newtype variant. A serde `alias` renames a *key*, it cannot reshape a struct into an enum variant. **The only viable backward-compat approach is scene-loader-side detection** (option (b) in the Open Questions below) — commit to it now rather than presenting a false choice between two options later. The `Orbit` fence above is also missing 3 fields that shipped after this plan's original 2026-05-05 draft: `look_speed` (keyboard camera-look rate — load-bearing in every split-screen room, since mouse orbit is disabled there), `party`, and `split` — the last two are addressed by Blocker 4 above (they move to a sibling field, not inside this fence, so the `Orbit` fence itself only needs `look_speed` added).

#### `TransitionConfig` (new, shared sub-struct)

```rust
pub struct CameraTransition {
    pub duration_secs: f32,
    pub ease: EaseKind,   // Linear, EaseIn, EaseOut, EaseInOut
}
```

Every mode variant carries an optional `transition` field. When `SetCameraMode` fires, the system lerps `Transform` (position + rotation via `Quat::slerp`) over `duration_secs` from the old position to the new one. If `transition` is absent, the cut is instant.

#### `Action::SetCameraMode` (new action variant, v2-scope)

**Multi-camera targeting shape, decided 2026-08-01 (both reviews converged on the same answer from
different angles — record it now even though `SetCameraMode` itself is v2, so the shape doesn't
need to change later):**

```rust
SetCameraMode { mode: String, owner_player: Option<u32> },   // struct variant, named fields
```

```ron
// Every active camera (single-player: the only one; split-screen: every viewport)
SetCameraMode(mode: "cutscene_fixed")

// Player 2's viewport only — player 1 keeps playing uninterrupted
SetCameraMode(mode: "cutscene_fixed", owner_player: 1)
```

`owner_player` reuses `ActionBarDef.owner_player`'s exact existing convention (already
authored by hand on every co-op prefab as `PrefabDef.player_index`) — **not** a raw
`SplitViewportSlot`/camera-slot index, which is engine-assigned and reassigned live on hot-join/
leave, and appears nowhere in RON a designer writes. Semantics:

| `owner_player` | Behavior |
|---|---|
| omitted | Applies to **every** active camera |
| `Some(n)`, split scene, player `n` exists | That player's camera only |
| `Some(n)`, party scene (one shared camera) | `warn!` + no-op — no per-player camera to retarget |
| `Some(n)`, seat not yet hot-joined | `warn!` + no-op |
| `Some(n)` ≥ live player count | `warn!` + no-op |

**Omitted must default to "all cameras," not "the primary camera"** — "the primary camera" is an
engine-internal `camera_priority_key` concept invisible in a designer's RON, so a rule authored in
a single-player project would silently change meaning the moment it's reused in a co-op scene. This
also matches the `CameraShake` acceptance bar above (which already requires "every active camera,"
not one) — the two camera actions having different implicit defaults would be an arbitrary
inconsistency to memorize. **Extend the same `owner_player: Option<u32>` field to
`Action::CameraShake` in the same pass** (e.g. "shake only the player who got hit," a natural co-op
want) rather than doing a second round of doc/schema churn on the same action later.

Designers fire this from logic rules:

```ron
// In state_machine.ron or rules.ron
on: "ui.button_pressed:enter_cutscene",
do_actions: [SetCameraMode(mode: "cutscene_fixed")],
```

Named modes are registered via the prefab catalog or a new `camera_modes` block at scene level (design decision: see Open Questions).

---

## Local co-op / split-screen compatibility (added 2026-07-31, ahead of plan-review)

This plan was drafted 2026-05-05, **before any of local co-op existed**. Since then, an entire
split-screen system has shipped and is now heavily used by `local_coop_demo` (**10 rooms** as of
2026-08-07, including room10's new mixed GLB+primitive player pairing from
`player_model_source_unification` v2 — confirmed to touch no camera code, so no new reconciliation
needed there) and this plan's own "Multiple cameras... out of scope for phase 1" open question
(below) is simply no longer true — split-screen is real, shipped, load-bearing functionality, not a
hypothetical future concern. This section reconciles the two before implementation starts, so
plan-review isn't
re-discovering a gap `planning/backlog.md`'s own entry for this feature already flags ("Local
co-op (2026-07-04) added a third sibling, `PartyOrbitCamera`, with its own duplicated tuning
fields/mouse-orbit block; Stage 3 (2026-07-05) uses real `OrbitCamera`s for split-screen but adds
`SplitViewportSlot`/`ActiveSplitScreen` as separate, camera-mode-agnostic state... this
unification should still account for split-screen's viewport-assignment concern as a fourth
mode-adjacent thing, not just Orbit/Fly/Party").

**Revision (2026-08-01, after plan-review — system-architect + ux-gamedesigner-reviewer): 4
blocking questions resolved.** Both reviews independently found the fold-in/keep-separate boundary
above sound in spirit, but found the plan under it contained a direct internal contradiction and
three unresolved schema decisions that make it un-buildable as written. Resolved below.

### Blocker 1 (both reviews) — `ActiveCameraMode` must be a component, not a resource

The Phases table says "`ActiveCameraMode` resource"; Key Rust change #2 says "a single component."
With split-screen shipped this is decisive, not a wording nit: `dynamic_split_screen_system` keeps
**three simultaneously-alive cameras** (2 per-player + 1 merged) of **two different kinds** alive
for a scene's entire lifetime, toggling only `Camera.is_active`. A single scene-global resource
cannot represent that. **Resolved: `ActiveCameraMode` is a per-camera-entity component**, holding
resolved *runtime* state (see Blocker 2). The Phases table row is corrected.

### Blocker 2 (system-architect) — two named types, not one

A deserialized `CameraModeDef` (the authored schema) cannot hold `Entity` references, pre-resolved
`KeyCode`s, or mutable per-frame state (yaw/pitch/radius, `manual_zoom_offset`). **Resolved: two
distinct types.** `CameraModeDef` (in `schema/camera.rs`, `Deserialize`) is the authored RON shape,
unchanged from the Approach section's fences. A separate, non-`Deserialize` `ActiveCameraMode`
(runtime component, `capabilities/camera.rs`) holds the resolved-at-spawn-time working state — this
is the direct analog of how `OrbitCamera` today is a runtime component built *from* a `CameraConfig`
at spawn time, not the `CameraConfig` itself. Every mode variant needs its own resolved payload
shape (mirroring today's `OrbitCamera` vs. `PartyOrbitCamera` field lists, which already differ
structurally — `target: Entity` vs `targets: Vec<Entity>`, presence/absence of `look_*_key`/
`gamepad_index`/`radius`-as-stored-vs-derived).

### Blocker 3 (system-architect) — camera-to-player ownership needs its own component

Six consumers currently read `OrbitCamera.target: Entity` for "which player does this camera
belong to": `click_select_system` (owning-player attribution — **the plan's original draft
incorrectly claimed this system doesn't query `OrbitCamera`; it does, and its documented fallback
behavior for a *targetless* camera, e.g. `PartyOrbitCamera`, changes once Party folds into the same
component**), `split_viewport_player_label_spawn_system`, `target_hud_update_system`,
`dynamic_split_screen_system` (separation distance), `SceneStateParams::orbit_cameras`
(`CameraShake`'s query), and — added by the 2026-08-07 confirmation pass, since `f4cca59`
(2026-08-05) deleted `OrbitCamera.gamepad_index` and made `camera_orbit_system` itself resolve the
owning player via `orbit.target` for gamepad binding (`camera.rs:135`, `bound_q.get(orbit.target)`)
— `camera_orbit_system` (the gamepad-lookup half of the orbit system, not just the five systems
above it). Burying `target`/`targets` inside each `ActiveCameraMode` variant's payload would force
all six into an exhaustive per-variant match just to ask "who owns this camera." **Resolved: a
single `CameraTargets(Vec<Entity>)` component, present on every camera regardless of mode** —
length 0 for `Fixed`/`Flycam` (no owner), length 1 for `Orbit`/`Follow`/`FirstPerson` (today's
single `target`), length N for `Party` (today's `targets: Vec<Entity>`). This unifies
`OrbitCamera.target` and `PartyOrbitCamera.targets` into one shape and lets all six consumers query
`Query<&CameraTargets>` uniformly (`.first()` for "the/a owning player," with the existing
fallback-to-primary-player convention when empty) instead of matching on `ActiveCameraMode`'s
variant. **Note: `Party` carries no gamepad/look bindings today (deliberately) and must not gain
any as a side effect of this unification** — `camera_orbit_system`'s gamepad-pitch resolution stays
scoped to the `Orbit` variant only.

### Blocker 4 (both reviews) — where does `split:` live? (the authoring-home question)

`SplitScreenDef` (`party`/`split`/`own_viewport_only`) is authored today **inside** the exact
`CameraConfig` struct this plan turns into `Orbit`'s payload — but the fold-in/keep-separate
boundary above says split-screen state must stay orthogonal to camera mode. Both statements can't
be true of the same authoring location at once; three options were on the table (sibling field
under `components:`, folded into `Orbit`'s payload, promoted to scene level) and none was chosen.
**Resolved for v1: `split:` (and `own_viewport_only`) become a sibling field of `camera_mode:`
under `components:`** — i.e.:
```ron
components: (
  camera_mode: Orbit(...),
  split: ( orientation: Vertical, own_viewport_only: true ),
),
```
(explicit fence added 2026-08-07 confirmation pass — `own_viewport_only` nests **inside**
`split:`, it does not become its own top-level sibling) — the lowest-migration-cost option that
still honestly reflects the orthogonality claim (a sibling field, not nested inside the mode
payload). This still requires a mechanical migration of every existing `camera: (split: (...))`
block, but is a smaller conceptual jump than promoting split-screen's designer-facing switch to
scene level. **Promoting `split:` to `GameSceneV2` (a scene-level field, not a per-player one) is a
genuinely stronger long-term answer** — it would finally retire the "only the first player-tagged
entity's config is read" footgun this plan's own docs task list doesn't currently address — but is
deliberately deferred past v1 to keep this already-large unification's scope from also becoming a
split-screen-authoring redesign; already logged to `planning/claude_suggestions.md` as a concrete
v2+ candidate. **This relocation applies identically to primitive players** (confirmed 2026-08-07):
`player_model_source_unification` v2 unified the GLB and primitive player spawn paths, and room10's
`player_p2_primitive_split_ring` already authors a full `camera:` block under `kind: Primitive` — so
the implementer should not assume `camera_mode:`/`split:` migration only touches `kind: Actor`
players.

**Party + `split` interaction, now defined** (ux-gamedesigner-reviewer): the pre-existing
mutual-exclusivity rule (`party`/`split` are exclusive; if both are set, `split` wins with a
warning) is **preserved, not implicitly loosened** — a designer directly authoring `camera_mode:
Party(...)` alongside a sibling `split: (...)` still gets the same warn-and-`split`-wins behavior
as today. The *internally-managed* merged-state camera `dynamic_split_screen_system` spawns for a
`split.dynamic` scene uses `ActiveCameraMode`'s `Party` variant under the hood (an engine
implementation detail, not something the designer authors directly) — this is not a contradiction
of the exclusivity rule, since the designer never writes `camera_mode: Party(...)` in that case.

**`split.dynamic` vs. `SetCameraMode` precedence, now defined** (ux-gamedesigner-reviewer): a
dynamic-split scene has two potential writers of a camera's active mode — the engine's
merge/split distance thresholds, and a designer's `SetCameraMode`. **Resolved: while a
`SetCameraMode`-originated mode is active on a camera, `dynamic_split_screen_system`'s automatic
merge/split transitions are suspended for that camera**, resuming only on an explicit
`SetCameraMode` back to the scene-authored default (or a full scene reload). This is a v2-scope
detail (since `SetCameraMode` itself is v2), recorded now so it isn't rediscovered as an ambiguity
once v2 starts.

**What must stay a *separate*, camera-mode-agnostic layer — unaffected by any of the above:**
- `SplitViewportSlot(u32)`, `ActiveSplitScreen(Option<SplitOrientation>)`, `DynamicSplitConfig`,
  `ActiveSplitSlotCount` — "how many cameras and how is the window tiled" stays orthogonal to "what
  does one camera's transform-following logic do," exactly as originally argued. Nothing about
  Blockers 1-4 changes this.
- `camera_priority_key(entity, slot: Option<&SplitViewportSlot>)` — verified (not just claimed)
  transparent for 3 of its 4 consumers (`rebuild_pool_meshes_system`, `world_label_screen_pos_
  system`, `nameplate.rs`'s distance stash); `click_select_system` is the 4th and needed Blocker 3's
  `CameraTargets` fix specifically because it *does* read ownership (see above) — with that fix
  applied, all four are transparent to the `OrbitCamera` → `ActiveCameraMode` rename.
- `own_viewport_only`'s per-camera `RenderLayers` (keyed on `PlayerIndex`) and
  `TargetRingVisibilityMode` — existing test coverage for the *insertion* sites is strong
  (system-architect verified 6 tests keying on exact layer membership, one of which queries
  `With<PartyOrbitCamera>` directly and so will fail to **compile** the moment that component is
  deleted — the ideal failure mode). **The actual uncovered risk is the *non*-insertion fallback
  path** (`spawn_player_entity`/`Action::Spawn`'s dedicated full-window camera, which today
  correctly gets neither a layer nor a warn-worthy gap) — collapsing three spawn paths into one
  unified path (Key Rust change #5) is exactly where a uniform insertion or a dropped warn could
  slip through silently. Add a **named** test for this fallback case specifically, not "the
  existing suite happens to cover it."

**What must be *fixed*, not just preserved: `Action::CameraShake`.** Already has a documented gap —
fires correctly on both simultaneously-active split cameras (real `OrbitCamera`s) but silently
no-ops in a `party:` scene (`PartyOrbitCamera` unqueried). Verified (system-architect): the
existing ordering chain already runs `camera_shake_system` after both `camera_orbit`/`party_camera_
follow`, so no reorder is needed once the query moves to the new component; and the executor's
shake loop is already a `for camera_entity in ... .iter()`, not a `.single()`, so "shake every
active split camera" is already correct, not new work. **What genuinely is new work, and a named
regression risk**: the query must stay **variant-filtered** (`Orbit` + `Party`, excluding `Fixed`/
`FirstPerson`/`Flycam`) — a naive `With<ActiveCameraMode>` would insert `CameraShakeState` onto a
flycam camera too, and since `fly_camera_system` runs *after* `camera_shake_system`, it would
silently overwrite the shake offset instead of preserving today's explicit `warn!("no orbit camera
in scene — shake ignored")`. Silently doing nothing where today's diagnostic explicitly warns is a
real, easy-to-miss regression — add it as a named acceptance criterion, not just "shake still
works."

### Blocker 5 (confirmation pass, 2026-08-07, system-architect) — `dynamic_split_screen_system` and `CameraShake` need type-level markers, not just `CameraTargets`

`CameraTargets` (Blocker 3) solves "who owns this camera" but not "what *kind* of camera is this,"
and two systems need the latter as a Bevy **query filter**, not as data to branch on at runtime.
`dynamic_split_screen_system` (`capabilities/camera.rs:700-701`) queries:
```rust
mut split_cameras: Query<(&mut Camera, &OrbitCamera), (With<SplitViewportSlot>, Without<PartyOrbitCamera>)>,
mut party_camera:  Query<&mut Camera, (With<PartyOrbitCamera>, Without<SplitViewportSlot>)>,
```
then calls `party_camera.single_mut()`. Bevy filters on **component types**, not enum variants —
`With<ActiveCameraMode::Party>` is not expressible, and a single `ActiveCameraMode` component
collapses exactly the type-level distinction this query relies on. The same problem hits the
`CameraShake` acceptance criterion above ("variant-filtered `Orbit`+`Party`, excluding
`Fixed`/`FirstPerson`/`Flycam`") — `SceneStateParams::orbit_cameras` (`scene_manager/mod.rs:604`) is
a `With<OrbitCamera>` filter today, and there is no drop-in replacement once `OrbitCamera` is gone.

**Resolved: alongside `ActiveCameraMode` (the data-carrying component), spawn a zero-sized
per-mode marker component** — `OrbitCameraMode`, `PartyCameraMode`, `FixedCameraMode`,
`FollowCameraMode`, `FirstPersonCameraMode`, `FlycamCameraMode` — one inserted per camera matching
its `ActiveCameraMode` variant. This makes both queries above trivially expressible
(`With<PartyCameraMode>`, `With<OrbitCameraMode>` + `With<PartyCameraMode>` filters) and preserves
today's fails-to-compile-on-rename safety net the plan already praises for
`local_coop_tests.rs:788` (`With<PartyOrbitCamera>` today; `With<PartyCameraMode>` after). Keep
`ActiveCameraMode` itself as the single source of truth for the *data*; the marker components exist
purely so queries can filter by kind without an exhaustive match. Whichever system spawns/updates
`ActiveCameraMode` (mode resolution, and later `SetCameraMode` in v2) is responsible for keeping the
matching marker in sync — insert-or-swap on mode change, not two independently-mutable facts.

## Key Rust changes

**(v1 scope unless marked v2)**

1. **`schema/camera.rs`** (new file)
   - `CameraModeDef` enum with all mode variants (the authored/`Deserialize` schema — see Blocker 2)
   - `CameraTransition` struct (**v2** — only meaningful once `SetCameraMode` exists)
   - Per-mode config structs (reuse existing `CameraConfig`/`FlyCamDef` as inner payloads); `split:`/
     `own_viewport_only` move to a sibling field, not inside `Orbit`'s payload (Blocker 4)

2. **`capabilities/camera.rs`** (refactor)
   - Replace `OrbitCamera` + `FlyCamera` + `PartyOrbitCamera` with a per-camera **component**
     `ActiveCameraMode` (runtime-resolved state — Blockers 1 & 2, NOT the same type as
     `CameraModeDef`) plus a separate `CameraTargets(Vec<Entity>)` component present on every
     camera (Blocker 3)
   - A zero-sized per-mode marker component alongside `ActiveCameraMode`
     (`OrbitCameraMode`/`PartyCameraMode`/`FixedCameraMode`/`FollowCameraMode`/
     `FirstPersonCameraMode`/`FlycamCameraMode`) so `dynamic_split_screen_system` and
     `CameraShake`'s query can filter by camera kind (Blocker 5) — Bevy queries filter on
     component types, not `ActiveCameraMode`'s enum variant
   - One unified `camera_system` that dispatches on the active mode
   - `CameraBlendState` component for in-progress transitions (**v2**)

3. **`schema/actions.rs`** (**v2**)
   - Add `SetCameraMode { mode: String, owner_player: Option<u32> }` (struct variant — see the
     multi-camera targeting decision above); extend `Action::CameraShake` with the same
     `owner_player: Option<u32>` field in the same pass

4. **`runtime/action_executor.rs`** (**v2**)
   - Handle `SetCameraMode`: resolve `owner_player` to the target camera(s) per the table above,
     look up the named mode, set `ActiveCameraMode`, insert `CameraBlendState`

5. **`runtime/scene_manager/scene_loader.rs`** and **`runtime/scene_manager/entity_spawner.rs`**
   - Replace the camera-spawn construction (today: `spawn_orbit_camera_for_player`, the sole
     `OrbitCamera` construction site as of `player_model_source_unification` v1 removing the
     scene_loader's inline capsule/primitive block) with a single helper that builds
     `ActiveCameraMode` + `CameraTargets` + the matching marker component (Blocker 5) from the
     resolved `CameraModeDef`. **Camera *count* is not collapsible** — the 1 party / N split / 1
     fallback branch in `spawn_players_and_camera` (a scene-level decision derived from the first
     player's `split`/`party`) must survive; only the per-camera *component construction* unifies.
   - Backward compat via **loader-side detection, keyed on the prefab's `tags`, not on
     field-presence** (corrected 2026-08-07 confirmation pass — see the corrected Open Questions
     entry): `camera:`/`flycam:` are both optional tuning blocks (`schema/catalog.rs`,
     `#[serde(default)] pub camera: Option<CameraConfig>` etc.) — 8 of 10 player-bearing projects
     and 2 of 3 flycam projects have no such block at all today, so detecting the *field* would
     leave them cameraless. Detect the **tag** instead: `tags: ["flycam"]` + no `camera_mode:` →
     synthesize `Flycam(...)` from `flycam:` if present, else engine defaults; `tags: ["player"]` +
     no `camera_mode:` → synthesize `Orbit(...)` from `camera:` if present, else
     `default_camera_config()`. Explicit `camera_mode:` always overrides both.
   - Also migrate the two remaining authoring-location readers of the old `split:`/`party:`
     location (Blocker 4): `entity_spawner.rs:649-650`'s `first.camera.split.as_ref()` /
     `first.camera.party.as_ref()` (the actual 3-way camera-spawn authority) and
     `scene_loader.rs:746`'s `is_split_screen` gate for `WorldLabelRank` duplication. Thread the new
     sibling `split:`/`party:` field through `PlayerConfig` (`schema/player.rs`) and
     `assemble_player_config`.
   - **Open, v1-scoped design decision (added 2026-08-07): does `tags: ["flycam"]` remain the
     selector once `camera_mode: Flycam(...)` exists, or does authoring `camera_mode:` retire the
     tag's role?** Cross-reference `planning/backlog.md`'s "Promote magic `tags` to typed prefab
     fields" icebox item — the two would fight if decided independently. **Recommended for v1
     (lowest churn): the tag remains required and remains the sole mode-selector; `camera_mode:`
     only supplies tuning when the tag is already `"flycam"`/`"player"`.** Full tag retirement is
     deferred, matching that icebox item's own existing scope.

6. **`ironhold_cli`** — **v1 scope is just the `cargo check -p ironhold_cli` compile gate**: v1
   adds no new `Action` variant (that's v2's `SetCameraMode`), and `PrefabComponents`' new
   `camera_mode`/relocated `split`/`party` fields carry no `deny_unknown_fields`, so they flow
   through the CLI's shared schema types without a code change there. Still worth a spot-check of
   `query prefabs` output on the migrated project (Task list), since field *type* changes (not just
   additions) have silently broken `query.rs` before in this codebase.

---

## Tasks

**v1 (unification, no runtime mode-switching yet):**
- [x] Write `CameraModeDef` enum and sub-structs in `schema/camera.rs`, including the `Party`
      variant (fence added above) and the `split:`/`own_viewport_only` sibling-field placement
      (Blocker 4)
- [x] Implement `ActiveCameraMode` (runtime component, distinct type from `CameraModeDef`) +
      `CameraTargets(Vec<Entity>)` in `capabilities/camera.rs`; implement `camera_system` dispatch
      for Orbit/Party (Follow/Fixed/FirstPerson/Flycam's *systems* can land in v1 too since they
      don't depend on `SetCameraMode` — only the *switching* action is v2)
- [x] Update `scene_loader.rs` to resolve `CameraModeDef` from prefab (+ the new sibling `split:`
      field) and spawn `ActiveCameraMode`/`CameraTargets`; commit to loader-side backward-compat
      detection (not serde aliasing — see the corrected note above)
- [x] Fold `PartyOrbitCamera` into `ActiveCameraMode`'s `Party` case, removing the ~10 duplicated
      tuning fields (`planning/claude_suggestions.md` ▸ Camera) in favor of the shared struct
- [x] Migrate `click_select_system`, `split_viewport_player_label_spawn_system`, `target_hud_
      update_system`, `dynamic_split_screen_system`, `SceneStateParams::orbit_cameras`, and
      `camera_orbit_system`'s gamepad-owner lookup to read `CameraTargets` instead of
      `OrbitCamera.target`/`PartyOrbitCamera.targets` (Blocker 3's six named consumers); also
      migrate `entity_spawner.rs:649-650` and `scene_loader.rs:746` off the old `camera.split`/
      `camera.party` authoring location onto the new sibling field, threading it through
      `PlayerConfig`/`assemble_player_config`
- [x] Add zero-sized per-mode marker components (`OrbitCameraMode`/`PartyCameraMode`/etc., Blocker
      5) alongside `ActiveCameraMode`, and re-point `dynamic_split_screen_system`'s
      `With<PartyOrbitCamera>`/`Without<PartyOrbitCamera>` filters at the new markers
- [x] Regression tests — confirm `SplitViewportSlot`/`ActiveSplitScreen`/`DynamicSplitConfig`/
      `ActiveSplitSlotCount` and `camera_priority_key`-based selection (`rebuild_pool_meshes_
      system`/`click_select_system`/`world_label_screen_pos_system`/`nameplate.rs`) continue to
      function unchanged — run the full `local_coop_tests.rs` suite (133+ tests) against the
      refactored code. **Additionally, a *named* test** (not just "the suite happens to cover it")
      asserting the non-hot-join `Action::Spawn`/`spawn_player_entity` fallback camera still gets
      no `RenderLayers` (and the existing warn still fires) in an `own_viewport_only` scene — the
      specific gap the three-spawn-path collapse risks silently losing. **Done**: full suite is now
      139 tests (136 pre-existing + 3 new — the fallback-camera test plus two tag-driven-compat
      tests for the "no camera:/flycam: block at all" majority shape), all passing.
- [x] Fix `Action::CameraShake` to also fire on the `Party` case (closing the documented gap) via a
      query filtered on the new **marker components** (`With<OrbitCameraMode>` or
      `With<PartyCameraMode>`, excluding `Fixed`/`FirstPerson`/`Flycam`'s markers — Blocker 5; a
      bare `With<ActiveCameraMode>` cannot express this distinction), so a flycam scene still gets
      its explicit `warn!` instead of a silent overwrite by `fly_camera_system`; confirm (already
      true today, per the ordering chain — not new work) every active split camera shakes, not just
      one
- [x] A Migration section in this plan (see Open Questions) with an old→new RON table for the three
      real shapes (plain `camera:`, `camera:` + `party:`, `camera:` + `split:`); migrate **one**
      low-risk project fully (`3rd_person_game_demo` — 3 plain `camera:` blocks, no `party`/`split`/
      `flycam`; two of its three player prefabs also spawn via `Action::Spawn` from
      character-select rather than as scene entities, so this migration also exercises the
      runtime-spawn camera path the fallback-camera test cares about) as living proof. **Also
      migrate one `local_coop_demo` room-exclusive pair to the new `split:`-as-sibling-field
      syntax** (recommended: room4's `player_p1_split_h`/`player_p2_split_h` — the only pair used
      exclusively by one room, simplest fixed-`Horizontal` split) so the new co-op authoring shape
      has at least one real shipped example, not just a docs table. The other 9 `local_coop_demo`
      rooms deliberately stay on legacy syntax with a RON comment saying so (its ~16 `camera:`
      blocks, post-room10, are the compat-path's own regression test). **Done** — both migrations
      also surfaced and fixed a real RON syntax bug in this very plan's own fences (missing a
      double-paren layer; see the Migration section above).
- [x] `ironhold_cli`: `cargo check -p ironhold_cli` gate (new schema type); spot-check `query
      actions`/`query prefabs` output on a migrated project
- [x] Docs: rewrite `docs/20_data_formats.md`'s camera-doc surface — corrected range **~1814-2557**
      as of 2026-08-07 (was mis-stated as ~2023-2350; the real span starts at the `"flycam"` tag
      section (1814) and runs through Grid split (2451-2557), including `own_viewport_only`
      (2282) which the old range omitted entirely) — for the new shape, keeping a short "Legacy
      `camera:`/`flycam:` fields" compat subsection; update the `CameraShake` actions-table row
      (line 3329, currently says "the active orbit camera," doesn't mention split/party behavior at
      all); also update the gamepad-camera notes at lines 2023/2027 which name `CameraConfig`/
      `OrbitCamera` directly
- [x] Docs: update `docs/STATUS.md` (lines 52/54, "Orbit camera"/"Fly camera" capability entries),
      `docs/10_architecture.md` (line 13), and `docs/00_overview.md` (line 52) — all three name
      `camera (orbit)`/`flycam` as separate capabilities and go stale the moment `ActiveCameraMode`
      unifies them
- [x] Docs: update `crates/ironhold_core/src/CLAUDE.md`'s local-coop camera section and its "Known
      limitation" `CameraShake` note to reflect the new unified reality
- [ ] Demo: `local_coop_demo` room demonstrating `SetCameraMode` retargeting one player's viewport
      to `Fixed` while the other keeps `Orbit` (**v2**, but name the room/scene now so it isn't
      forgotten — the single-player-only demo task below doesn't showcase the co-op-specific
      capability the split-screen reconciliation spent the most words on)

**v2 (runtime mode-switching):**
- [ ] Add `SetCameraMode { mode: String, owner_player: Option<u32> }` to `Action` enum and document
      it; extend `Action::CameraShake` with the same `owner_player` field in the same pass
- [ ] Implement `CameraBlendState` transition lerp (position + slerp rotation + FOV lerp)
- [ ] Handle `SetCameraMode` in `action_executor.rs`, resolving `owner_player` per the targeting
      table above (including the `warn!`+no-op cases: party scene, unjoined seat, out-of-range
      index)
- [ ] Hot-join interaction: a player joining after `SetCameraMode` has retargeted another camera
      spawns in the scene-authored default mode, not the currently-active override — the two are
      independent per-camera states
- [ ] `dynamic_split_screen_system` interaction: suspend automatic merge/split transitions on any
      camera currently under a `SetCameraMode` override, resuming only on an explicit
      `SetCameraMode` back or a scene reload (see the precedence decision above)
- [ ] Update `entity_logic_demo`/`quick_scene` with a single-player camera-switch example, and
      complete the `local_coop_demo` per-viewport retargeting demo named in v1's task list
- [ ] Integration tests: mode switch fires correctly (including `owner_player` targeting each
      `warn!` case), transition completes, fallback camera spawns

---

## Notes

### Runtime player spawn and the default camera

When a player character is spawned at runtime via `Action::Spawn` (e.g. from a character-select screen), the orbit camera spawns as part of that path. If the scene has no player entity in its RON, no 3D camera exists until the spawn fires — which causes at least one black frame.

**Clean solution**: `Camera::is_active = false` on the default camera (Bevy supports this natively without despawning). A "fallback" scene camera can sit deactivated, then the orbit camera takes over at full priority (`Camera::order`) when it spawns. No despawn/respawn needed.

**Open design question for camera modes implementation**: should the fallback camera be a standard part of every scene that omits a player entity, or should the camera-modes system make the primary camera persistent across scene loads and simply switch its `ActiveCameraMode`? The persistent-camera approach avoids the one-black-frame problem entirely and is architecturally cleaner for scene transitions. Consider this when implementing the unified `camera_system`.

---

## Implementation notes

### CameraShake coupling
`Action::CameraShake` (shipped `b8723ec` 2026-06-19) inserts a `CameraShakeState` component onto `OrbitCamera` entities; `camera_shake_system` filters `With<OrbitCamera>`. **Shipped as planned, v1** (`ironhold_core::CLAUDE.md`'s "Fixed" note): `scene_state.orbit_cameras` and `camera_shake_system` both now filter `Or<(With<OrbitCameraMode>, With<PartyCameraMode>)>` — the "shared marker" approach anticipated here, not a separate `ActiveOrbitCamera` tag.

---

## Migration

Old→new RON shapes for the three real cases in this codebase, and where each is demonstrated:

**Plain `camera:` (no party/split/flycam)** — fully migrated in `3rd_person_game_demo` (all 3 player prefabs):
```ron
# Old
camera: ( offset: (0.0, 2.5, 8.0), look_at_offset: (0.0, 1.0, 0.0), zoom_speed: 8.0,
          orbit_speed: 0.4, min_radius: 3.0, max_radius: 18.0, orbit_button: "Right" ),
# New — note the double parens (see the RON syntax gotcha in docs/20_data_formats.md,
# found empirically: a single-paren `Orbit(field: value, ...)` fails to parse)
camera_mode: Orbit((
  offset: (0.0, 2.5, 8.0), look_at_offset: (0.0, 1.0, 0.0), zoom_speed: 8.0,
  orbit_speed: 0.4, min_radius: 3.0, max_radius: 18.0, orbit_button: "Right",
)),
```

**`camera:` + `split:`** — migrated for one room-exclusive pair in `local_coop_demo`
(`player_p1_split_h`/`player_p2_split_h`, room4 only):
```ron
# Old (nested inside camera:, authored on the first player only)
camera: ( offset: ..., ..., split: ( orientation: Horizontal ) ),
# New (split: is a sibling of camera_mode:, still first-player-only)
camera_mode: Orbit(( offset: ..., ... )),
split: ( orientation: Horizontal ),
```

**`camera:` + `party:`** — not migrated in this pass (no test/demo project currently uses a plain
`party:` block without also being a `split`/`dynamic` scene); the mechanical shape is identical to
the `split:` case above (`party:` becomes a sibling field the same way) and needs no new pattern.

The other 9 `local_coop_demo` rooms and every other example project deliberately stay on the legacy
nested form — this is the backward-compat path's own regression test, not an oversight. Both
migrated shapes above pass `ironhold_cli validate` and their full existing test coverage unchanged.

---

## Open questions

- **Named mode registry**: should named modes live in the prefab catalog, in a new `camera_modes:` block at scene level, or as inline RON in the action argument? Inline is simplest but prevents reuse across scenes. A scene-level `camera_modes:` map (key → `CameraModeDef`) feels right — small and local to the scene.

- **Multiple cameras / split-screen**: **(resolved 2026-08-01, post plan-review — no longer
  hypothetical or open)**. Local co-op split-screen shipped and is load-bearing, heavily-used
  functionality (`local_coop_demo`, 9 rooms). See the "Local co-op / split-screen compatibility"
  section above for the 4 blocking questions this raised and their resolutions: `ActiveCameraMode`
  is a per-camera component, not a resource (Blocker 1); it's a distinct runtime type from the
  authored `CameraModeDef` (Blocker 2); camera-to-player ownership is a separate `CameraTargets`
  component, not buried per-variant (Blocker 3); `split:`/`own_viewport_only` move to a sibling
  field of `camera_mode:` under `components:`, not inside `Orbit`'s payload, with scene-level
  promotion logged as a v2+ candidate rather than attempted now (Blocker 4). `SetCameraMode`'s
  multi-camera targeting is resolved to `owner_player: Option<u32>` (reusing `ActionBarDef`'s
  existing convention), defaulting to "all active cameras" when omitted — see the Approach section.

- **Backwards compatibility**: **(resolved 2026-08-01; detection mechanism corrected 2026-08-07
  confirmation pass)** — serde aliasing is **not viable** (confirmed: `camera: (...)` is a
  named-field struct, `camera_mode: Orbit(...)` is an enum newtype variant; an alias renames a key,
  it cannot reshape a struct into a variant). The viable approach is **loader-side detection — but
  keyed on the prefab's `tags`, not on whether the old `camera:`/`flycam:` fields are present.**
  Both fields are optional tuning blocks (`#[serde(default)] pub camera: Option<CameraConfig>` in
  `schema/catalog.rs`); field-presence detection was verified to leave 8 of 10 player-bearing
  projects and 2 of 3 flycam projects (every one that omits the block and relies on engine
  defaults) cameraless. Corrected rule: `tags: ["flycam"]` + no `camera_mode:` → synthesize
  `Flycam(...)` (from `flycam:` if present, else defaults); `tags: ["player"]` + no `camera_mode:`
  → synthesize `Orbit(...)` (from `camera:` if present, else `default_camera_config()`). See the new
  Migration task in the Tasks list for the accompanying old→new RON table and the two-project
  migration proof (`3rd_person_game_demo` fully migrated, plus one `local_coop_demo` room pair).

- **Target entity for `Orbit` / `Follow` / `FirstPerson`**: **resolved** — these modes accept an optional `target_entity: String` (prefab instance name). If omitted, the engine defaults to the player entity. This allows a designer to track any named entity (NPC, prop) without code changes.

- **Input suppression during transitions**: **resolved** — all player camera input is suppressed while a `CameraBlendState` is active. Designer controls feel via `duration_secs`; keep blends ≤0.4 s for gameplay transitions. An `allow_input_during_transition: bool` field can be added to `CameraTransition` later if a real project hits the "locked out" complaint.

- **Interrupted transitions**: **resolved** — if `SetCameraMode` fires while a blend is in progress, the new transition starts from the current interpolated camera position. This keeps motion smooth regardless of how quickly modes are switched.

- **`Fixed` look_at_entity**: **resolved** — `Fixed` accepts either `look_at: (x, y, z)` (static world point) or `look_at_entity: "name"` (tracked moving target). At runtime the system resolves the name to an entity each frame, so the camera keeps pointing at the target as it moves.


---

## Acceptance criteria

- Given a scene with `camera_mode: Fixed(...)`, the camera spawns at the specified world position looking at the specified target, with no player input moving it.
- Given a scene with `camera_mode: Orbit(...)`, behaviour matches the current `OrbitCamera` with equivalent parameters.
- Given a logic rule `do_actions: [SetCameraMode("my_fixed")]`, the camera transitions from its current position to the fixed position over `transition.duration_secs` seconds using the specified ease curve.
- Given an instant cut (no `transition` field), the camera snaps to the new mode position in the same frame.
- Given `camera_mode: Follow(...)`, the camera tracks the target entity at the configured offset with no free orbit input; `smoothing` controls how quickly it catches up.
- Given `camera_mode: FirstPerson(...)`, the camera is locked to the target's head position and yaw rotates with the character; mouse look controls pitch only.
- Given a mode with `fov: 90.0`, the spawned camera uses that field-of-view; during a transition to a mode with a different FOV, the FOV interpolates linearly alongside the transform blend.
- Given a prefab with the old `camera:` field (no `camera_mode:`), the engine still spawns an orbit camera with the old parameters — no migration required for existing projects.
- Given a prefab with the old `flycam:` field, the engine still spawns a flycam — no migration required.
- Given a `tags: ["player"]` prefab with **no** `camera:` block at all and no `camera_mode:` (the
  majority shape — `quick_scene`, `primitive_world`, `entity_logic_demo`, and 5 others), the engine
  still spawns a default orbit camera — added 2026-08-07 confirmation pass, since this is the
  common case the original two criteria above didn't cover.
- Given a `tags: ["flycam"]` prefab with **no** `flycam:` block at all and no `camera_mode:`
  (`terrain_demo`, `custom_materials`), the engine still spawns a default flycam.
- Given any existing `local_coop_demo` room (`Vertical`/`Horizontal`/`Grid`/`dynamic`/`party`),
  when this ships, then split-screen viewport layout, per-viewport HUD labels, target rings
  (including `own_viewport_only` mode), and camera-priority-based selection (click-to-select,
  particle billboarding, nameplate culling) all behave identically to before this refactor —
  regression, verified by the full existing `local_coop_tests.rs` suite passing unchanged, not
  just this feature's own new tests (**browser-observable**: replay this session's local-coop
  playtest checklist across a representative sample of rooms).
- Given a `party:` scene, when `Action::CameraShake` fires, then the shared `PartyOrbitCamera`
  (now `CameraModeDef::Party`) actually shakes — closing the documented pre-existing gap, not just
  preserving it.
- Given a `split:` scene with 2+ simultaneously active cameras, when `Action::CameraShake` fires,
  then every active split camera shakes, not just one.
- Given a `flycam:` scene, when `Action::CameraShake` fires, then it still `warn!`s ("no orbit
  camera in scene — shake ignored") exactly as it does today — **not** a silent no-op, which is
  what a naive variant-unfiltered query would produce once `fly_camera_system` runs after the
  shake write and overwrites it.
- Given the non-hot-join `Action::Spawn`/`spawn_player_entity` fallback camera path in an
  `own_viewport_only` scene, when this ships, then that camera still gets **no** `RenderLayers`
  component and the existing warn still fires — the specific non-insertion case the three-spawn-
  path collapse risks losing, verified by a named test, not inferred from the insertion-site tests.
- Given `local_coop_demo`, when the migrated project (`3rd_person_game_demo`) and the deliberately-
  unmigrated project (`local_coop_demo`, with its own RON comment explaining why) are both played,
  then both work identically — the compat path is proven by a real unmigrated project still
  loading correctly, not just by unit tests.

---

## Phase 2 — Cinematic mode

`Cinematic` (spline/keyframe camera) is deliberately deferred. It requires a timeline or sequencer primitive that doesn't exist yet (see backlog icebox: "Timeline / sequencer"). When that feature lands, `Cinematic` can be added as a new variant of `CameraModeDef` without changing the rest of this system. A separate feature file should be written at that point.
