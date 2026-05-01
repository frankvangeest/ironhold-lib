# Data Formats

> **Doc type:** Reference
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
✅ Schema v2 implemented and in use across all example projects.

---

## Rendering philosophy

The runtime prioritises **consistent, performant rendering across all supported platforms** over per-platform visual quality upgrades.

- **Web builds are the performance baseline.** Every rendering feature must work acceptably in a WebGPU WASM build. If a feature cannot meet that bar it is not enabled on any platform.
- **All platforms look the same.** There are no native-only graphics options. A scene authored once should render identically on desktop and in the browser.
- **Performance over visual fidelity.** When a trade-off must be made the engine chooses the faster option. Developers can push visual quality within the allowed feature set, but cannot opt in to slower paths.

**Excluded features and why:**

| Feature | Why excluded |
|---------|--------------|
| `TonyMcMapface` tonemapper | Requires a LUT texture lookup — additional bandwidth, slower, and web-unfriendly |
| `BlenderFilmic` tonemapper | Same as above |
| HDR | Adds memory and bandwidth overhead; consistent non-HDR output avoids per-platform clip differences |
| Bloom | Post-processing pass; not worth the GPU cost at the web performance baseline |

---

## Versioning ✅

All top-level data files include `schema_version: <u32>`. The runtime validates this on load and rejects unsupported versions. Both v1 and v2 are accepted where noted.

---

## Project folder layout ✅

Each game project lives under `assets/projects/{name}/`:

```
assets/projects/{name}/
  {name}.project.ron          ← ProjectConfig (entry point)
  assets.ron                  ← AssetCatalog  (model/texture/audio/material keys)
  prefabs/prefabs.ron         ← PrefabCatalog (named entity templates)
  prefabs/animation/*.ron     ← AnimationPolicy per character type
  scenes/*.scene.ron          ← GameSceneV2   (one file per scene)
  logic/rules.ron             ← LogicRulesAsset (event → action rules)
  overrides/model_fixes.ron   ← ModelFixesAsset (per-asset transform corrections)
```

The native runner selects a project by name: `cargo run -p ironhold_native -- --project quick_scene`.
The web runner uses a URL param: `play.html?project=quick_scene`.
Both default to `quick_scene` if nothing is specified.

---

## `{name}.project.ron` — ProjectConfig ✅

Entry point for a project. References all other files.

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | `u32` | ✅ | Must be `1`, `2`, or `3` |
| `initial_scene` | `String` | ✅ | Path to starting scene, relative to project root |
| `project_id` | `Option<String>` | v2+ | Machine-readable identifier |
| `display_name` | `Option<String>` | v2+ | Human-readable name |
| `asset_catalog` | `Option<String>` | v2+ | Path to `assets.ron` |
| `prefab_catalog` | `Option<String>` | v2+ | Path to `prefabs/prefabs.ron` |
| `rules_path` | `Option<String>` | v2 | Path to `logic/rules.ron` (rules workflow) |
| `state_machine_path` | `Option<String>` | v3 | Path to `logic/state_machine.ron` (FSM workflow; use instead of `rules_path`) |
| `model_fixes_path` | `Option<String>` | v1+ | Path to `overrides/model_fixes.ron` |
| `global_environment` | `Option<EnvironmentMapConfig>` | — | Project-wide fallback IBL lighting |
| `global_key_bindings` | `Map<String, String>` | — | Key name → trigger name (e.g. `"Escape": "toggle_pause"`) |
| `primitive_default_color` | `Option<(f32,f32,f32)>` | — | Default linear sRGB for all `kind: "primitive"` prefabs that omit their own `color`. Falls back to grey `(0.7, 0.7, 0.7)` when absent. |
| `rules` | `Vec<LogicRule>` | v1 only | Inline rules (v1 only; use `rules_path` in v2) |
| `model_fixes` | `Map<String, TransformFix>` | v1 only | Inline fixes (v1 only; use `model_fixes_path` in v2+) |

**Example (v2 — rules workflow):**
```ron
(
    schema_version: 2,
    project_id: Some("quick_scene"),
    display_name: Some("Quick Scene"),

    initial_scene: "scenes/main.scene.ron",

    asset_catalog: Some("assets.ron"),
    prefab_catalog: Some("prefabs/prefabs.ron"),
    rules_path: Some("logic/rules.ron"),
    model_fixes_path: Some("overrides/model_fixes.ron"),
)
```

**Example (v3 — FSM workflow):**
```ron
(
    schema_version: 3,
    project_id: Some("my_game"),
    display_name: Some("My Game"),

    initial_scene: "scenes/start_menu.scene.ron",

    asset_catalog: Some("assets.ron"),
    prefab_catalog: Some("prefabs/prefabs.ron"),
    state_machine_path: Some("logic/state_machine.ron"),
    model_fixes_path: Some("overrides/model_fixes.ron"),

    global_key_bindings: {
        "Escape": "toggle_pause",
    },
)
```

---

## `scenes/*.scene.ron` — GameSceneV2 ✅

