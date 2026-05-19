# Feature: Particle System v2 — Scalable Stylized Effects

_Status: Draft_
_Planned at: `2cc61ca` (2026-05-19)_

---

## What

A full rework of the particle effect pipeline to support rich, stylised spell and ability
effects — the kind seen in action RPGs, MMOs, and hero shooters — running smoothly in the
WASM web build even when dozens of players are casting simultaneously.

The system stays entirely data-driven: game designers author effects in RON files and
asset catalogs, with no recompilation required. A single well-authored RON effect
definition should be reusable across multiple game types and be presentable as a
shared prefab.

---

## Why

The current system works well for ambient effects (campfire, smoke, ambient particles)
but breaks down in three key scenarios:

1. **Spell-heavy encounters** — 40 active spell casters means hundreds of simultaneous
   particle bursts. Each particle is a separate ECS entity with its own material handle.
   That is O(N) draw calls and O(N) per-frame material updates. WebGPU is sensitive to
   draw call count; this is a hard performance wall.

2. **Expressive spell effects** — orbiting particles, spinning runes, growing AoE circles,
   coloured ground projections, synced point lights — none of these are in the current
   schema. Designers must approximate them with workarounds.

3. **Visual quality ceiling** — the current `FlameParticleMaterial` is the only custom
   shader path. Bloom, per-particle rotation, non-uniform billboard scale, sprite flipbooks,
   and glow gradients are not exposed. Effects look flat next to stylised references.

---

## Architecture overview

### Rendering: instanced GPU particles (the critical change)

The current model — one `Mesh3d` + unique `Handle<Material>` per particle — breaks
Bevy's automatic draw-call batching. The fix is a dedicated instanced particle renderer:

- All particles of the same effect type share **one mesh** and **one material asset**.
- Per-particle state (position, color, size, rotation, UV offset, age) is written into a
  **`Vec<ParticleInstance>` GPU buffer** each frame by a CPU simulation tick.
- A single instanced draw call renders the entire buffer.
- Bevy 0.18 supports this via `ExtractedInstances` / `RenderCommand` — the pattern is
  already used by Bevy's built-in sprite batching.

Target: 2 000 simultaneous billboard particles → ≤ 20 draw calls (one per unique
material variant: standard-additive, standard-blend, flame-distort, glow-gradient, decal).

The simulation (velocity integration, size lerp, color lerp, lifetime) remains CPU-side
in a normal `Update` system. The GPU only sees the final per-frame instance buffer.

### Effect definition: multi-layer composition

A single `EffectDef` gains a `layers: Vec<LayerDef>` field. Each layer is an independent
sub-emitter with its own sprite(s), particle count, lifetime, colour gradient, emitter
shape, and behaviour curve. Layers are spawned and simulated together as one unit.

This replaces the current workaround of firing multiple `SpawnEffect` actions per fire
tick. Designers compose a campfire by authoring one `"campfire"` effect with a body layer
and a core layer, not by coordinating two separate effect keys in the behavior file.

`SpawnEffect` still accepts a single key; the executor spawns all layers of that effect.

### Effect lights: temporary point lights tied to effect lifetime

`EffectDef` gains an optional `light` block. When present, a `PointLight` entity is
spawned at the effect origin when the effect fires, fades out over `fade_out_secs`, and
despawns. No extra actions required from the designer.

---

## Feature areas

### 1. Instanced particle renderer

**New:** `InstancedParticleMaterial` — a `Material` impl that reads per-instance data
(translation, color, size_xy, rotation_rad, uv_offset) from a vertex attribute buffer.
Replaces per-entity `StandardMaterial` + `FlameParticleMaterial` for all particles.
`FlameParticleMaterial` stays as a variant with the UV distort/scroll uniforms but is
also instanced.

Material variants (each is one pipeline, pre-warmed on scene load):
| Variant | Alpha | Use |
|---|---|---|
| `Additive` | `AlphaMode::Add` | Fire, magic glow, electricity |
| `Blend` | `AlphaMode::Blend` | Smoke, cloud, soft auras |
| `FlameDistort` | `AlphaMode::Add` | Animated UV flame / energy |
| `Glow` | `AlphaMode::Add` | Shader-generated radial gradient, no texture |

