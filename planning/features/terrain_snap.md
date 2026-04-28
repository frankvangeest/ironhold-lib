# Feature: Terrain Snap

_Status: Ready_
_Planned at: `91cd464` (2026-04-27)_

## What
A `snap_to_terrain: true` flag on any scene entity makes its authored Y coordinate a
height offset above the terrain surface rather than an absolute world position. The
engine samples the terrain heightmap at the entity's X/Z position and adjusts Y at
spawn time. If the scene has no terrain, or terrain is not yet ready, the entity falls
back to its authored Y with a warning.

Typical use: place rocks, trees, props, and NPCs in a scene without knowing the exact
terrain height at each X/Z — just set Y to 0 (flush with ground) or a small positive
offset (e.g. 0.5 to half-embed a rock).

## Why
Without this, terrain scene authors must manually look up the heightmap pixel value at
each entity's X/Z position, compute the world-space Y, and hard-code it in the scene
file. This is fragile: changing `height_scale` or terrain position breaks every
hand-tuned Y value. It also makes it impossible to scatter entities procedurally or
move terrain without re-authoring every entity transform.

## Approach

### 1. `TerrainHeightSampler` resource
When the heightmap image finishes loading (in `terrain_loading_system`, before the
async mesh task is dispatched), clone the pixel data and terrain parameters into a new
`TerrainHeightSampler` resource:

```rust
pub struct TerrainHeightSampler {
    pub data: Vec<u8>,      // RGBA8 pixel data
    pub width: usize,
    pub height: usize,
    pub height_scale: f32,
    pub horizontal_scale: f32,
    pub offset: Vec3,       // terrain world-space position
}
```

Expose a `sample(world_x: f32, world_z: f32) -> f32` method using bilinear
interpolation over the grid — the same formula already used by `generate_terrain_mesh_raw`:

```
grid_x = (world_x - offset.x + half_width)  / horizontal_scale
grid_z = (world_z - offset.z + half_height) / horizontal_scale
height  = bilinear(data, grid_x, grid_z) * height_scale + offset.y
```

The `TerrainHeightSampler` resource is inserted immediately when the image loads, so
it is available even before the async mesh generation completes.

### 2. Schema change
Add `snap_to_terrain` to `SceneEntityDef`:

```ron
(
  id: "rock_01",
  prefab: "rock_small",
  snap_to_terrain: true,          // Y becomes offset above terrain surface
  transform: (
    translation: (40.0, 0.0, -60.0),   // Y = 0 → flush with ground
    ...
  ),
)
```

Default is `false`, so no existing scenes are affected.

### 3. Spawn-time resolution
In `scene_loader.rs`, after computing `translation` for each entity, check
`snap_to_terrain`:

- **Terrain sampler available** (`TerrainHeightSampler` resource exists): resolve the
  snapped Y immediately and spawn at the adjusted position. No deferral needed.
- **Terrain sampler not yet available** (image still loading): add the entity def to a
  new `PendingTerrainSnap` resource (a `Vec<SceneEntityDef>` with its resolved
  prefab/material handles). A new system `terrain_snap_system` processes this queue
  each frame, draining it once `TerrainHeightSampler` is inserted.
- **No terrain in scene**: log a warning and spawn at the authored Y.

`PendingTerrainSnap` follows the same pattern as the existing `PendingPlayerConfig`
deferral for terrain-delayed player spawning.

### Timing note
`TerrainHeightSampler` is populated from the image data before the async task starts,
so in practice it will almost always be ready by the time most entity spawning happens.
The deferred queue is a safety net for the rare case where the heightmap image and the
scene RON load in the same frame.

## Tasks
- [ ] Add `TerrainHeightSampler` resource with `sample(x, z) -> f32` method to `terrain.rs`
- [ ] Populate `TerrainHeightSampler` in `terrain_loading_system` when heightmap image is ready
- [ ] Add `snap_to_terrain: bool` (default `false`) to `SceneEntityDef` in `scene_v2.rs`
- [ ] Resolve snapped Y in `scene_loader.rs`; collect deferred entities in `PendingTerrainSnap`
- [ ] `terrain_snap_system`: drain `PendingTerrainSnap` once sampler is available
- [ ] Warn + fall back to authored Y when no terrain in scene
- [ ] Tests: entity with `snap_to_terrain` lands at correct world Y for known heightmap pixel
- [ ] Docs: add `snap_to_terrain` to `20_data_formats.md` entity def section

## Open questions
- Should `snap_to_terrain` also affect the player spawn position? The player already has
  a separate terrain-ready deferral path; it could be unified with this system later.
- For NPC patrol waypoints expressed as relative offsets — should those also be
  terrain-snapped? Probably a follow-up; waypoints are a different code path.

## Acceptance criteria
- Given a scene with terrain and an entity at `translation: (40.0, 0.0, -60.0)` with
  `snap_to_terrain: true`, the entity spawns at the terrain surface height at X=40, Z=-60.
- Given `snap_to_terrain: true` with `translation: (0.0, 2.0, 0.0)`, the entity spawns
  2 units above the terrain surface.
- Given a scene with no terrain, the entity spawns at Y=0.0 and a warning is logged.
- Given no change to existing scenes (no `snap_to_terrain` field), behaviour is identical
  to before.
