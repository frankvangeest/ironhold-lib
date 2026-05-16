# Feature: Particle Effect Spawning

_Status: Draft_
_Planned at: `98ca5d0` (2026-05-16)_

## What

Adds `Action::SpawnEffect { key, position, entity }` so designers can trigger short-lived
burst particle effects entirely from RON — no code changes needed to add a new effect type.
Effects are defined in `assets.ron` under an `effects` section, keyed by a designer-chosen
name. A game rule or behavior file references the key; the engine spawns the particles at
the resolved world position and fades them out over their lifetime.

```ron
// In a behavior file on hit:
Action: SpawnEffect(
    key: "hit_spark",
    entity: "{self}",
)

// In assets.ron:
effects: {
    "hit_spark": (
        particle_count: 12,
        lifetime_secs: 0.4,
        speed: 3.0,
        spread_deg: 180.0,         // 0 = straight up, 90 = hemisphere, 180 = full sphere
        offset: (0.0, 1.0, 0.0),  // spawn at chest height above entity origin
        size: 0.06,
        color_start: (1.0, 0.8, 0.2, 1.0),  // RGBA, linear sRGB
        color_end:   (1.0, 0.1, 0.0, 0.0),
        gravity: -4.0,             // negative = falls; -9.8 = Earth-like; 0 = floaty
    ),
}
```

## Why

Hit reactions, pickups, deaths, and heals all feel flat without visual feedback. The stat
system, behaviors, and per-entity events are already in place — particles are the missing
low-cost "juice" layer. This unblocks attack dummy polish and makes `primitive_world` a
usable showcase of the full gameplay loop.

## Out of scope for v1

Looping / continuous emitters (torch flames, smoke stacks, fountains) are not in v1. To
approximate, fire `SpawnEffect` repeatedly via `EmitEventAfterDelay` chains — though this
is wasteful for long-lived effects. Proper looping emitters are a follow-up feature.

## Approach

### Schema: `EffectDef` in `AssetCatalog`

New struct in `schema/catalog.rs`:

```rust
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EffectDef {
    /// Number of particles spawned. Validated at load time: must be ≤ MAX_PARTICLES_PER_EFFECT (256).
    #[serde(default = "default_particle_count")]  // 12
    pub particle_count: u32,
    /// Seconds until all particles are despawned.
    pub lifetime_secs: f32,
    /// Initial speed of each particle in m/s.
    #[serde(default)]
    pub speed: f32,
    /// Speed randomness: actual speed is in [speed - jitter, speed + jitter]. Default 0.0.
    #[serde(default)]
    pub speed_jitter: f32,
    /// Emission cone half-angle in degrees. 0 = straight up, 90 = hemisphere, 180 = full sphere.
    #[serde(default = "default_spread_deg")]  // 180.0
    pub spread_deg: f32,
    /// Spawn offset added to the resolved world position (entity origin or explicit position).
    /// Default (0.0, 1.0, 0.0) so effects appear at roughly chest height for a 1.8 m entity.
    #[serde(default = "default_offset")]
    pub offset: (f32, f32, f32),
    /// Radius of each particle sphere in metres at spawn. Default 0.06.
    #[serde(default = "default_size")]
    pub size: f32,
    /// Radius at end of lifetime (interpolated linearly from `size`). `None` = constant size.
    #[serde(default)]
    pub size_end: Option<f32>,
    /// RGBA colour at spawn (linear sRGB, alpha 0.0 = transparent).
    pub color_start: (f32, f32, f32, f32),
    /// RGBA colour at end of lifetime (interpolated linearly).
    pub color_end: (f32, f32, f32, f32),
    /// Y-axis acceleration in m/s². Negative = falls, positive = rises.
    /// Reference: -2.0 light sparks, -9.8 Earth-like, 0.0 floaty, +2.0 rising embers.
    #[serde(default)]
    pub gravity: f32,
}
```

Add to `AssetCatalog`:

```rust
#[serde(default)]
pub effects: HashMap<String, EffectDef>,
```