Declaratively defines a scene: entities, UI, lighting, terrain, spawn points.
File extension must be `.scene.ron`.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | `u32` | Must be `2` |
| `name` | `String` | Scene identifier (used in `SceneEvent::Ready`) |
| `tonemapping` | `TonemappingOption` | Tonemapping for all cameras in this scene. Defaults to `AcesFitted`. See below. |
| `lighting` | `Option<SceneLightingV2>` | Ambient, directional, and point light config. See below. |
| `terrain` | `Option<TerrainConfigV2>` | Heightmap-based terrain |
| `spawn_points` | `Map<String, (f32,f32,f32)>` | Named world-space positions |
| `entities` | `Vec<SceneEntityDef>` | Prefab instances to spawn |
| `ui` | `Vec<UiElementDefV2>` | UI elements (buttons, labels, rects) to show in this scene |
| `ui_panel` | `Option<UiPanelDef>` | When set, UI elements are laid out in a centered panel box instead of absolute positioning |
| `scene_key_bindings` | `Map<String, String>` | Per-scene key overrides; same format as `global_key_bindings`. Cleared on each scene load. |
| `world_labels` | `Vec<WorldLabelDef>` | 3D world-space text labels that project to screen space and face the camera |
| `label_depth_scale` | `Option<LabelDepthScaleDef>` | When set, all labels shrink as camera distance increases. Individual labels can override with `depth_scale: Some(false/true)`. |

**Example:**
```ron
(
  schema_version: 2,
  name: "main",

  lighting: Some((
    ambient: Some((0.35, 0.35, 0.4)),
    directional: Some((
      color: (1.0, 0.98, 0.92),
      intensity: 12000.0,
      rotation_euler_deg: (-45.0, 35.0, 0.0),
    )),
  )),

  spawn_points: {
    "player_start": (0.0, 4.0, 0.0),
  },

  entities: [
    (
      id: "player_01",
      prefab: "player_warrior",
      transform: (
        translation: (0.0, 4.0, 0.0),
        rotation_euler_deg: (0.0, 0.0, 0.0),
        scale: (1.0, 1.0, 1.0),
      ),
    ),
    (
      id: "chest_01",
      prefab: "chest_01",
      transform: (
        translation: (5.0, 0.0, 3.0),
        rotation_euler_deg: (0.0, 45.0, 0.0),
        scale: (1.0, 1.0, 1.0),
      ),
    ),
  ],

  ui: [
    (
      kind: "button",
      id: "dance_button",
      text: "Dance",
      action: "ui.dance",
      position: (20.0, 60.0),
      size: (150.0, 40.0),
    ),
    (
      kind: "button",
      id: "quit_button",
      text: "Quit",
      action: "ui.quit",
      position: (20.0, 100.0),
      size: (150.0, 40.0),
    ),
  ],
)
```

### Tonemapping (`TonemappingOption`)

Applied to **all cameras** spawned for the scene (flycam, orbit camera, and fallback camera). Omit the field to get the default.

| Value | Style | Notes |
|-------|-------|-------|
| `AcesFitted` *(default)* | Cinematic, high-contrast | No LUT. Good for most 3D scenes. |
| `None` | Raw linear | Colours clip at 1.0. For flat / stylised scenes. |
| `Reinhard` | Smooth, muted | Can look washed out at high exposures. |
| `ReinhardLuminance` | Detail-focused | Preserves hue better than plain Reinhard. |
| `SomewhatBoringDisplayTransform` | Neutral, predictable | Minimal artistic flavour. |