**RON:** transparent to designers — the engine picks the variant based on existing
`additive`, `uv_distort`, and a new `glow` bool.

**Files:** new `capabilities/particle_renderer.rs`, new `assets/shared/shaders/instanced_particle.wgsl`.

---

### 2. Multi-layer EffectDef

```ron
"campfire": (
  layers: [
    (   // outer body
      sprites: ["particle/flame_01", "particle/flame_02", "particle/flame_03", "particle/flame_04"],
      particle_count: 4,
      lifetime_secs: 1.0,
      speed: 0.0,
      emit_radius: 0.16,
      offset: (0.0, 0.22, 0.0),
      size: 0.65,
      color_start: (1.0, 0.52, 0.08, 0.0),
      color_mid:   (1.0, 0.42, 0.05, 1.0),
      color_end:   (0.55, 0.06, 0.0,  0.0),
      uv_distort: 0.50,
      uv_scroll_speed: 0.55,
      additive: true,
    ),
    (   // white-hot core
      sprites: ["particle/flame_05", "particle/flame_06"],
      particle_count: 2,
      lifetime_secs: 0.80,
      speed: 0.0,
      emit_radius: 0.06,
      offset: (0.0, 0.26, 0.0),
      size: 0.28,
      color_start: (1.0, 1.0,  0.88, 0.0),
      color_mid:   (1.0, 0.80, 0.18, 1.0),
      color_end:   (1.0, 0.28, 0.0,  0.0),
      uv_distort: 0.35,
      uv_scroll_speed: 1.00,
      additive: true,
    ),
  ],
  light: (
    color: (1.0, 0.55, 0.15),
    intensity: 8000.0,
    range: 6.0,
    fade_out_secs: 0.5,
  ),
)
```

The top-level `EffectDef` retains its flat fields as a shorthand for single-layer effects
(no migration required for existing assets.ron files). When `layers` is present and
non-empty, the flat fields are ignored.

---

### 3. Extended particle behaviour

#### Per-particle rotation over lifetime

```ron
rotation_start_deg: 0.0,
rotation_end_deg:   180.0,   // rotates half-turn over lifetime
// OR constant spin:
rotation_speed_deg: 120.0,   // degrees per second; overrides start/end if set
```

Billboard quads already have a transform; the per-frame simulation simply applies a
Z-axis rotation each tick. No shader change needed.

#### Non-uniform billboard scale (width × height)

```ron
size_x: 0.30,      // quad width
size_y: 0.80,      // quad height — taller than wide for flame tongues
size_x_end: 0.30,
size_y_end: 0.10,  // pinch to a point over lifetime
```

The existing `size` / `size_end` fields become aliases for uniform scale. When
`size_x` / `size_y` are present they take precedence and drive independent X/Y scale on
the billboard transform.

#### Emitter shapes

New `emitter` field on `LayerDef` (default: current disc behaviour):

```ron
emitter: Point,                          // current default — single point or disc
emitter: Ring(radius: 1.2),              // circle — particles orbit an origin
emitter: Sphere(radius: 0.5),           // uniform surface of sphere
emitter: Line(length: 2.0, axis: Y),    // vertical or horizontal beam
emitter: Arc(radius: 1.0, angle_deg: 120.0, axis: Y),  // partial ring (sweeping cast)
```

`Ring` and `Arc` combined with slow upward speed and rotation enables channeling-style
orbiting rune particles without any extra logic.

#### Velocity curve

```ron
velocity_curve: Linear,     // current default — constant speed
velocity_curve: EaseOut,    // fast start, decelerates (impact burst)
velocity_curve: EaseIn,     // slow start, accelerates (rising energy)
velocity_curve: Pulse,      // fast → slow → fast (orbit-like bob)
```

---

### 4. Dynamic effect lights

```ron
// On any EffectDef or LayerDef
light: (
  color: (1.0, 0.55, 0.15),   // warm orange
  intensity: 8000.0,
  range: 6.0,
  fade_in_secs: 0.1,
  fade_out_secs: 0.4,          // light fades after the burst
  // omit duration_secs to match effect lifetime
),
```

Implementation: `action_executor` spawns a `PointLight` entity with a `FadingLight`
component that the light-fade system ticks and despawns. No extra action required.

