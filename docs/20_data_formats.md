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
The web runner uses a URL param: `index.html?project=quick_scene`.
Both default to `quick_scene` if nothing is specified.

---

## `{name}.project.ron` — ProjectConfig ✅

Entry point for a project. References all other files.

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | `u32` | ✅ | Must be `1` or `2` |
| `initial_scene` | `String` | ✅ | Path to starting scene, relative to project root |
| `project_id` | `Option<String>` | v2 | Machine-readable identifier |
| `display_name` | `Option<String>` | v2 | Human-readable name |
| `asset_catalog` | `Option<String>` | v2 | Path to `assets.ron` |
| `prefab_catalog` | `Option<String>` | v2 | Path to `prefabs/prefabs.ron` |
| `rules_path` | `Option<String>` | v2 | Path to `logic/rules.ron` |
| `model_fixes_path` | `Option<String>` | v1/v2 | Path to `overrides/model_fixes.ron` |
| `global_environment` | `Option<EnvironmentMapConfig>` | — | Project-wide fallback IBL lighting |
| `rules` | `Vec<LogicRule>` | v1 only | Inline rules (v1 only; use `rules_path` in v2) |
| `model_fixes` | `Map<String, TransformFix>` | v1 only | Inline fixes (v1 only; use `model_fixes_path` in v2) |

**Example (v2):**
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

    global_environment: Some((
        intensity: 400.0,
        fallback: Some((
            top_color: (0.1, 0.2, 0.4),
            bottom_color: (0.01, 0.01, 0.01),
        )),
    )),
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
| `lighting` | `Option<SceneLightingV2>` | Ambient + directional light config |
| `terrain` | `Option<TerrainConfigV2>` | Heightmap-based terrain |
| `spawn_points` | `Map<String, (f32,f32,f32)>` | Named world-space positions |
| `entities` | `Vec<SceneEntityDef>` | Prefab instances to spawn |
| `ui` | `Vec<UiButtonDefV2>` | Buttons to show in this scene |

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

### Lighting (`SceneLightingV2`)

- `ambient: Option<(f32, f32, f32)>` — linear RGB colour (intensity is set project-wide)
- `directional`: colour `(f32,f32,f32)`, `intensity: f32` (lux), `rotation_euler_deg: (f32,f32,f32)`, `shadows_enabled: bool` (default `true`)

### Terrain (`TerrainConfigV2`)

| Field | Type | Description |
|-------|------|-------------|
| `heightmap` | `String` | Path to greyscale PNG heightmap |
| `splatmap` | `String` | Path to RGBA splatmap (one channel per layer) |
| `scale` | `(f32, f32, f32)` | `(horizontal_x, height_multiplier, horizontal_z)` |
| `material_paths` | `Vec<String>` | Texture paths for up to 4 terrain layers |
| `chunk_size` | `u32` | Mesh chunk size in vertices (default `64`) |

Terrain generation runs on `AsyncComputeTaskPool` — do not block the main thread.

### UI Buttons (`UiButtonDefV2`) ✅

Buttons are rendered by Bevy UI inside the WebGPU canvas. They are **not** DOM elements — clicks in browser automation must use canvas pixel coordinates.

| Field | Type | Description |
|-------|------|-------------|
| `kind` | `String` | Must be `"button"` |
| `id` | `String` | Unique identifier within the scene |
| `text` | `String` | Label displayed on the button |
| `action` | `String` | Trigger string; `"ui."` prefix is stripped before firing (e.g. `"ui.dance"` → trigger `"dance"`) |
| `position` | `(f32, f32)` | Top-left corner in pixels `(x, y)` |
| `size` | `(f32, f32)` | Width and height in pixels |

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
      animation_policy: Some("prefabs/animation/player_policy.ron"),
      components: (
        tags: ["player"],
      ),
    ),
    "prop_anvil": (
      kind: "prop",
      model: "anvil",
      material: Some("wood_crate"), // overrides embedded material
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
| `kind` | `String` | `"actor"` or `"prop"` |
| `model` | `String` | Key into `AssetCatalog.models` |
| `animation_policy` | `Option<String>` | Path to `.ron` animation policy, relative to project root |
| `material` | `Option<String>` | Key into `AssetCatalog.materials` to override the model's material |
| `components.tags` | `Vec<String>` | Runtime tags; other component fields are design-time only |

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

**Event name format:** `"<domain>.<event_type>:<payload>"`. UI button events are always `"ui.button_pressed:<trigger>"` where `<trigger>` is the button's `action` field with the `"ui."` prefix stripped.

**Available actions:**

| Action | Description |
|--------|-------------|
| `LoadScene("path")` | Load a `.scene.ron` file relative to the project root |
| `Spawn("asset/path.glb#Scene0")` | Spawn a model by asset path |
| `PlayAnimation("id")` | Play an animation by semantic ID (see AnimationPolicy) |
| `PlaySound("key")` | Play a sound by audio catalog key (`.wav`, `.ogg`, `.mp3`); warns on missing key or unsupported format |
| `Log("message")` | Emit an `info!` log line |
| `Quit` | Exit the application |

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