`TonyMcMapface` and `BlenderFilmic` are not available — they require a LUT texture which reduces performance and is not compatible with the web baseline. See [Rendering philosophy](#rendering-philosophy).

### Lighting (`SceneLightingV2`)

**`SceneLightingV2` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `ambient` | `Option<(f32,f32,f32)>` | engine default | Ambient light colour as linear RGB |
| `ambient_brightness` | `Option<f32>` | `150.0` | Ambient brightness in lux. Without HDR colours clip at 1.0, so keep this low (50–300 is typical). |
| `directional` | `Option<DirectionalLightDefV2>` | none | A single directional (sun) light |
| `point_lights` | `Vec<PointLightDefV2>` | `[]` | Point (omnidirectional) lights |
| `shadow_map_size` | `Option<u32>` | `2048` | Texel resolution of the directional-light shadow atlas. Must be a power of two. Lower values (`512`, `1024`) improve GPU performance; higher values (`4096`) give sharper shadows on large scenes. |
| `point_shadow_map_size` | `Option<u32>` | `1024` | Texel resolution of each point-light shadow cube face. Same power-of-two rule applies. Only relevant when a point light has `shadows_enabled: true`. |

**`DirectionalLightDefV2` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `color` | `(f32,f32,f32)` | required | Linear RGB colour |
| `intensity` | `f32` | required | Illuminance in lux |
| `rotation_euler_deg` | `(f32,f32,f32)` | required | Euler angles in degrees (XYZ order) |
| `shadows_enabled` | `bool` | `true` | Whether this light casts shadows |
| `shadow_distance` | `Option<f32>` | engine default | Maximum world-unit distance at which shadow cascades are rendered. Tune downward for sharper shadows on a small scene; set to the full scene depth on large showcases. |
| `cascade_overlap` | `Option<f32>` | `0.2` | Fraction of each cascade's range that overlaps the next cascade (0.0–1.0). A wider overlap blends the transition zone so the seam between cascades is invisible. `0.5` eliminates most visible seam bands on large flat surfaces. |
| `num_cascades` | `Option<u32>` | `4` | Number of shadow cascade splits. Fewer cascades (`1`–`2`) reduce the GPU shadow pass cost significantly; more cascades give better shadow resolution over large distances. |

**`PointLightDefV2` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `position` | `(f32,f32,f32)` | required | World-space position |
| `color` | `(f32,f32,f32)` | `(1,1,1)` | Linear RGB colour |
| `intensity` | `f32` | `800.0` | Luminous power in lumens (≈ a bright 60 W bulb) |
| `radius` | `f32` | `0.0` | Sphere radius for specular highlights |
| `range` | `f32` | `20.0` | Maximum reach in world units |
| `shadows_enabled` | `bool` | `false` | Whether this light casts shadows (expensive — use sparingly) |

**Example (full lighting block):**
```ron
lighting: Some((
  ambient: Some((0.25, 0.30, 0.45)),
  ambient_brightness: Some(15.0),

  directional: Some((
    color: (1.0, 0.95, 0.85),
    intensity: 30000.0,
    rotation_euler_deg: (-45.0, 25.0, 0.0),
    shadows_enabled: true,
    shadow_distance: Some(450.0),
    cascade_overlap: Some(0.5),
  )),

  point_lights: [
    (
      position: (0.0, 15.0, -40.0),
      color: (0.5, 0.7, 1.0),
      intensity: 80000.0,
      range: 60.0,
    ),
  ],
)),
```

### Label depth scaling (`LabelDepthScaleDef`)

Controls how labels scale with camera distance. Set at scene level; individual labels can opt out.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `reference_distance` | `f32` | `50.0` | Camera distance at which labels render at their authored `font_size` (1:1). Labels further away shrink proportionally; labels closer stay at 1:1 (never grow larger). |
| `min_scale` | `Option<f32>` | `None` | Minimum scale floor as a fraction of `font_size` (0.0–1.0). `Some(0.25)` means labels never shrink below 25% of their authored size. `None` means no floor — labels scale toward zero at extreme distances. |

**Per-label override** — both `WorldLabelDef` and `EntityLabelDef` accept a `depth_scale: Option<bool>` field:
- `depth_scale: Some(false)` — pin this label at its authored size regardless of scene setting
- `depth_scale: Some(true)` — force depth scaling on even if the scene has no `label_depth_scale` block (uses `reference_distance: 50.0`, no floor)
- `depth_scale` omitted — inherits the scene setting (default)

**Example:**
```ron
label_depth_scale: Some((
  reference_distance: 80.0,
  min_scale: Some(0.25),
)),

// In entities — a nearby header pinned at full size:
label: Some((text: "Header", depth_scale: Some(false))),
```

### Terrain (`TerrainConfigV2`)

| Field | Type | Description |
|-------|------|-------------|
| `heightmap` | `String` | Path to greyscale PNG heightmap |
| `splatmap` | `String` | Path to RGBA splatmap (one channel per layer) |
| `scale` | `(f32, f32, f32)` | `(horizontal, max_height, horizontal_z)` — world units per heightmap pixel (X/Z) and max terrain elevation in world units (Y). E.g. `(5.0, 30.0, 5.0)` with a 128×128 heightmap gives a ~635×635 unit terrain with 30 units of elevation. |
| `position` | `Option<(f32,f32,f32)>` | World-space offset for the entire terrain mesh. Defaults to `(0, 0, 0)`. Set a negative Y to sink the terrain so player spawn points sit above the surface. |
| `material_paths` | `Vec<String>` | Texture paths for up to 4 terrain layers |
| `chunk_size` | `u32` | Mesh chunk size in vertices (default `64`) |

Terrain generation runs on `AsyncComputeTaskPool` — do not block the main thread.

### UI Elements (`UiElementDefV2`) ✅

UI elements are rendered by Bevy UI inside the WebGPU canvas. They are **not** DOM elements — clicks in browser automation must use canvas pixel coordinates.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `kind` | `String` | required | `"button"` — interactive; `"label"` — non-interactive text; `"rect"` — non-interactive coloured rectangle |
| `id` | `String` | required | Unique identifier within the scene |
| `text` | `String` | `""` | Display text (ignored for `kind: "rect"`) |
| `action` | `String` | `""` | For `kind: "button"`: trigger string; `"ui."` prefix is stripped (e.g. `"ui.dance"` → `"dance"`) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | required | Width and height in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.15,0.15,0.15,1)` | Fill/background colour as linear RGBA. No effect on `kind: "label"`. |
| `absolute` | `bool` | `false` | In panel mode: position this element absolutely relative to the panel's top-left instead of flowing in the column |
| `align` | `UiTextAlign` | `Center` | Horizontal text alignment for `kind: "label"`. Values: `Left`, `Center`, `Right`. Ignored for buttons and rects. |
| `bind` | `Option<String>` | `None` | For `kind: "label"`: name of a `GameVariables` key. When set, the label text is replaced every frame with the variable's current value. |
| `format` | `Option<String>` | `None` | Template used with `bind`. `"{}"` is replaced by the variable value (e.g. `"Score: {}"`). Defaults to the raw value when omitted. |

Click coordinates for browser tests: **center = `(position.x + size.w/2, position.y + size.h/2)`**.

---

## `assets.ron` — AssetCatalog ✅

Named registry of all assets available to prefabs and scenes.

```ron
(
  models: {
    "hero": ( path: "shared/models/character-01.glb#Scene0" ),
    "orc":  ( path: "shared/models/creatures/orc.glb#Scene0" ),
  },
  textures: {
    "grass": "shared/terrain/grass.png",
  },
  audio: {
    "click": "shared/audio/menu-button-click.wav",
  },
  materials: {
    "wood_crate": (
      kind: Standard((
        base_color_texture: Some("shared/textures/wood_crate_albedo.png"),
        metallic: 0.0,
        perceptual_roughness: 0.85,
      )),
      alpha_mode: Opaque,
      double_sided: false,
    ),
  },
)
```

**MaterialDef top-level fields** (apply to all kinds):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `kind` | `MaterialKind` | — | Required. `Standard(…)`, `Terrain(…)`, or `Custom(…)` |
| `alpha_mode` | `AlphaModeDef` | `Opaque` | `Opaque`, `Mask(f32)`, `Blend`, `Premultiplied`, `Add`, `Multiply` |
| `double_sided` | `bool` | `false` | Disables back-face culling. Use for geometry that must be visible from the inside (sky spheres, double-sided leaves, portals). Creates a separate GPU pipeline from the single-sided variant. |
| `unlit` | `bool` | `false` | Bypasses the lighting pipeline entirely — output colour is the raw shader result. Automatically adds `NotShadowCaster` to the entity so it cannot cast shadows. Required for additive emissive effects. |
| `uv_transform` | `Option<UvTransformDef>` | `None` | Offset, scale, and rotation applied to UVs before sampling |
| `tags` | `Vec<String>` | `[]` | Arbitrary string tags for runtime filtering |

**MaterialDef kinds:**
- `Standard(StandardMaterialDef)` — PBR material (base colour, textures, metallic, roughness, etc.)
- `Terrain(TerrainMaterialDef)` — splatmap + layer textures (WebGPU 16-byte alignment required)
- `Custom(CustomMaterialDef)` — shader path + arbitrary texture/float/colour uniforms

---

## `prefabs/prefabs.ron` — PrefabCatalog ✅

Named entity templates. Scenes reference prefabs by key; the runtime resolves the model via the AssetCatalog.

```ron
(
  prefabs: {
    "player_warrior": (
      kind: "actor",
      model: "hero",               // key in AssetCatalog.models
      animation_policy: "prefabs/animation/player_policy.ron",
      components: (
        tags: ["player"],
        movement: (
          walk_speed: 4.0,
          run_speed: 8.0,
          double_jump: true,
          collider_radius: 0.35,
          collider_height: 1.75,
        ),
        sounds: { "jump": "jump_sfx" },
      ),
    ),
    "prop_anvil": (
      kind: "prop",
      model: "anvil",
      material: "wood_crate",      // overrides embedded material
      components: (
        tags: ["prop"],
      ),
    ),
  }
)
```

**PrefabDef fields:**

| Field | Type | Description |
|-------|------|-------------|
| `kind` | `String` | `"actor"`, `"prop"`, or `"primitive"` |
| `model` | `String` | Key into `AssetCatalog.models`; for `kind: "primitive"` this is the Bevy shape name (see below) |
| `animation_policy` | `Option<String>` | Path to `.ron` animation policy, relative to project root |
| `material` | `Option<String>` | Key into `AssetCatalog.materials` to override the model's material |
| `components.tags` | `Vec<String>` | Runtime-meaningful tags: `"player"` and `"flycam"` affect spawning; others are design-time only |
| `components.movement` | `MovementConfig` | Movement tuning for player prefabs. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.sounds` | `HashMap<String, String>` | Map from event key to `AssetCatalog` audio key. `"jump"` plays on every player jump. |
| `primitive` | `Option<PrimitiveParams>` | Shape dimensions and appearance; only used when `kind: "primitive"` |
| `children` | `Vec<ChildPrimitiveDef>` | Sub-meshes composing a composite primitive (e.g. lamp post + orb). Only used when `kind: "primitive"`. See below. |
| `colliders` | `Vec<ColliderDef>` | One or more static physics colliders for `kind: "actor"` / `kind: "prop"`. All shapes are combined into a single Rapier compound body — use multiple entries to approximate curved geometry or multi-part shapes. Empty list = no physics. See below. |

