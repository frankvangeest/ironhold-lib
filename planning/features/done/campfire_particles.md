# Feature: Campfire with Fire and Smoke Particles

_Status: Done_
_Planned at: `b8e6f87` (2026-05-16)_

## What

Adds a primitive campfire prop to `primitive_world` that continuously emits fire and smoke particle effects. The campfire is assembled from colored primitive shapes (no GLB needed) and drives itself entirely through a looping `.behavior.ron` file — no Rust code changes required. This also establishes the **self-sustaining loop pattern** using `EmitEventAfterDelay`, which is the canonical way to author any repeating ambient effect.

## Why

- The particle system (`Action::SpawnEffect`) was just added. A campfire is the natural first ambient use case that proves continuous effects work in production.
- Demonstrates the full data-driven loop pattern: a behavior file that re-queues its own event, creating perpetual animation without a dedicated Rust timer system.
- Adds environmental life to `primitive_world` with zero Rust changes — a clear demonstration of the engine's RON-only authoring goal.
- The campfire's two-layer design (fast fire bursts + slow smoke bursts) exercises the `SpawnEffect` action across two different effect profiles in the same entity.

## Approach

### Self-sustaining loop pattern

The campfire uses `EmitEventAfterDelay` to re-queue itself on each tick. Each event handler fires the effect **and** schedules the next tick, creating a perpetual loop that starts on behavior entry and runs until the entity is despawned:

```
entry_actions: EmitEventAfterDelay(event: "campfire.fire:{self}", delay_secs: 0.1)
     ↓
on campfire.fire:{self}:
  SpawnEffect(key: "campfire_fire", entity: "{self}")
  EmitEventAfterDelay(event: "campfire.fire:{self}", delay_secs: 0.2)   ← re-queues itself
     ↓ (0.2 s later)
on campfire.fire:{self}: [repeats forever]
```

Fire and smoke run as two independent loops with different intervals, staggered by initial delays so bursts don't land on the same frame.

### Effect definitions

**`campfire_fire`** — bright orange/amber wisps rising from the log center, decelerating as they climb:

```ron
"campfire_fire": (
    particle_count: 10,
    lifetime_secs: 0.5,
    speed: 1.8,
    speed_jitter: 0.5,
    spread_deg: 20.0,
    offset: (0.0, 0.2, 0.0),
    size: 0.10,
    size_end: Some(0.0),
    color_start: (1.0, 0.55, 0.05, 1.0),
    color_end:   (0.9, 0.08, 0.0,  0.0),
    gravity: -0.8,
),
```

- `spread_deg: 20.0` — tight upward cone (20° half-angle); particles stay in the flame column
- `gravity: -0.8` — gentle downward deceleration creates the "flame licks up then slows" arc
- `size_end: Some(0.0)` — wisps shrink to nothing as they fade (additive blend does the rest)
- Fire loop interval: 0.2 s → max ~25 particles alive at once per campfire

**`campfire_smoke`** — gray puffs rising above the flames, expanding as they drift upward:

```ron
"campfire_smoke": (
    particle_count: 3,
    lifetime_secs: 2.2,
    speed: 0.5,
    speed_jitter: 0.15,
    spread_deg: 12.0,
    offset: (0.0, 0.7, 0.0),
    size: 0.15,
    size_end: Some(0.45),
    color_start: (0.35, 0.30, 0.28, 0.7),
    color_end:   (0.55, 0.52, 0.50, 0.0),
    gravity: 0.4,
),
```

- `spread_deg: 12.0` — very narrow cone; smoke column stays vertical
- `offset: (0.0, 0.7, 0.0)` — spawns above the fire, not inside it
- `size_end: Some(0.45)` — expands 3× as it rises (smoke spreading out)
- `gravity: 0.4` — slow sustained upward drift (positive = upward buoyancy)
- Smoke loop interval: 0.8 s → max ~8 particles alive per campfire

**Particle budget (2 campfires):** ~66 particles total — well within WASM limits.

### Campfire prefab

Composite primitive prefab — stone ring, crossed logs, central ember glow:

```ron
campfire: (
    kind: "primitive",
    shape: Cylinder(radius: 0.38, half_height: 0.02),
    color: (0.10, 0.08, 0.06, 1.0),
    behavior: "behaviors/campfire.behavior.ron",
    children: [
        // Stone ring
        ( offset: ( 0.32, 0.06,  0.0),  shape: Sphere(radius: 0.10), color: (0.45, 0.42, 0.40, 1.0) ),
        ( offset: (-0.32, 0.06,  0.0),  shape: Sphere(radius: 0.11), color: (0.43, 0.41, 0.39, 1.0) ),
        ( offset: ( 0.0,  0.06,  0.32), shape: Sphere(radius: 0.10), color: (0.44, 0.42, 0.38, 1.0) ),
        ( offset: ( 0.0,  0.06, -0.32), shape: Sphere(radius: 0.09), color: (0.42, 0.40, 0.38, 1.0) ),
        // Crossed logs
        ( offset: (0.0, 0.07, 0.0), rotation_euler_deg: (15.0,  40.0, 0.0), shape: Box(half_extents: (0.25, 0.055, 0.055)), color: (0.28, 0.14, 0.06, 1.0) ),
        ( offset: (0.0, 0.07, 0.0), rotation_euler_deg: (15.0, -40.0, 0.0), shape: Box(half_extents: (0.25, 0.055, 0.055)), color: (0.24, 0.12, 0.05, 1.0) ),
        // Ember glow
        ( offset: (0.0, 0.12, 0.0), shape: Sphere(radius: 0.07), color: (1.0, 0.40, 0.05, 1.0) ),
    ],
),
```