This gives every fire, explosion, and spell cast a matching coloured light halo with
near-zero designer effort.

---

### 5. Ground decals / AoE projections

A simple, WASM-friendly decal: a flat quad mesh slightly above the ground plane
(y = 0.01), rotated to face straight down, with a `DecalMaterial` that uses
`AlphaMode::Blend` and a texture key. Works on flat terrain; no true projection math
required.

```ron
// New action
ProjectDecal(
  key: "aoe_fire_circle",
  entity: "boss_01",          // or position: Some(...)
  radius: 3.0,
  duration_secs: 5.0,
  color: (1.0, 0.40, 0.10, 0.70),
  pulse_speed: 0.8,           // optional — pulsing opacity for active AoE
),
```

Decal textures go in `assets/shared/textures/decals/` — ring, filled circle, hex, rune,
splat, shockwave.

RON schema: `AssetCatalog.decals: HashMap<String, String>` (key → path), same pattern as
`textures`.

---

### 6. Bloom / HDR exposure in scene RON

Bevy 0.18 ships `BloomSettings` as a camera component. Expose it in `GameSceneV2`:

```ron
post_processing: (
  bloom: (
    intensity: 0.25,
    low_frequency_boost: 0.45,     // makes large bright areas glow softly
    high_frequency_boost: 0.10,    // fine bright specks bloom sharply
    threshold: 0.75,               // only pixels brighter than this bloom
    composite_mode: EnergyConserving,
  ),
),
```

Additive particles naturally blow out to white-hot above threshold, then bloom spreads
into a halo — this is the "vibrant glowing gradient" look from stylised games.
The default (omitting `post_processing`) leaves bloom off for backwards compatibility.

---

### 7. Sprite flipbook / sheet animation

Animated sprite sheets allow a single texture to drive a full frame sequence — useful
for impact splats, magic seals that draw themselves, and explosion blooms.

```ron
flipbook: (
  cols: 4,
  rows: 4,         // 4×4 = 16 frames
  fps: 24.0,
  loop: false,     // play once then despawn; true = loop for duration
),
```

The particle shader reads `uv_offset` from the per-instance data (already planned in the
instanced renderer) and the CPU simulation advances the frame each tick.

No new asset format — just a regular PNG sprite sheet. Sheets go in
`assets/shared/textures/particles/sheets/`.

---

### 8. Particle quality tiers and budget

**Quality tiers** — a global `ParticleQuality` resource with four levels:
`Minimal`, `Low`, `Medium`, `High` (default: `High`).

Per-layer, designers can author count overrides:

```ron
quality: (
  minimal:  1,    // always show at least something
  low:      2,
  medium:   4,
  high:     8,    // matches particle_count
),
```

If `quality` is omitted, the engine scales `particle_count` by a global multiplier
(Minimal = 0.25×, Low = 0.5×, Medium = 0.75×, High = 1.0×), rounded up with a minimum
of 1 — so the spell always fires *something*.

**Effect budget** — a `ParticleBudget` resource tracks live particle count.
When the budget is exceeded, new `SpawnEffect` calls for low-priority effects are
silently skipped. Priority is an optional per-EffectDef field:

```ron
priority: Player,    // Player | Npc | Ambient — default: Npc
```

With `Player > Npc > Ambient`, player abilities always render; ambient effects (smoke,
fireflies) shed first under load.

**Quality action** — exposes the setting to designers:

```ron
Action::SetParticleQuality(Low)   // can be called from rules.ron on scene load
```

---

### 9. Shared effect library

`assets/shared/effects/` — a catalog of reusable, parameterisable effects that any
project can reference. Initial set:

| Key | Description | Techniques used |
|---|---|---|
| `fire/campfire` | Stationary layered campfire | Multi-layer, UV distort/scroll, dynamic light |
| `fire/torch` | Small vertical flame | UV distort, tight emit_radius |
| `fire/explosion` | Burst sphere + flash | Sphere emitter, EaseOut curve, decal shockwave |
| `magic/arcane_nova` | Orbiting ring burst | Ring emitter, rotation, Glow variant |
| `magic/heal_pulse` | Rising star bloom | EaseIn, positive gravity, star sprites |
| `magic/channel_ring` | Orbiting rune orbit | Ring emitter, continuous loop, arc |
| `impact/frost_burst` | Ice shard hemisphere | Sphere emitter, EaseOut, non-uniform scale |
| `ambient/smoke_column` | Rising grey smoke | Blend, size growth, turbulence |
| `ambient/sparkles` | Gentle ambient sparkles | Low count, long life, glow variant |