### Special tag: `"flycam"` ✅

A prefab with `components.tags: ["flycam"]` and any `kind` spawns a free-flying camera instead of a model. The `model` field is ignored. The engine creates a `Camera3d` + `FlyCamera` component at the entity's transform.

**Controls:**
- **W/S** — forward / back
- **A/D** — strafe left / right
- **E / Space** — ascend
- **Q / LCtrl** — descend
- **LShift / RShift** — fast mode
- **Hold LMB or RMB + move mouse** — rotate view (mouse is free for UI when no button is held)

To display the camera's world position in the UI, add a label element with `id: "flycam_position"` to the scene's `ui` array. The engine will update it every frame.

```ron
// In prefabs/prefabs.ron
"flycam": (
  kind: "prop",
  model: "",
  components: ( tags: ["flycam"] ),
),

// In scenes/main.scene.ron — entity
(
  id: "camera_01",
  prefab: "flycam",
  transform: (
    translation: (0.0, 12.0, 0.0),
    rotation_euler_deg: (-25.0, 0.0, 0.0),
    scale: (1.0, 1.0, 1.0),
  ),
),

// In scenes/main.scene.ron — ui label
(
  kind: "label",
  id: "flycam_position",
  text: "",
  position: (16.0, 16.0),
  size: (300.0, 24.0),
),
```