### Behavior file (`behaviors/campfire.behavior.ron`)

```ron
(
    schema_version: 1,
    initial_state: "burning",
    states: [
        (
            name: "burning",
            entry_actions: [
                // Stagger fire and smoke so their bursts don't land on the same frame.
                EmitEventAfterDelay(event: "campfire.fire:{self}", delay_secs: 0.1),
                EmitEventAfterDelay(event: "campfire.smoke:{self}", delay_secs: 0.5),
            ],
            exit_actions: [],
            on: [
                (
                    event: "campfire.fire:{self}",
                    do_actions: [
                        SpawnEffect(key: "campfire_fire", entity: "{self}"),
                        EmitEventAfterDelay(event: "campfire.fire:{self}", delay_secs: 0.2),
                    ],
                ),
                (
                    event: "campfire.smoke:{self}",
                    do_actions: [
                        SpawnEffect(key: "campfire_smoke", entity: "{self}"),
                        EmitEventAfterDelay(event: "campfire.smoke:{self}", delay_secs: 0.8),
                    ],
                ),
            ],
        ),
    ],
    transitions: [],
)
```

### Scene placements

Two campfires — thematically placed in the existing scene:

| ID | Position | Rationale |
|----|----------|-----------|
| `campfire_goblin` | `(4.0, 0.0, -18.5)` | Between the two goblin guards; goblins would have a fire |
| `campfire_pond` | `(-4.0, 0.0, -25.5)` | South of the pond for a cozy lakeside campsite atmosphere |

```ron
// In scenes/main.scene.ron — entities list
( id: "campfire_goblin", prefab: "campfire", position: (4.0, 0.0, -18.5) ),
( id: "campfire_pond",   prefab: "campfire", position: (-4.0, 0.0, -25.5) ),
```

### WebGPU warmup

`campfire_fire` and `campfire_smoke` both use `AlphaMode::Add` — the same pipeline variant as all other effects. The existing warmup burst in `state_machine.ron` (`SpawnEffect(key: "hit_spark", ...)`) already pre-compiles this pipeline during the scene-load window. No additional warmup is needed.

## Tasks

- [x] Add `campfire_fire` and `campfire_smoke` effect defs to `assets/projects/primitive_world/assets.ron`
- [x] Add `campfire` prefab to `assets/projects/primitive_world/prefabs/prefabs.ron`
- [x] Create `assets/projects/primitive_world/behaviors/campfire.behavior.ron`
- [x] Add two campfire instances to `assets/projects/primitive_world/scenes/main.scene.ron`
- [x] Run `cargo test -p ironhold_core --test ron_validation` — 160 passed
- [x] Run `cargo test -p ironhold_core --test integration_tests` — 107 passed
- [ ] Run `python test_web.py --update-baseline primitive_world` — update screenshot baseline

**No Rust changes required.** All tasks are RON edits.

## Open questions

- **Point light**: Should the campfire emit a dynamic point light (warm orange glow on nearby geometry)? Point lights aren't in the schema yet — if added, they'd go on `PrefabDef`. Out of scope for this feature but worth tracking.
- **Wind drift**: Should smoke particles drift horizontally based on a wind direction? The current `EffectDef` has no directional offset field. A `drift: Option<(f32, f32, f32)>` field on `EffectDef` could add this — could be a follow-up enhancement.
- **Trigger zone warmth**: Should standing near a campfire grant a buff? Could wire `TriggerZone` + `ApplyModifier` in the behavior file. Intentionally out of scope here to keep the feature minimal.

## Acceptance criteria

- Given the `primitive_world` project is running, when the main scene loads, two campfires are visible in the world (goblin camp and pond areas).
- Given a campfire entity exists, when one second passes, fire particles are continuously rising from the log center and smoke particles are rising above them.
- Given both campfires are running, when checking particle counts, the total active particle count stays under 100 at all times.
- Given a fresh WASM build, when the main scene loads, there is no frame freeze when fire or smoke particles first appear (covered by existing warmup).
- Given the feature is implemented, no Rust files are modified — all changes are in RON asset files only.