Shared effects live in `assets/shared/effects/*.ron` (a new asset type: `SharedEffectDef`
or simply `EffectDef` loaded from the shared catalog). Projects reference them via their
key; their `assets.ron` can override individual fields:

```ron
// In project assets.ron
effects: {
  "boss_fire": SharedEffect(
    base: "fire/explosion",
    overrides: (
      light: ( color: (0.8, 0.2, 1.0), intensity: 15000.0 ),  // violet instead of orange
    ),
  ),
},
```

---

### 10. What textures, shaders, and animations are needed

#### Textures (additions to `assets/shared/textures/particles/`)

| Category | Needed |
|---|---|
| Energy / magic | `swirl_01`, `glow_ring_01`, `sparkle_01`, `orb_soft` |
| Impact | `impact_flash`, `shockwave_ring`, `smoke_puff_dark` |
| Ice | `shard_01`, `frost_crystal`, `ice_spray` |
| Lightning | `bolt_01`, `spark_01` |
| Healing | `leaf_01`, `star_soft`, `cross_glow` |
| Corruption | `tendril_01`, `dark_swirl` |
| Ground decals | `circle_filled`, `ring_thin`, `ring_thick`, `hex_rune`, `splat_01`, `shockwave` |
| Sprite sheets | `explosion_16f` (4×4), `impact_flash_9f` (3×3) |

All textures: white-on-black (or white-on-transparent), the colour comes from the
gradient in RON. This keeps textures reusable across any colour scheme.

#### Shaders (new or extended)

| Shader | Purpose |
|---|---|
| `instanced_particle.wgsl` | Core instanced billboard renderer; reads per-instance buffer |
| `instanced_flame.wgsl` | Instanced variant with UV distort + scroll uniforms |
| `glow_particle.wgsl` | Shader-generated radial gradient — no texture, just a soft glow circle |
| `decal_ground.wgsl` | Ground-projected flat quad; optional pulse uniform |
| `fading_light.wgsl` | Not a particle shader — drives the bloom halo around a `PointLight` |

The existing `custom_flame_particle.wgsl` is superseded by `instanced_flame.wgsl` but
kept for backwards compatibility until v2 is fully migrated.

#### Animations

No new animation clip format is needed. The existing animation system handles character
casting anims. What the particle system adds:

- **Flipbook frame advance** — CPU logic only, drives `uv_offset` in the instance buffer
- **Effect "intro" envelope** — already covered by the `color_start alpha=0.0` fade-in
- **Continuous loop effects** — handled by the behavior system (EmitEventAfterDelay loop)
- **Synchronised cast start** — the behavior file binds a `entity.interacted:{self}` or
  `animation.reached_frame:{self}:12` event to `SpawnEffect`, so the particle burst
  lands on a specific animation frame

---

## Integration with Bevy 0.18

| Concern | Approach |
|---|---|
| Instanced draw calls | `RenderCommand` + `DrawFunctions` with a custom `SpecializedRenderPipeline`; instance data uploaded via `Buffer` + `BindGroup` each frame |
| `BloomSettings` | Inserted on the `Camera3d` entity by `spawn_scene_v2` when `post_processing.bloom` is present in scene RON |
| Dynamic point lights | Standard `PointLight` component; `FadingLight` is an engine component, no Bevy plugin needed |
| WASM pipeline warmup | Each new material variant (Additive, Blend, FlameDistort, Glow, Decal) needs one warmup `SpawnEffect` on `scene.ready`; document in `CLAUDE.md` |
| `FixedUpdate` vs `Update` | Particle simulation stays in `Update` (visual-only, no physics coupling); only trigger zone and physics sensors stay in `FixedUpdate` |
| Material asset reuse | All particles of the same type share a single `Handle<InstancedParticleMaterial>` stored in a resource — eliminates per-particle asset allocation |

---

## WASM performance targets