### Special tag: `"player"` ✅

A prefab with `components.tags: ["player"]` spawns a third-person character controller with an orbit camera. Works on both `kind: "actor"` (GLB model) and `kind: "primitive"` (capsule shape). Movement is tuned via `components.movement`.

**`MovementConfig` fields** (all optional — defaults apply when omitted):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `walk_speed` | `f32` | `5.0` | Walking speed in m/s |
| `run_speed` | `f32` | `10.0` | Running speed in m/s (hold Shift) |
| `rot_speed` | `Option<f32>` | `3.0` | Yaw rotation speed in rad/s |
| `jump` | `Option<JumpConfig>` | own height | Jump height; see below |
| `double_jump` | `bool` | `false` | Enable a second jump while airborne |
| `double_jump_height` | `Option<JumpConfig>` | same as `jump` | Second-jump height |
| `collider_radius` | `Option<f32>` | `0.4` | Capsule collider radius (**GLB players only** — primitive players use `primitive.radius`) |
| `collider_height` | `Option<f32>` | `1.8` | Capsule total height (**GLB players only** — primitive players use `primitive.height`) |

**`JumpConfig` variants:**
- `Fixed(height: <f32>)` — absolute world-space height in metres (e.g. `Fixed(height: 2.5)`)
- `RelativeToHeight(percent: <f32>)` — fraction of the player's own height (e.g. `RelativeToHeight(percent: 100)`)

**Jump sound** — add `"jump"` to `components.sounds` to play a catalog audio key on every jump:
```ron
sounds: { "jump": "sfx_jump" }
```

### Primitive shapes ✅

When `kind: "primitive"`, no GLB model is loaded. Instead the runtime generates a procedural Bevy mesh from the `model` field (the Bevy shape name) and the optional `primitive` parameters block.

**Supported shape names:**

| `model` value | Shape | Key dimension fields |
|---|---|---|
| `"Cuboid"` | Box | `size: (x, y, z)` |
| `"Sphere"` | Sphere | `radius` |
| `"Cylinder"` | Cylinder | `radius`, `height` |
| `"Capsule3d"` | Capsule | `radius`, `height` (used as half_length) |
| `"Cone"` | Cone | `radius`, `height` |
| `"Torus"` | Torus / donut | `radius` (outer), `radius_top` (inner) |
| `"ConicalFrustum"` | Truncated cone | `radius` (bottom), `radius_top` (top), `height` |

**`PrimitiveParams` fields** (all optional — defaults apply when omitted):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `size` | `Option<(f32,f32,f32)>` | `(3,3,3)` | Cuboid XYZ extents |
| `radius` | `Option<f32>` | shape-specific | Sphere/Cylinder/Capsule/Cone/Torus outer/Frustum bottom |
| `radius_top` | `Option<f32>` | shape-specific | ConicalFrustum top radius; Torus inner radius |
| `height` | `Option<f32>` | shape-specific | Cylinder/Capsule/Cone/ConicalFrustum height |
| `color` | `Option<(f32,f32,f32)>` | project default | Linear sRGB. Priority: this field → `primitive_default_color` in project → grey `(0.7,0.7,0.7)` |
| `roughness` | `Option<f32>` | `0.5` | PBR perceptual roughness (0 = mirror, 1 = fully rough) |
| `metallic` | `Option<f32>` | `0.0` | PBR metallic factor (0 = dielectric, 1 = full metal) |
| `physics` | `bool` | `false` | Spawn a static `RigidBody::Fixed` Rapier collider (supported: Cuboid, Sphere, Cylinder) |
| `sensor` | `bool` | `false` | Spawn a ghost `Sensor` collider that fires `GameEvent::Trigger` on overlap (takes precedence over `physics`; supported: same shapes) |

