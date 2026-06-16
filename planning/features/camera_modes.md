# Feature: Camera Modes

_Status: Draft_
_Planned at: `ece80c1` (2026-05-05)_

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

```ron
// In a prefab's components block, or at scene level
camera_mode: Orbit(
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
),
```

```ron
camera_mode: Fixed(
    position: (20.0, 10.0, 0.0),
    // Either a static world point OR a named entity (not both):
    look_at: (0.0, 0.0, 0.0),
    // look_at_entity: "boss",   // tracks a moving entity each frame
    fov: 50.0,   // optional; narrower FOV suits cinematic fixed shots
    transition: (duration_secs: 0.6, ease: "EaseIn"),
),
```

```ron
camera_mode: Follow(
    offset: (0.0, 4.0, 8.0),
    look_at_offset: (0.0, 1.5, 0.0),
    smoothing: 8.0,         // position lerp speed — higher = snappier, 0 = instant
    rotation_smoothing: 6.0, // separate smoothing for look-at rotation
    fov: 75.0,               // optional, degrees; default 60
    transition: (duration_secs: 0.3, ease: "Linear"),
),
```

```ron
camera_mode: FirstPerson(
    eye_offset: (0.0, 1.7, 0.0),
    sensitivity: 0.002,
    min_pitch: -1.4,
    max_pitch: 1.4,
    fov: 90.0,   // optional, degrees; FPS games typically use 80–100
),
```

```ron
camera_mode: Flycam(
    speed: 20.0,
    fast_speed: 60.0,
    sensitivity: 0.002,
    look_button: "Right",
),
```

The existing `CameraConfig` and `FlyCamDef` structs become the inner payloads of `Orbit` and `Flycam` variants respectively — this is a backwards-compatible rename at the RON level if we add serde aliases.

#### `TransitionConfig` (new, shared sub-struct)

```rust
pub struct CameraTransition {
    pub duration_secs: f32,
    pub ease: EaseKind,   // Linear, EaseIn, EaseOut, EaseInOut
}
```

Every mode variant carries an optional `transition` field. When `SetCameraMode` fires, the system lerps `Transform` (position + rotation via `Quat::slerp`) over `duration_secs` from the old position to the new one. If `transition` is absent, the cut is instant.

#### `Action::SetCameraMode` (new action variant)

```rust
SetCameraMode(String),   // Named mode key (matches a CameraModeDef defined in prefabs/scene)
```

Designers fire this from logic rules:

```ron
// In state_machine.ron or rules.ron
on: "ui.button_pressed:enter_cutscene",
do_actions: [SetCameraMode("cutscene_fixed")],
```

Named modes are registered via the prefab catalog or a new `camera_modes` block at scene level (design decision: see Open Questions).

---

## Key Rust changes

1. **`schema/camera.rs`** (new file)
   - `CameraModeDef` enum with all mode variants
   - `CameraTransition` struct
   - Per-mode config structs (reuse existing `CameraConfig`/`FlyCamDef` as inner payloads)

2. **`capabilities/camera.rs`** (refactor)
   - Replace `OrbitCamera` + `FlyCamera` components with a single `ActiveCameraMode` component holding the current `CameraModeDef`
   - One unified `camera_system` that dispatches on the active mode
   - `CameraBlendState` component for in-progress transitions (start transform, target transform, elapsed, duration, ease fn)

3. **`schema/actions.rs`**
   - Add `SetCameraMode(String)` variant

4. **`runtime/action_executor.rs`**
   - Handle `SetCameraMode`: look up named mode, set `ActiveCameraMode`, insert `CameraBlendState`

5. **`runtime/scene_manager/scene_loader.rs`**
   - Replace the three-path camera spawn (orbit / flycam / fallback) with a single spawn of `ActiveCameraMode` from the resolved `CameraModeDef`
   - Keep backwards compat: if prefab has old `camera:` or `flycam:` fields, map them to `Orbit`/`Flycam` variants

---

## Tasks

- [ ] Write `CameraModeDef` enum and sub-structs in `schema/camera.rs`
- [ ] Add `SetCameraMode(String)` to `Action` enum and document it
- [ ] Implement `camera_system` in `capabilities/camera.rs` (Orbit, Follow, Fixed, FirstPerson, Flycam)
- [ ] Implement `CameraBlendState` transition lerp (position + slerp rotation)
- [ ] Update `scene_loader.rs` to resolve `CameraModeDef` from prefab/scene and spawn `ActiveCameraMode`
- [ ] Handle `SetCameraMode` in `action_executor.rs`
- [ ] Backwards compat: map existing `camera:` / `flycam:` RON fields to new variants
- [ ] Update `entity_logic_demo` or `quick_scene` with a camera-switch example
- [ ] Integration tests: mode switch fires correctly, transition completes, fallback camera spawns
- [ ] Docs: add camera modes section to `docs/20_data_formats.md` and `docs/30_runtime_events_and_logic.md`

---

## Notes

### Runtime player spawn and the default camera

When a player character is spawned at runtime via `Action::Spawn` (e.g. from a character-select screen), the orbit camera spawns as part of that path. If the scene has no player entity in its RON, no 3D camera exists until the spawn fires — which causes at least one black frame.

**Clean solution**: `Camera::is_active = false` on the default camera (Bevy supports this natively without despawning). A "fallback" scene camera can sit deactivated, then the orbit camera takes over at full priority (`Camera::order`) when it spawns. No despawn/respawn needed.

**Open design question for camera modes implementation**: should the fallback camera be a standard part of every scene that omits a player entity, or should the camera-modes system make the primary camera persistent across scene loads and simply switch its `ActiveCameraMode`? The persistent-camera approach avoids the one-black-frame problem entirely and is architecturally cleaner for scene transitions. Consider this when implementing the unified `camera_system`.

---

## Open questions

- **Named mode registry**: should named modes live in the prefab catalog, in a new `camera_modes:` block at scene level, or as inline RON in the action argument? Inline is simplest but prevents reuse across scenes. A scene-level `camera_modes:` map (key → `CameraModeDef`) feels right — small and local to the scene.

- **Multiple cameras**: some games render picture-in-picture or split-screen. Out of scope for phase 1 — `SetCameraMode` only affects the primary camera. Worth noting so the design doesn't close the door.

- **Backwards compatibility**: the old `camera:` and `flycam:` prefab fields should continue to work without migration. Two options: (a) serde `#[serde(alias = "camera")]` on the new `camera_mode` field and auto-wrap at deserialise time; (b) detect the old fields in the scene loader and synthesise the new component. Option (b) is more explicit but adds loader noise; option (a) requires the old and new shapes to be compatible.

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

---

## Phase 2 — Cinematic mode

`Cinematic` (spline/keyframe camera) is deliberately deferred. It requires a timeline or sequencer primitive that doesn't exist yet (see backlog icebox: "Timeline / sequencer"). When that feature lands, `Cinematic` can be added as a new variant of `CameraModeDef` without changing the rest of this system. A separate feature file should be written at that point.
