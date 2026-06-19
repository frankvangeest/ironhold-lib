# Feature: Camera Shake

_Status: In Progress_
_Planned at: `b8723ec` (2026-06-19)_

## What

Adds `Action::CameraShake { duration_secs, intensity }` so designers can trigger a procedural position
shake on the active camera directly from any rule, state machine, or behavior RON file — no Rust changes
required to use it in a game project.

```ron
// behavior RON — shake on hit
CameraShake(duration_secs: 0.4, intensity: 0.15)
```

## Why

Hit-feel and explosion feedback require short camera jolts. Without this, designers have no way to
communicate impact weight; it currently requires Rust code changes or workarounds. Camera shake is a
high-value, low-cost "juice" primitive used in virtually every action game.

## Approach

### Schema (`schema/actions.rs`)
Add a struct variant:
```rust
CameraShake {
    duration_secs: f32,
    intensity: f32,
}
```

### Component (`capabilities/camera.rs`)
Add `CameraShakeState` component:
```rust
#[derive(Component)]
pub struct CameraShakeState {
    pub remaining: f32,   // seconds left
    pub duration: f32,    // initial duration (for decay curve)
    pub intensity: f32,   // peak displacement in world-space metres
}
```

The component is inserted on the orbit-camera entity by the executor. A single component supports
re-triggering: a new `CameraShake` action overwrites `remaining`/`intensity` if the new shake is
stronger, otherwise it extends duration.

### System (`capabilities/camera.rs`)
New `camera_shake_system` runs in `Update`, after `camera_orbit_system`. For every camera with
`CameraShakeState`:
- Decay `remaining` by `time.delta_secs()`; remove component when `remaining <= 0`
- Compute time-based displacement using two sine waves at co-prime frequencies:
  ```rust
  let t = time.elapsed_secs();
  let decay = (remaining / duration).sqrt();  // sqrt gives snappier initial impulse
  let x = (t * 37.0).sin() * intensity * decay;
  let y = (t * 53.0).sin() * intensity * decay * 0.5;
  cam_transform.translation += Vec3::new(x, y, 0.0);
  ```
- No `rand` dependency — pure deterministic math, WASM safe.
- The offset is applied **after** `camera_orbit_system` sets the orbital position so it is additive
  and doesn't interfere with zoom/pitch/yaw.

### Executor (`runtime/scene_manager/action_executor.rs`)
Match arm for `Action::CameraShake`:
- Query `(Entity, &mut CameraShakeState) With<OrbitCamera>` + `Commands`.
- If a camera entity already has `CameraShakeState`, pick max intensity and sum duration (capped at
  `3.0 s` to avoid runaway stacks).
- Otherwise insert a fresh `CameraShakeState`.
- If no orbit camera exists, log a warning and no-op (safe in flycam scenes).

### RON syntax (struct variant — named fields)
```ron
CameraShake(duration_secs: 0.4, intensity: 0.15)
```

## Tasks
- [x] Feature plan written
- [ ] Add `CameraShake` variant to `schema/actions.rs`
- [ ] Add `CameraShakeState` component + `camera_shake_system` to `capabilities/camera.rs`
- [ ] Add executor arm in `action_executor.rs`
- [ ] Wire shake into `3rd_person_game_demo`: fire on spider/snake kill
- [ ] Integration test: executor inserts `CameraShakeState`; system decays it
- [ ] Docs: `docs/20_data_formats.md` — new Action row
- [ ] CLI check

## Open questions
- None — approach is settled.

## Acceptance criteria
- Given a scene with an orbit camera, when `CameraShake(duration_secs: 0.5, intensity: 0.2)` fires,
  the camera oscillates visibly for ~0.5 s then returns to its normal orbital position.
- Re-triggering while a shake is active extends the effect without a visible snap.
- Flycam scenes do not crash; a warning is logged and no shake occurs.
- The action is usable in `rules.ron`, `state_machine.ron`, and `.behavior.ron` without recompiling.