**Particle count policy:** `particle_count > MAX_PARTICLES_PER_EFFECT` (256) is rejected at
catalog load time with a clear parse error ("particle_count must be ≤ 256"). This matches the
existing `#[serde(deny_unknown_fields)]` discipline — fail loudly at load, not silently at
runtime. Document the cap in `docs/20_data_formats.md`.

### Schema: `Action::SpawnEffect`

In `schema/actions.rs`:

```rust
/// Spawn a named particle burst effect. The key must exist in `AssetCatalog.effects`.
/// Position resolution precedence:
///   1. `entity` (by SpawnId) + `EffectDef.offset`
///   2. `position` + `EffectDef.offset`
///   3. (neither given) — no-op with a warning logged
/// If both `entity` and `position` are given, `entity` wins and a warning is logged.
/// `{self}` substitution applies to the `entity` field in behavior files.
SpawnEffect {
    key: String,
    #[serde(default)]
    position: Option<(f32, f32, f32)>,
    #[serde(default)]
    entity: Option<String>,
},
```

**`{self}` substitution note:** The entity FSM interpreter's `rewrite_self` function must
handle the `entity` field on `SpawnEffect`. This is not automatic — it requires an explicit
match arm in `rewrite_self` (see task list). This is the same pattern as `ShowDamagePopup`
and `SetEntityVisible`.

### Capability: `capabilities/particle.rs`

**Components:**

```rust
#[derive(Component)]
pub struct Particle {
    pub velocity: Vec3,
    pub elapsed: f32,
    pub duration: f32,
    pub start_size: f32,
    pub end_size: Option<f32>,
    pub gravity: f32,
    pub color_start: Color,
    pub color_end: Color,
    pub mat_handle: Handle<StandardMaterial>,
}
```

**Resource (mesh cache):**

```rust
#[derive(Resource, Default)]
pub struct ParticleMeshCache {
    /// Shared unit-sphere mesh, created once at startup and reused by all particles.
    /// Scale is applied per-particle via Transform::scale.
    pub sphere: Option<Handle<Mesh>>,
}
```

**Startup system:** create a `Sphere { radius: 1.0 }` mesh and store the handle in
`ParticleMeshCache`.

**Per-frame system (`particle_system` in `Update`):**

```
for each Particle entity:
  elapsed += dt
  t = elapsed / duration  (clamped 0–1)
  velocity.y += gravity * dt
  transform.translation += velocity * dt

  // size lerp
  if size_end is Some(end):
    new_size = start_size.lerp(end, t)
    if (current_scale.x - new_size).abs() > 0.001: transform.scale = Vec3::splat(new_size)

  // color lerp
  new_color = color_start.lerp(color_end, t)
  if RGBA delta > 0.01: material.get_mut(mat_handle).base_color = new_color

  if elapsed >= duration: commands.entity(e).despawn()
```

Change-detection guards on both `transform.scale` and `base_color` per project discipline.

**AlphaMode:** Use `AlphaMode::Add` (additive blending) as the default. Sparks and glow
effects look better additive, and it avoids depth-sorting artefacts in WASM that
`AlphaMode::Blend` can cause.

**Plugin:**

```rust
pub struct ParticlePlugin;
impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleMeshCache>()
           .add_systems(Startup, particle_startup_system)
           .add_systems(Update, particle_system);
    }
}
```

### Action executor arm

In `action_executor_system`:

