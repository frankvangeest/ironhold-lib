# Feature: Particle System v2 — 6. Ground Decals / AoE Projections

_Status: Done — shipped at `d2c2860` (2026-05-24)_
_Planned at: `ff085be` (2026-05-19)_
_Reviewed at: `f46d462` (2026-05-23)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

A `ProjectDecal` action spawns a flat textured quad on the ground plane at a position or
attached to an entity, visible for a specified duration with optional pulsing opacity.
Designers use it for AoE targeting circles, persistent debuff zones, cast indicators,
and impact splats.

## Why

AoE ground circles are a fundamental visual element in ability-based games. Currently
there is no RON-driven way to place a persistent ground texture. Workarounds (invisible
plane prefabs) require pre-baking every possible shape into the scene. `ProjectDecal`
makes them dynamic and composable with effects.

## Approach

**Implementation note:** this feature uses a flat `Mesh3d` quad + `StandardMaterial`,
not Bevy's `ClusteredDecal`. `ClusteredDecal` was considered and rejected for this scope:
it requires `DepthPrepass` on the camera (extra render pass, unverified WebGPU cost) and
only pays off when decals must conform to sloped or curved surfaces. All current use cases
(AoE circles, cast indicators) live on flat ground, so the simpler path is the right
choice. If terrain-conformed decals become a concrete requirement in the future, that
should be a separate feature with a WebGPU compatibility verification.

**Decal mesh:** a flat `1m × 1m` quad in the XZ plane, face up (normal +Y), placed at
`y = 0.02` to float above the ground without Z-fighting. Scaled by `radius` in XZ.

**`DecalMaterial`:** a thin wrapper around `StandardMaterial` with:
- `unlit: true`
- `alpha_mode: AlphaMode::Blend`
- `depth_bias: 128.0` (pushes the quad forward in the depth buffer to avoid Z-fighting
  on slightly uneven surfaces)
- `base_color_texture`: the decal texture
- `base_color`: tinted by authored `color`

**`FadingDecal` component:** carries `duration_secs`, `elapsed`, and `pulse_speed`.

**`fading_decal_system` (Update):** ticks elapsed, applies pulse `alpha *= 0.7 + 0.3 * sin(elapsed * pulse_speed * TAU)`, updates `DecalMaterial::base_color`, despawns at end.

**New action:**

```ron
// In rules.ron or behavior file
ProjectDecal(
  key: "aoe_fire_circle",
  entity: "boss_01",        // decal follows entity XZ; use position: for static placement
  radius: 3.0,
  duration_secs: 5.0,
  color: (1.0, 0.40, 0.10, 0.70),
  pulse_speed: 0.8,         // optional; 0 = no pulse
),
```

**New catalog entry:**

```ron
// In assets.ron
decals: {
  "aoe_fire_circle": "shared/textures/decals/ring_thick.png",
  "cast_indicator":  "shared/textures/decals/circle_filled.png",
},
```

**Initial shared decal textures** (`assets/shared/textures/decals/`):
`circle_filled.png`, `ring_thin.png`, `ring_thick.png`, `splat_01.png`, `shockwave.png`
— all white-on-transparent, 256×256; colour comes from RON `color` field.

**Files:**
- `capabilities/decal.rs` (new)
- `schema/actions.rs` — add `ProjectDecal` variant
- `schema/catalog.rs` — add `decals: HashMap<String, String>` to `AssetCatalog`
- `runtime/scene_manager/action_executor.rs` — dispatch `ProjectDecal`

## Tasks

- [ ] Add `decals: HashMap<String, String>` to `AssetCatalog`
- [ ] Add `ProjectDecal` action variant with all fields
- [ ] Create `DecalMaterial` type (or configure `StandardMaterial` directly)
- [ ] Implement `FadingDecal` component + `fading_decal_system`
- [ ] Wire `ProjectDecal` into `action_executor_system`
- [ ] Create initial decal textures (5 PNGs, white-on-transparent, 256×256)
- [ ] Add decals to `particles_demo`:
  - [ ] Explosion pad: orange ring decal on trigger entry
  - [ ] Frost crystal: blue circle on interact
- [ ] Asset checker: validate `decals` paths exist on disk
- [ ] Integration test: `ProjectDecal` → quad entity exists → despawns at duration
- [ ] Update `docs/20_data_formats.md`

## Open questions

- **Entity-tracking form**: when `entity` is set, should the decal follow the entity's
  XZ position each frame? Useful for character aura effects. Requires a
  `TrackedDecal(entity_id)` component + position update in `fading_decal_system`.
- **Z-fighting on terrain**: `y = 0.02` + `depth_bias` works on flat ground; on sloped
  terrain quads may clip. If terrain support is needed, consider a separate slope-aligned
  decal pass. Defer until terrain + decal combo is actually needed.
- **Multiple decals per entity**: should a second `ProjectDecal` on the same entity
  replace or stack with the first? Stack (no unique constraint); the budget system (feature 7)
  caps total count.

## Acceptance criteria

- `ProjectDecal` spawns a visible textured ring/circle on the ground plane
- Pulse creates a visible heartbeat-like opacity oscillation
- Decal despawns at `duration_secs`; `LevelEntity` ensures cleanup on scene transition
- No Z-fighting against the flat demo ground
- `entity` form: decal XZ position tracks the named entity