| Scenario | Particle count | Target frame time |
|---|---|---|
| Campfire scene (ambient) | ~30 | < 1 ms GPU |
| Solo dungeon, 3 spells cast | ~150 | < 3 ms GPU |
| 10-player encounter | ~500 | < 6 ms GPU |
| 40-player raid | ~2 000 | < 16 ms GPU (60 fps) |

Achieved via: instanced rendering (one draw call per variant), particle budget (shed
low-priority effects under load), `Low` quality tier available (halves count), and
warmup pre-compilation (no mid-combat GPU stalls).

---

## RON schema changes summary

| Location | Change |
|---|---|
| `EffectDef` | Add `layers: Vec<LayerDef>` (optional; flat fields become single-layer shorthand) |
| `LayerDef` | New type: all current flat `EffectDef` fields + `emitter`, `rotation_start/end/speed_deg`, `size_x/y`, `size_x/y_end`, `velocity_curve`, `flipbook`, `quality`, `priority`, `glow` |
| `EffectDef` | Add top-level `light: Option<EffectLightDef>` |
| `AssetCatalog` | Add `decals: HashMap<String, String>` |
| `GameSceneV2` | Add `post_processing: Option<PostProcessingDef>` |
| `Action` | Add `ProjectDecal(...)` and `SetParticleQuality(...)` variants |
| New: `SharedEffectDef` | `base: String` + `overrides: EffectDefOverrides` for shared effect references |

All changes are additive. No existing RON files require migration.

---

## Suggested implementation order

1. **Instanced renderer** — most impactful; unlocks everything else. Implement the
   instanced material + CPU simulation tick in one PR; migrate existing effects to it.
2. **Multi-layer EffectDef** — consolidate campfire, torch, etc. from two SpawnEffect
   calls into one effect definition. Low risk, high designer UX improvement.
3. **Bloom in scene RON** — tiny scope, high visual impact.
4. **Dynamic effect lights** — straightforward, transforms every fire and explosion.
5. **Extended particle behaviour** (rotation, emitter shapes, velocity curves) — enables
   orbiting rune effects, beam particles, AoE rings.
6. **Ground decals** — enables AoE targeting circles and impact splats.
7. **Quality tiers + budget** — add after the instanced renderer so the budget math is
   operating on the final performance-correct system.
8. **Flipbook animation** — useful for impact effects; depends on instanced UV offset.
9. **Shared effect library** — build the RON catalog once several effects are polished;
   the first entries are the existing particles_demo effects promoted to `shared/`.

---

## Open questions

- **Instancing approach**: use Bevy's `ExtractedInstances` pattern (requires some render
  world boilerplate) or a simpler `StorageBuffer` + one `draw_indexed_indirect` call?
  The latter is simpler to write but `draw_indexed_indirect` is a WebGPU feature that
  requires explicit capability check.
- **SharedEffectDef loading**: should shared effects live in `assets/shared/effects/`
  as standalone RON files (loaded via `AssetServer`) or be inlined in a shared catalog?
  Standalone files are cleaner but add one more asset loading path.
- **Decal Z-fighting**: flat quads at y=0.01 will fight with terrain that isn't perfectly
  flat. Is a `polygon_offset` sufficient, or do we need a stencil-based approach?
- **Fading lights in raids**: 40 simultaneous point lights may be expensive. Should the
  budget system also cap light count, or rely on `intensity` distance falloff being
  sufficient?
- **Flipbook + UV distort**: can a particle use both a flipbook and `uv_distort` on the
  same sprite? (The UV offset would conflict.) Probably disallow via validation.

## Acceptance criteria

- A particles_demo scene with 8 effect stations runs at 60 fps in the WASM web build,
  confirmed by `test_web.py`.
- A scene with 40 simultaneous `SpawnEffect` calls (simulating a raid) stays above
  45 fps on a mid-range laptop GPU in Chrome with the WASM build.
- Setting `SetParticleQuality(Minimal)` reduces visible particles but never produces
  zero — every ability fires at least one particle.
- All new fields in `EffectDef` have default values; all existing `assets.ron` files
  parse and validate without modification.
- The campfire in particles_demo uses the multi-layer `EffectDef` format (single key,
  no paired SpawnEffect calls in the behavior file).
- Bloom is opt-in and off by default; existing scene screenshots are pixel-identical
  when `post_processing` is absent.