```rust
Action::SpawnEffect { key, position, entity } => {
    // 1. Look up EffectDef
    let Some(def) = asset_catalog.0.effects.get(&key) else {
        warn!("Action::SpawnEffect: unknown effect key {:?}", key);
        continue;
    };
    let offset = Vec3::from(def.offset);

    // 2. Resolve world position
    let world_pos: Option<Vec3> = if let Some(entity_id) = &entity {
        if position.is_some() {
            warn!("SpawnEffect {:?}: both entity and position given; entity wins", key);
        }
        spawn_params.registry.entities.get(entity_id)
            .and_then(|e| scene_state.global_transforms.get(*e).ok())
            .map(|gt| gt.translation() + offset)
    } else {
        position.map(|(x, y, z)| Vec3::new(x, y, z) + offset)
    };

    let Some(origin) = world_pos else {
        warn!("SpawnEffect {:?}: no entity or position resolved; skipping", key);
        continue;
    };

    // 3. Spawn particles
    let sphere = particle_mesh_cache.sphere.clone().unwrap();
    for i in 0..def.particle_count {
        let dir = deterministic_cone_dir(i, def.particle_count, def.spread_deg.to_radians());
        let jitter = deterministic_jitter(i, def.speed_jitter);
        let velocity = dir * (def.speed + jitter);
        let mat = materials.add(StandardMaterial {
            base_color: color_from_tuple(def.color_start),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        });
        commands.spawn((
            Mesh3d(sphere.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(origin).with_scale(Vec3::splat(def.size)),
            LevelEntity,
            Particle {
                velocity,
                elapsed: 0.0,
                duration: def.lifetime_secs,
                start_size: def.size,
                end_size: def.size_end,
                gravity: def.gravity,
                color_start: color_from_tuple(def.color_start),
                color_end:   color_from_tuple(def.color_end),
                mat_handle: mat,
            },
        ));
    }
}
```

`MAX_PARTICLES_PER_EFFECT = 256` as a module constant; validated at catalog load time, not
clamped silently at runtime.

**RNG note:** `deterministic_cone_dir(i, count, spread_rad)` uses a Fibonacci sphere
distribution (no random state required — purely index-based) so direction sampling is
deterministic across runs. `speed_jitter` uses a simple per-index hash. When Beta 0.5
introduces the seeded deterministic RNG resource, these helpers can be replaced if needed.
Designer-facing docs will note: "Particle directions are deterministic — the same effect
with the same count always produces the same pattern."

### Demo in `primitive_world`

Three differentiated effects demonstrating the design space:

| Key | Description | Teaching the designer |
|---|---|---|
| `"hit_spark"` | Fast orange sparks, full sphere, heavy gravity, short life | `spread_deg: 180`, falling debris |
| `"heal_burst"` | Green sparkles, rising upward, narrow cone, longer life | `spread_deg: 30`, positive `gravity`, `size_end` fade |
| `"pickup_sparkle"` | White-to-blue floaty particles, slow, large `size`, long life | Low `speed`, near-zero `gravity`, growing `size_end` |

1. Add all three effect defs (with explanatory inline RON comments) to
   `assets/projects/primitive_world/assets.ron`.
2. Wire `hit_spark` into the attack dummy behavior on each `entity.damaged` event.
3. Wire `heal_burst` when health regenerates to full.
4. Wire `pickup_sparkle` to an existing collectible or scene event in `primitive_world`.
5. Screenshot baseline update for `primitive_world`.

### Documentation precedence table

Must appear in `docs/30_runtime_events_and_logic.md` under `SpawnEffect`:

| `entity` | `position` | Result |
|---|---|---|
| set | unset | Spawns at entity's world position + `offset` |
| unset | set | Spawns at explicit world position + `offset` |
| set | set | `entity` wins; `position` ignored (warning logged) |
| unset | unset | No-op (warning logged) |

Also document: "`{self}` substitution applies to the `entity` field when used inside a
`.behavior.ron` file. Particle directions are deterministic — the same effect always produces
the same pattern."

## Tasks

**Schema:**
- [ ] Add `EffectDef` struct to `schema/catalog.rs` with all fields above
- [ ] Add `effects: HashMap<String, EffectDef>` to `AssetCatalog`
- [ ] Add load-time validation: `particle_count > 256` → error with clear message
- [ ] Add `Action::SpawnEffect { key, position, entity }` to `schema/actions.rs`

**`{self}` substitution:**
- [ ] Verify (and if needed extend) `rewrite_self` in `message_interpreter.rs` to handle
      `SpawnEffect.entity` — same pattern as `ShowDamagePopup.entity`
- [ ] Update "Supported `{self}` targets in actions" list in `crates/ironhold_core/src/CLAUDE.md`

**Capability:**
- [ ] Create `capabilities/particle.rs` (`Particle` component, `ParticleMeshCache` resource,
      `particle_startup_system`, `particle_system`, `ParticlePlugin`)