**Example:**
```ron
(
  prefabs: {
    "marker_cube": (
      kind: "primitive",
      model: "Cuboid",
      components: (),
      primitive: Some((
        size: Some((2.0, 2.0, 2.0)),
        // color omitted — uses project primitive_default_color
        roughness: Some(0.4),
      )),
    ),
    "beacon_sphere": (
      kind: "primitive",
      model: "Sphere",
      components: (),
      primitive: Some((
        radius: Some(1.5),
        color: Some((0.9, 0.2, 0.2)),  // red override
        roughness: Some(0.2),
        metallic: Some(0.3),
      )),
    ),
  }
)
```

### GLB prop colliders (`colliders`) ✅

To make a GLB prop solid (so the player can stand on it or bump into it), add a `colliders` list to the prefab. All shapes are combined into a single Rapier compound `RigidBody::Fixed` — one entry for simple props, multiple entries to approximate curved geometry (arches, barrels) or multi-part shapes (chest base + lid). Works identically whether the prefab is a top-level scene entity or nested inside a composite prefab; in either case the prop's collider is an independent static body.

**`ColliderDef` fields** (each entry in the `colliders` list):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `shape` | `String` | — | `"Cuboid"`, `"Sphere"`, or `"Cylinder"` |
| `size` | `Option<(f32,f32,f32)>` | `(1,1,1)` | Full extents (width, height, depth) for Cuboid |
| `radius` | `Option<f32>` | `0.5` | Radius for Sphere / Cylinder |
| `height` | `Option<f32>` | `1.0` | Total height for Cylinder |
| `offset` | `(f32,f32,f32)` | `(0,0,0)` | Local-space offset of this shape from the entity origin |

```ron
// Simple single-shape prop
"barrel": (
  kind: "prop",
  model: "barrel",
  components: (),
  colliders: [
    (shape: "Cylinder", radius: Some(0.35), height: Some(0.9)),
  ],
),

// Multi-shape prop: chest with separate base and lid colliders
"chest_01": (
  kind: "prop",
  model: "chest_01",
  components: (tags: ["loot"]),
  colliders: [
    (shape: "Cuboid", size: Some((0.70, 0.55, 1.00)), offset: (0.0, -0.125, 0.0)),
    (shape: "Cuboid", size: Some((0.68, 0.28, 0.98)), offset: (0.0,  0.275, 0.0)),
  ],
),

// Archway approximated with three boxes
"archway": (
  kind: "prop",
  model: "archway",
  components: (),
  colliders: [
    (shape: "Cuboid", size: Some((0.4, 3.0, 0.4)), offset: (-1.5, 1.5, 0.0)),
    (shape: "Cuboid", size: Some((0.4, 3.0, 0.4)), offset: ( 1.5, 1.5, 0.0)),
    (shape: "Cuboid", size: Some((3.4, 0.4, 0.4)), offset: ( 0.0, 3.2, 0.0)),
  ],
),
```

**For composite primitive prefabs**, child shapes that have `physics: true` in their `PrimitiveParams` automatically add a collider to that child mesh and `RigidBody::Fixed` to the parent anchor — no `colliders` field is needed on the parent `PrefabDef`.

**Nested GLB props inside a composite** use their own `PrefabDef.colliders` field independently. The parent composite anchor gets `RigidBody::Fixed` only from inline primitive children that have `physics: true`; nested GLB props build their own separate static body. In practice both are `Fixed`, so the player interacts with each surface correctly.

### Composite prefabs (`children`) ✅

A primitive prefab with a non-empty `children` list spawns multiple child meshes under a single parent entity. Each child is a `ChildPrimitiveDef`, which is either an **inline primitive shape** or a **nested prefab reference** — set exactly one of `shape` or `prefab` per child.

