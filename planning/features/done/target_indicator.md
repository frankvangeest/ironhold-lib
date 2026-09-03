# Feature: Selected Target Indicator (Ground Decal)

_Status: Ready_
_Planned at: `0f79cc8` (2026-06-17)_

## What

When the player selects an entity (click or Tab), a configurable ground-circle decal appears under
it and tracks it as it moves. When the target is cleared or the entity dies, the decal disappears.
Designers configure the appearance (texture, radius, colour, Y offset) via an optional
`target_indicator:` block in scene RON — no code changes needed to enable or style it per project.

## Why

Targeting was shipped without visual feedback — the player has no in-world confirmation that an
entity is selected beyond the target-name UI label. A ground ring is the clearest, least intrusive
convention (WoW, Diablo, most ARPGs); it works at any camera angle and doesn't obscure the model.
`3rd_person_game_demo` is the immediate beneficiary, but the system is scene-generic.

## Approach

### Schema (`scene_v2.rs`)

Add an optional `target_indicator` field to `GameSceneV2`, following the same pattern as
`label_depth_scale`:

```rust
#[serde(default)]
pub target_indicator: Option<TargetIndicatorDef>,
```

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TargetIndicatorDef {
    /// Decal catalog key (from `assets.ron` `decals:` section).
    pub texture: String,
    /// Radius of the projected circle in metres. Default: 1.0
    #[serde(default = "default_indicator_radius")]
    pub radius: f32,
    /// RGBA tint applied to the decal texture. Default: white (1,1,1,1)
    #[serde(default = "default_indicator_color")]
    pub color: (f32, f32, f32, f32),
    /// How far above Y=0 the decal is lifted to avoid z-fighting. Default: 0.05
    #[serde(default = "default_indicator_offset_y")]
    pub offset_y: f32,
}
```

### Runtime (`capabilities/targeting.rs` or a new `target_indicator.rs`)

A `TargetIndicatorPlugin` adds a single `target_indicator_system` to `Update`:

1. **On `CurrentTarget` change** — read `LoadedTargetIndicator` resource (populated at scene load
   from `GameSceneV2.target_indicator`, similar to `LoadedLabelDepthScale`).
   - If `CurrentTarget` is `Some(id)`: look up the entity via `SpawnRegistry`. If the entity
     exists, spawn (or unhide) a `TargetIndicatorEntity` flat quad/decal child entity positioned
     below it at `translation + Vec3::Y * offset_y`. Attach a `TrackingTarget(Entity)` component
     so the follow system knows which entity to shadow.
   - If `CurrentTarget` is `None`: despawn or hide the indicator.
2. **Every frame** — for the live `TrackingTarget` entity, read the tracked entity's `GlobalTransform`
   and update the indicator's `Transform::translation` (XZ from target, Y fixed to `offset_y`).
   If the tracked entity no longer exists (entity died), despawn the indicator.

### New component

```rust
/// Marks the active target indicator entity. Tracks a specific world entity.
#[derive(Component)]
struct TrackingTarget(Entity);
```

### New resource

```rust
/// Populated at scene load from `GameSceneV2.target_indicator`.
/// None means no indicator is configured for the current scene.
#[derive(Resource, Default)]
pub struct LoadedTargetIndicator(pub Option<ResolvedTargetIndicator>);

pub struct ResolvedTargetIndicator {
    pub texture_handle: Handle<Image>,
    pub radius: f32,
    pub color: Color,
    pub offset_y: f32,
}
```

Populated in `scene_loader` alongside the other scene-load resources. Cleared on `LoadScene`.

### Decal rendering

The indicator is a flat, double-sided, unlit `StandardMaterial` quad (alpha blend mode) with the
configured texture. Spawned as a `Mesh3d` + `MeshMaterial3d` pair. No new shader needed — the
existing `StandardMaterial` with `alpha_mode: Blend`, `unlit: true`, and `double_sided: true` is
sufficient for a simple ring or circle texture.

The quad is **not** parented to the target entity (avoids inheriting animation scale transforms
from the GLB hierarchy); it is a top-level `LevelEntity` that the system repositions each frame.

### RON example (`3rd_person_game_demo/scenes/playing.scene.ron`)

```ron
target_indicator: (
    texture: "target_ring",
    radius: 1.2,
    color: (0.3, 0.8, 1.0, 0.7),
    offset_y: 0.05,
),
```

Requires `"target_ring"` in `assets.ron`:
```ron
decals: {
    "target_ring": "shared/textures/decals/target_ring.png",
}
```

A simple white ring PNG (transparent fill, white ring edge) lets the `color` tint do all the work.
`assets/shared/textures/decals/ring_thick.png` already exists and is a suitable placeholder.

## Tasks

- [ ] Add `TargetIndicatorDef` struct and `target_indicator` field to `GameSceneV2` in `schema/scene_v2.rs`
- [ ] Add `LoadedTargetIndicator` resource; populate it in `scene_loader` on scene load; clear on `LoadScene`
- [ ] Add `TrackingTarget` component and `target_indicator_system` (spawn/move/despawn decal on `CurrentTarget` change)
- [ ] Wire `TargetIndicatorPlugin` into `IronholdCorePlugin`
- [ ] Add `"target_ring"` decal entry to `3rd_person_game_demo/assets.ron` (use `ring_thick.png` placeholder)
- [ ] Add `target_indicator:` block to `3rd_person_game_demo/scenes/playing.scene.ron`
- [ ] Integration test: load a scene with `target_indicator:` set, emit `SetTarget`, assert `TrackingTarget` entity spawns; emit `ClearTarget`, assert it is despawned
- [ ] Docs: add `target_indicator` field to `GameSceneV2` reference table in `docs/20_data_formats.md`

## Open questions

- Should the indicator pulse (scale/opacity animation) in v1? Leaning no — static ring is clean and
  avoids adding a time-uniform parameter. Add a `pulse_speed` field in v2 if requested.
- What if the scene has no `decals:` section in `assets.ron`? Gracefully warn and skip indicator
  spawn (same pattern as `ProjectDecal` on an unknown key). No silent crash.
- Should the indicator also appear for mouse-hover (pre-select)? Deferred — hover feedback could be
  a different colour/texture, but adds complexity. V1: selected only.

## Acceptance criteria

- Selecting a target in `3rd_person_game_demo` shows a coloured ring decal on the ground under it.
- The ring moves with the entity as it patrols or is pushed.
- Pressing `ClearTarget` (or clicking empty space) removes the ring immediately.
- If the targeted entity dies and is despawned, the ring disappears within one frame.
- A scene RON without `target_indicator:` shows no ring and logs no error.
- WASM build is clean — no new non-wasm dependencies.
