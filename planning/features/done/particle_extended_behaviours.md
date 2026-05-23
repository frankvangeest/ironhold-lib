# Feature: Particle System v2 — 5. Extended Particle Behaviours

_Status: Done_
_Planned at: `2cc61ca` (2026-05-19)_
_Reviewed at: `a16bd98` (2026-05-23) — approach rewritten for CPU pool renderer_
_Shipped at: `b6dc0f9` (2026-05-23)_
_Part of: see `planning/features/particle_system_v2.md` for the full v2 overview_

## What

Extends `LayerDef` with four new behaviour families:
1. **Rotation over lifetime** — billboard quads spin on their Z axis
2. **Non-uniform billboard scale** — independent width and height for flame tongues, shards
3. **New emitter shapes** — Ring, Sphere surface, Line, Arc (in addition to current disc)
4. **Velocity curves** — ease in/out deceleration profiles

## Why

These four capabilities unlock effects that are currently impossible:
- Orbiting rune particles (Ring emitter + rotation)
- Tall narrow flame tongues that don't look circular (non-uniform scale)
- Wide horizontal shockwave rings (Ring, large radius)
- Impact shards that slow on outward travel (EaseOut curve)
- Channeling beam particle stream (Line emitter)

## Approach

**Implementation note:** this feature targets the CPU pool renderer (shipped as feature 1),
which rebuilds quad vertex data on the CPU every frame. There is no per-instance GPU buffer
and no instanced particle shader. All rotation and scale changes are applied by modifying
the 4 quad corner positions computed during the mesh rebuild step. No shader changes are
needed.

All changes are in `LayerDef` fields, `PooledParticle` state, and the mesh rebuild step.
Emitter shapes replace the current `fibonacci_cone_dir` + `emit_radius` logic in the
spawn function.

### 1. Rotation over lifetime

```ron
rotation_start_deg: 0.0,
rotation_end_deg: 360.0,    // full spin over lifetime
// OR constant angular velocity (takes precedence if non-zero):
rotation_speed_deg: 120.0,  // degrees/second
```

`PooledParticle` gains a `rotation_rad: f32` field. The simulation tick updates it each
frame (interpolate start→end over lifetime, or add `rotation_speed_rad * dt`).

In the mesh rebuild step, when computing the 4 corner offsets for a quad, the current
code uses axis-aligned `±half_size` offsets. Rotation is applied by rotating those
offsets using `cos(rotation_rad)` / `sin(rotation_rad)`:

```rust
let (s, c) = rotation_rad.sin_cos();
let hw = half_size_x;
let hh = half_size_y;
// corners: TL, TR, BR, BL in local billboard space
let corners = [
    Vec2::new(-hw,  hh),
    Vec2::new( hw,  hh),
    Vec2::new( hw, -hh),
    Vec2::new(-hw, -hh),
];
let rotated = corners.map(|v| Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y));
```

The rotated corner offsets are then added to the particle world position to produce
final vertex positions.

### 2. Non-uniform scale

```ron
size_x: 0.30,      // width
size_y: 0.80,      // height (taller = flame tongue)
size_x_end: 0.10,  // optional — pinch at end
size_y_end: 0.10,
```

When `size_x` / `size_y` are set, they override the uniform `size` / `size_end`.
`PooledParticle` gains `size_x: f32` and `size_y: f32` fields (interpolated at spawn
from the layer def, same as the existing `size` interpolation). In the mesh rebuild step,
`half_size_x` and `half_size_y` replace the current `half_size` used for both axes.
The rotation code above already uses `hw`/`hh` independently.

### 3. Emitter shapes

New `EmitterShape` enum as an optional field on `LayerDef` (default: current disc):

```ron
emitter: Disc(radius: 0.16),                        // current default
emitter: Ring(radius: 1.2),                         // circle — all on circumference
emitter: Sphere(radius: 0.5),                       // uniform surface of sphere
emitter: Line(length: 2.0, axis: Y),                // vertical or horizontal beam
emitter: Arc(radius: 1.0, angle_deg: 120.0),        // partial ring (sweeping cast)
```

Replaces `fibonacci_cone_dir` + `emit_radius` dispatch in spawn function. Each variant
has its own deterministic position distribution (evenly spaced on circumference for Ring,
Fibonacci point cloud for Sphere, etc.).

### 4. Velocity curves

```ron
velocity_curve: Linear,     // default — constant speed
velocity_curve: EaseOut,    // fast start, decelerates (impact burst)
velocity_curve: EaseIn,     // slow start, accelerates (rising energy)
velocity_curve: Pulse,      // fast → slow → fast (orbit-like bob)
```

Applied as a multiplier to `particle.velocity` in the simulation tick, derived from
`elapsed / duration`.

## Tasks

- [x] Add `rotation_start_deg`, `rotation_end_deg`, `rotation_speed_deg` to `LayerDef`
- [x] Add `size_x`, `size_y`, `size_x_end`, `size_y_end` to `LayerDef`
- [x] Add `EmitterShape` enum + `emitter` field to `LayerDef`
- [x] Add `VelocityCurve` enum + `velocity_curve` field to `LayerDef`
- [x] Add `rotation_rad` field to `PooledParticle`; update simulation tick to advance it
- [x] Apply rotation in mesh rebuild: rotate 4 quad corner offsets by `rotation_rad`
- [x] Add `size_x`/`size_y` fields to `PooledParticle`; interpolate from layer def at spawn
- [x] Use `size_x`/`size_y` as independent half-extents in mesh rebuild corner computation
- [x] Implement all emitter shapes in the spawn distribution function
- [x] Implement velocity curve multiplier in simulation tick
- [x] Add particles_demo effects using new capabilities:
  - [x] Magic orbit using `Ring` emitter + `rotation_speed_deg`
  - [x] Frost shard using non-uniform scale (`size_x < size_y`)
  - [x] Explosion using `EaseOut` velocity curve
- [x] RON parse + round-trip tests for all new fields and enum variants
- [x] Update `docs/20_data_formats.md`

## Open questions

- **Ring emitter distribution**: evenly spaced on circumference, or random (hash-based)?
  Evenly spaced looks better for small counts (3–8 particles) and is deterministic.
- **`size_x` / `size_y` vs `size` precedence**: when both are set, `size_x`/`size_y` win.
  Validate that only one pair is used per layer.
- **`Pulse` curve formula**: `sin²(t * π)` gives a smooth fast-slow-fast profile.
  Consider whether the "slowest" point should be at `t=0.5` (mid-life) or configurable.

## Acceptance criteria

- A `Ring` emitter with `rotation_speed_deg: 90.0` produces a continuous orbiting ring effect
- `size_x: 0.2, size_y: 0.8` produces visibly tall-narrow quads (not square)
- `EaseOut` velocity curve produces burst-then-coast trajectory visually distinct from `Linear`
- All new fields default to off; existing RON files unchanged