**`ChildPrimitiveDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `shape` | `String` | `""` | Inline primitive shape name — same vocabulary as `model` for single primitives. Leave empty when using `prefab`. |
| `primitive` | `PrimitiveParams` | defaults | Shape dimensions and colour. Only used when `shape` is set. |
| `offset` | `(f32,f32,f32)` | `(0,0,0)` | Translation offset from the parent entity's origin |
| `rotation_euler_deg` | `(f32,f32,f32)` | `(0,0,0)` | Euler rotation in degrees (XYZ order) |
| `scale` | `(f32,f32,f32)` | `(1,1,1)` | Scale for this child |
| `material` | `Option<String>` | `None` | Key into `AssetCatalog.materials` for the child's material (inline primitives only). |
| `prefab` | `Option<String>` | `None` | **Nested prefab reference** — key into `PrefabCatalog.prefabs`. Mutually exclusive with `shape`. See below. |

The `material` field accepts the same custom/standard/terrain keys as the top-level `material` field, including `Custom` materials with WGSL shaders.

### Nested prefab references ✅

A child can reference another named prefab by key instead of defining an inline shape. All three prefab kinds are supported as nested children — `kind: "primitive"` (both composite and single-shape), `kind: "actor"`, and `kind: "prop"` (GLB meshes). Transforms compose **multiplicatively** (standard Bevy hierarchy), so rotation and scale inherit correctly at every nesting level.

```ron
"village": (
  kind: "primitive",
  model: "",
  components: (),
  children: [
    // Inline primitive — existing syntax, unchanged
    (
      shape: "Cuboid",
      material: Some("mat_stone_cobble"),
      primitive: (size: Some((18.0, 0.02, 14.0))),
      offset: (0.0, 0.01, 0.0),
    ),
    // Nested composite prefab (kind: "primitive" with children)
    (
      prefab: Some("well"),
      offset: (5.0, 0.0, 0.0),
      rotation_euler_deg: (0.0, 45.0, 0.0),
    ),
    // Nested GLB prop (kind: "prop" — loads a .glb file)
    (
      prefab: Some("rock_deco"),
      offset: (3.0, 0.0, -2.0),
      rotation_euler_deg: (0.0, 35.0, 0.0),
    ),
    // Nested single-shape primitive (kind: "primitive" with no children, just a model)
    (
      prefab: Some("beacon"),
      offset: (-6.0, 0.0, -4.0),
    ),
  ],
),
```

**What spawns for each nested prefab kind:**

| Nested prefab `kind` | Has `children`? | Result |
|---|---|---|
| `"primitive"` | yes | Anchor + all children spawned recursively |
| `"primitive"` | no (single `model`) | Anchor + one mesh child |
| `"actor"` / `"prop"` | — | GLB loaded via `spawn_prefab_instance`; the GLB root entity sits at the child `offset` |

**Rules and constraints:**

- `shape` and `prefab` are mutually exclusive. Setting both is a validation error.
- Every child must set exactly one of `shape` or `prefab`. Setting neither is a validation error.
- The referenced prefab key must exist in the same catalog — forward references are validated when the catalog loads.
- Circular references (`a → b → a`) are detected at validation time and rejected with an error.
- Nesting depth is capped at 8 levels in the spawner. Exceeding this logs an error and skips the deep child.
- If a nested `"actor"` / `"prop"` prefab's `model` key is missing from the asset catalog, a `load_errors` entry is emitted and the prefab is skipped — no panic.

**Transform composition (multiplicative):**

When a `well` prefab at `offset: (5, 0, 0)` has its own children (a cylinder at local `(0, 0.4, 0)` and a torus at `(0, 0.82, 0)`), those children land in the world at:
- cylinder → `village_origin + (5, 0, 0) + (0, 0.4, 0)` = `(5, 0.4, 0)` relative to village
- If the village anchor is rotated 45° around Y, all of the above rotates with it — including the well's inner parts.

**Scale inheritance caveat:**  Non-uniform scale on a parent entity (e.g., `scale: (2, 1, 1)`) combined with a rotation on a nested child causes shearing — the same limitation that applies in every 3D scene hierarchy. Keep scale uniform (or leave it at `(1, 1, 1)`) on any prefab that contains nested children.

---

## `prefabs/animation/*.ron` — AnimationPolicy ✅

Defines the locomotion clips and override animations for a character type.

```ron
(
    default_transition_ms: Some(150),

    base: (
        idle:      "Idle_Loop",
        walk:      "Walk_Loop",
        run:       "Sprint_Loop",
        jump_loop: "Jump_Loop",
    ),

    clips: {
        "dance": "Dance_Loop",   // semantic alias → glTF clip name
        "sit":   "Sitting_Idle_Loop",
    },

    overrides: [
        (
            id: "dance",
            clip: "Dance_Loop",
            priority: 50,
            looping: true,
            cancel_on_move: true,
            stop_action: Some("stop_dance"),
        ),
        (
            id: "attack_light",
            clip: "Sword_Attack",
            priority: 100,
            looping: false,
            duration: Some(0.6),
            cancel_on_move: false,
        ),
    ],
)
```

**AnimationOverrideDef fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | — | Semantic ID used by `PlayAnimation("<id>")` |
| `clip` | `String` | — | glTF animation clip name |
| `priority` | `i32` | `0` | Higher priority wins |
| `looping` | `bool` | `true` | Whether to loop |
| `cancel_on_move` | `bool` | `false` | Cancel this override when the player moves |
| `stop_action` | `Option<String>` | — | `PlayAnimation` ID that cancels this override |
| `duration` | `Option<f32>` | — | Auto-expire after N seconds (one-shots) |
| `transition_ms` | `Option<u64>` | — | Per-override blend duration; overrides `default_transition_ms` |

---

## `logic/rules.ron` — LogicRulesAsset ✅

Maps runtime events to action sequences. This is the primary place for data-driven game logic.

```ron
(
    schema_version: 2,
    rules: [
        (
            on: "ui.button_pressed:start_game",
            do_actions: [ Log("Starting"), LoadScene("scenes/main.scene.ron") ],
        ),
        (
            on: "ui.button_pressed:dance",
            do_actions: [ Log("Dance triggered"), PlayAnimation("dance") ],
        ),
        (
            on: "ui.button_pressed:quit",
            do_actions: [ Quit ],
        ),
    ],
)
```

**Event name format:** `"<domain>.<event_type>:<payload>"`. Available event names:

| Event name | Source |
|-----------|--------|
| `ui.button_pressed:<trigger>` | `UiEvent` — UI button or key binding; `<trigger>` is the button's `action` field with the `"ui."` prefix stripped |
| `<name>` (as-is) | `GameEvent::Trigger(name)` — gameplay capability (e.g. `"entity.collected:coin_01"`); the string is the full rule key, no prefix added |
| `scene.requested:<stem>` | Scene load initiated |
| `scene.loaded:<stem>` | RON asset deserialized; entities not yet spawned |
| `scene.ready:<stem>` | All entities spawned |
| `scene.unloading:<stem>` | Before a full scene replace |

`<stem>` is the filename without `.scene.ron` (e.g. `"main"` for `scenes/main.scene.ron`).

`GameEvent` naming convention: `"<category>.<verb>:<id>"` — e.g. `"entity.collected:coin_01"`, `"zone.entered:checkpoint_1"`.

**Available actions:**

| Action | Description |
|--------|-------------|
| `LoadScene("path")` | Load a `.scene.ron` file relative to the project root |
| `Spawn("asset/path.glb#Scene0")` | Spawn a model by asset path |
| `PlayAnimation("id")` | Play an animation by semantic ID (see AnimationPolicy) |
| `PlaySound("key")` | Play a sound by audio catalog key (`.wav`, `.ogg`, `.mp3`); warns on missing key or unsupported format |
| `Log("message")` | Emit an `info!` log line |
| `Quit` | Exit the application |
| `EnterState("name")` | Transition the interpreter to a named logic state; `""` returns to stateless |
| `SetVariable("key", "value")` | Write a named string variable into `GameVariables`; readable by data-bound UI labels |
| `IncrementVariable("key", i32)` | Parse the variable as `i32` and add the delta; missing or unparseable values default to `0` |

---

## `overrides/model_fixes.ron` — ModelFixesAsset ✅

Per-asset transform corrections applied to every spawned instance of a model. Use to fix off-centre pivots, wrong-axis imports, or scale mismatches.

```ron
(
    schema_version: 2,
    model_fixes: {
        "shared/models/character-01.glb#Scene0": (
            pivot_offset: (0.0, 0.0, 0.0),
            rotation_deg: (0.0, 180.0, 0.0),
            scale: (1.0, 1.0, 1.0),
        ),
    },
)
```

**TransformFix fields** (all optional, sane defaults):

| Field | Default | Description |
|-------|---------|-------------|
| `pivot_offset` | `(0,0,0)` | Metres; applied as child local translation |
| `rotation_deg` | `(0,0,0)` | Euler degrees, order **YXZ** |
| `scale` | `(1,1,1)` | Local scale |

Instances are spawned with a parent (instance transform) + child (GLB scene). The fix is applied to the child so gameplay transforms remain clean.

---

## `logic/state_machine.ron` — StateMachineAsset ✅

Used when `state_machine_path` is set in the project config (schema v3). Replaces `rules.ron` for FSM-based projects. See `docs/30_runtime_events_and_logic.md` for detailed FSM semantics.

**Top-level fields:**

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | `u32` | Must be `1` |
| `initial_state` | `String` | Starting logic state; set immediately when the asset loads |
| `states` | `Vec<FsmState>` | Named states |
| `transitions` | `Vec<FsmTransition>` | State-change triggers |
| `global_on` | `Vec<FsmEventBinding>` | Event bindings that fire from any state without changing state |

**`FsmState` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | State identifier |
| `entry_actions` | `Vec<Action>` | Queued when entering this state |
| `exit_actions` | `Vec<Action>` | Queued when leaving this state |
| `on` | `Vec<FsmEventBinding>` | In-state event bindings (do not change state) |

**`FsmTransition` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `from` | `Option<String>` | Source state; `None` matches any current state |
| `on` | `String` | Event that triggers this transition |
| `to` | `String` | Target state |

Execution order on transition: `exit_actions` of old state → state change → `entry_actions` of new state.

---

## Global environment lighting ✅

Can be set on `ProjectConfig.global_environment` and overrides per-scene lighting if a scene has no environment block.

```ron
global_environment: Some((
    intensity: 400.0,
    fallback: Some((
        top_color: (0.1, 0.2, 0.4),
        bottom_color: (0.01, 0.01, 0.01),
    )),
)),
```

- `intensity` — IBL strength
- `fallback` — procedural gradient sky used when no `.ktx2` cubemap is specified
- `asset_path` (optional) — path to a `.ktx2` cubemap for full IBL

---

## Schema evolution

When adding fields to any data type:
1. Add the field as `#[serde(default)]` in the Rust struct to keep old files valid.
2. Add an example to the relevant project under `assets/projects/`.
3. Add or update a test in `ironhold_core/tests/ron_validation.rs`.
4. Update this document and `docs/STATUS.md`.

Breaking changes (removing or renaming fields) require a `schema_version` bump.
