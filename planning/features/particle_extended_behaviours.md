# Feature: Particle System v2 — 5. Extended Particle Behaviours

_Status: Draft_
_Planned at: `2cc61ca` (2026-05-19)_
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

All changes are in `LayerDef` fields and the CPU simulation tick. No shader changes are
needed for rotation or scale (handled by the billboard transform). Emitter shapes replace
the current `fibonacci_cone_dir` + `emit_radius` logic.

### 1. Rotation over lifetime

```ron
rotation_start_deg: 0.0,
rotation_end_deg: 360.0,    // full spin over lifetime
// OR constant angular velocity (takes precedence if non-zero):
rotation_speed_deg: 120.0,  // degrees/second
```

Each particle stores a `rotation_rad: f32` field. The simulation writes this into the
per-instance data each frame. The billboard shader reads it as a Z-axis rotation.

### 2. Non-uniform scale

```ron
size_x: 0.30,      // width
size_y: 0.80,      // height (taller = flame tongue)
size_x_end: 0.10,  // optional — pinch at end
size_y_end: 0.10,
```

When `size_x` / `size_y` are set, they override the uniform `size` / `size_end`.
Simulation writes `size_xy: [f32; 2]` into instance data. Shader reads for non-uniform
scale on the quad.

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

- [ ] Add `rotation_start_deg`, `rotation_end_deg`, `rotation_speed_deg` to `LayerDef`
- [ ] Add `size_x`, `size_y`, `size_x_end`, `size_y_end` to `LayerDef`
- [ ] Add `EmitterShape` enum + `emitter` field to `LayerDef`
- [ ] Add `VelocityCurve` enum + `velocity_curve` field to `LayerDef`
- [ ] Implement rotation in simulation tick (writes `rotation_rad` to instance data)
- [ ] Implement non-uniform scale in simulation tick (writes `size_xy`)
- [ ] Implement all emitter shapes in the spawn distribution function
- [ ] Implement velocity curve multiplier in simulation tick
- [ ] Add particles_demo effects using new capabilities:
  - [ ] Magic orbit using `Ring` emitter + `rotation_speed_deg`
  - [ ] Frost shard using non-uniform scale (`size_x < size_y`)
  - [ ] Explosion using `EaseOut` velocity curve
- [ ] RON parse + round-trip tests for all new fields and enum variants
- [ ] Update `docs/20_data_formats.md`

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