- [ ] Implement Fibonacci-sphere `deterministic_cone_dir` and hash-based `deterministic_jitter` helpers
- [ ] Export from `capabilities/mod.rs`

**Executor:**
- [ ] `SpawnEffect` match arm in `action_executor_system`
- [ ] Register `ParticlePlugin` in `lib.rs`

**Demo:**
- [ ] Add `"hit_spark"`, `"heal_burst"`, `"pickup_sparkle"` (with inline RON comments) to
      `primitive_world/assets.ron`
- [ ] Wire `SpawnEffect` into attack dummy behavior and a pickup/scene event
- [ ] Update `primitive_world` screenshot baseline

**Tests:**
- [ ] RON validation: `EffectDef` round-trip (all fields, including `size_end`, `speed_jitter`, `offset`)
- [ ] RON validation: `SpawnEffect` action parse (with and without optional fields)
- [ ] RON validation: `particle_count > 256` → expect parse error
- [ ] RON validation: unknown effect key → expect warning path (integration test)

**Docs:**
- [ ] Add `EffectDef` field table to `docs/20_data_formats.md` (after AudioEntry section)
- [ ] Update the full `AssetCatalog` RON example in `docs/20_data_formats.md` to include an
      `effects: {}` block alongside `audio` and `materials`
- [ ] Add `SpawnEffect` to the Action reference in `docs/30_runtime_events_and_logic.md`,
      including the precedence table, `{self}` note, and determinism note
- [ ] Append `SpawnEffect` to the Action appendix list in `docs/30_runtime_events_and_logic.md`
- [ ] Update `docs/STATUS.md` Engine ABI section (new Action, new catalog field)
- [ ] Cross-link `EffectDef` entry to `primitive_world/assets.ron` examples
- [ ] Cross-link `SpawnEffect` entry to `primitive_world/behaviors/attack_dummy.behavior.ron`
- [ ] Extend `tools/asset_checker/check.py` to validate effect keys referenced in RON files
      against `assets.ron effects` entries

## Open questions

- **`global_transforms` in executor**: `SceneStateParams.global_transforms` is a
  `Query<&GlobalTransform>` — confirm it covers dynamically spawned entities. It should,
  since Bevy's transform propagation inserts `GlobalTransform` on all entities with
  `Transform`, but verify `SpawnRegistry` stores entity IDs in a way the query can reach them.
- **Fibonacci sphere vs golden-angle spiral**: Both are index-based and deterministic.
  Fibonacci gives better sphere coverage; golden-angle gives a good spiral pattern that can
  look intentional for some effects. Pick based on visual test during implementation.
- **Particle shape (future)**: Unlit sphere meshes are simple but billboard quads would batch
  better on GPU (fewer draw calls). Keep spheres for v1; track as a micro-optimisation.
- **Custom material support (future)**: Designers using `CustomMaterial` WGSL shaders elsewhere
  may want particle-specific shaders. Adding `material: Option<String>` (key into
  `AssetCatalog.materials`) in v2 would unlock this without a schema version bump.

## Acceptance criteria

- Given `effects: { "hit_spark": (particle_count: 12, ...) }` in `assets.ron`, when
  `Action::SpawnEffect { key: "hit_spark", entity: "dummy_01" }` executes, then 12 particles
  appear at `dummy_01`'s world position + `offset`, animate outward, fade to `color_end`,
  and self-despawn after `lifetime_secs`.
- Given an unknown key, when `SpawnEffect` executes, then a warning is logged and no crash occurs.
- Given `LoadScene`, when a new scene loads, then all in-flight particles are despawned (via `LevelEntity`).
- Given `particle_count: 300` in `assets.ron`, when the catalog loads, then the project fails
  to load with a clear error message naming the effect key and the limit.
- Given `entity: "{self}"` in a behavior file, when the entity FSM executes `SpawnEffect`,
  then particles appear at the correct entity's position (not world origin).
- Same direction pattern for same `particle_count` / `spread_deg` across runs (determinism).
- All new RON validation tests pass. All existing 150 tests continue to pass.
