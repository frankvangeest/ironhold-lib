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
  behaviors/*.behavior.ron    ← per-entity FSM behavior files (optional)
  scenes/*.scene.ron          ← GameSceneV2   (one file per scene)
  logic/rules.ron             ← LogicRulesAsset (event → action rules)
  overrides/model_fixes.ron   ← ModelFixesAsset (per-asset transform corrections)
  stats/stats.ron             ← StatCatalog     (global named stat definitions; optional)
```

The native runner selects a project by name: `cargo run -p ironhold_native -- --project quick_scene`.
The web runner uses a URL param: `play.html?project=quick_scene`.
Both default to `quick_scene` if nothing is specified.

### File naming conventions

The engine identifies files by their path (as set in the project config) and by extension for a few special types. Here is the full convention:

| File type | Extension | Typical path |
|---|---|---|
| Project config | `.project.ron` | `{name}.project.ron` |
| Scene | `.scene.ron` | `scenes/{name}.scene.ron` |
| Entity behavior | `.behavior.ron` | `behaviors/{id}.behavior.ron` |
| Prefab catalog | `.ron` | `prefabs/prefabs.ron` |
| Animation policy | `.ron` | `prefabs/animation/{name}_policy.ron` |
| Asset catalog | `.ron` | `assets.ron` |
| Logic rules | `.ron` | `logic/rules.ron` |
| State machine | `.ron` | `logic/state_machine.ron` |
| Model overrides | `.ron` | `overrides/model_fixes.ron` |
| Stat catalog | `.ron` | `stats/stats.ron` |

`.scene.ron`, `.project.ron`, and `.behavior.ron` use a double extension so the engine can discover them by suffix. All other `.ron` files are found by their exact path, which is set in the project config — the filename itself is not significant to the runtime.

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
| `stats_path` | `Option<String>` | — | Path to a `stats.ron` file. When absent, the stat system is inactive for this project. |
| `damage_popup_style` | `Option<DamagePopupStyle>` | — | Visual style for `Action::ShowDamagePopup` popups. Omit for built-in defaults. See [DamagePopupStyle](#damagepopupstyle) below. |
| `rules` | `Vec<LogicRule>` | v1 only | Inline rules (v1 only; use `rules_path` in v2) |
| `model_fixes` | `Map<String, TransformFix>` | v1 only | Inline fixes (v1 only; use `model_fixes_path` in v2+) |

**Example (v2 — rules workflow):**
```ron
(
    schema_version: 2,
    project_id: "quick_scene",
    display_name: "Quick Scene",

    initial_scene: "scenes/main.scene.ron",

    asset_catalog: "assets.ron",
    prefab_catalog: "prefabs/prefabs.ron",
    rules_path: "logic/rules.ron",
    model_fixes_path: "overrides/model_fixes.ron",
)
```

**Example (v3 — FSM workflow):**
```ron
(
    schema_version: 3,
    project_id: "my_game",
    display_name: "My Game",

    initial_scene: "scenes/start_menu.scene.ron",

    asset_catalog: "assets.ron",
    prefab_catalog: "prefabs/prefabs.ron",
    state_machine_path: "logic/state_machine.ron",
    model_fixes_path: "overrides/model_fixes.ron",

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
| `ui` | `Vec<UiNodeDef>` | UI elements (buttons, labels, rects) to show in this scene |
| `ui_panel` | `Option<UiPanelDef>` | When set, UI elements are laid out in a centered panel box instead of absolute positioning |
| `scene_key_bindings` | `Map<String, String>` | Per-scene key overrides; same format as `global_key_bindings`. Cleared on each scene load. |
| `world_labels` | `Vec<WorldLabelDef>` | 3D world-space text labels that project to screen space and face the camera |
| `label_depth_scale` | `Option<LabelDepthScaleDef>` | When set, all labels shrink as camera distance increases. Individual labels can override with `depth_scale: false` or `depth_scale: true`. |

**Example:**
```ron
(
  schema_version: 2,
  name: "main",

  lighting: (
    ambient: (0.35, 0.35, 0.4),
    directional: (
      color: (1.0, 0.98, 0.92),
      intensity: 12000.0,
      rotation_euler_deg: (-45.0, 35.0, 0.0),
    ),
  ),

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
    Button((
      id: "dance_button",
      text: "Dance",
      action: "ui.dance",
      position: (20.0, 60.0),
      size: (150.0, 40.0),
    )),
    Button((
      id: "quit_button",
      text: "Quit",
      action: "ui.quit",
      position: (20.0, 100.0),
      size: (150.0, 40.0),
    )),
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
lighting: (
  ambient: (0.25, 0.30, 0.45),
  ambient_brightness: 15.0,

  directional: (
    color: (1.0, 0.95, 0.85),
    intensity: 30000.0,
    rotation_euler_deg: (-45.0, 25.0, 0.0),
    shadows_enabled: true,
    shadow_distance: 450.0,
    cascade_overlap: 0.5,
  ),

  point_lights: [
    (
      position: (0.0, 15.0, -40.0),
      color: (0.5, 0.7, 1.0),
      intensity: 80000.0,
      range: 60.0,
    ),
  ],
),
```

### Label depth scaling (`LabelDepthScaleDef`)

Controls how labels scale with camera distance. Set at scene level; individual labels can opt out.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `reference_distance` | `f32` | `50.0` | Camera distance at which labels render at their authored `font_size` (1:1). Labels further away shrink proportionally; labels closer stay at 1:1 (never grow larger). |
| `min_scale` | `Option<f32>` | `None` | Minimum scale floor as a fraction of `font_size` (0.0–1.0). `0.25` means labels never shrink below 25% of their authored size. Omitting `min_scale` means no floor — labels scale toward zero at extreme distances. |

**Per-label override** — both `WorldLabelDef` and `EntityLabelDef` accept a `depth_scale: Option<bool>` field:
- `depth_scale: false` — pin this label at its authored size regardless of scene setting
- `depth_scale: true` — force depth scaling on even if the scene has no `label_depth_scale` block (uses `reference_distance: 50.0`, no floor)
- `depth_scale` omitted — inherits the scene setting (default)

**Example:**
```ron
label_depth_scale: (
  reference_distance: 80.0,
  min_scale: 0.25,
),

// In entities — a nearby header pinned at full size:
label: (text: "Header", depth_scale: false),
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
| `uv_scale` | `f32` | UV tiling scale for layer textures. Higher values tile textures more finely. Defaults to `10.0`. |

Terrain generation runs on `AsyncComputeTaskPool` — do not block the main thread.

### Heightmap files

Heightmaps live at `projects/{name}/terrain/` and consist of two files:

| File | Purpose |
|------|---------|
| `heightmap.png` | Greyscale PNG — white = maximum elevation, black = sea level. The `scale.y` value in `TerrainConfigV2` maps the full white-to-black range to world units. Any image editor can produce one. |
| `heightmap.json` | Generation manifest written by `tools/texture_gen/generate.py`. The engine does **not** read this file — it is only used by the tool to regenerate the heightmap with tweaked parameters. Do not edit it by hand. |

**Generating a heightmap with the texture tool:**
```bash
python tools/texture_gen/generate.py --project my_game --type fbm --size 128 --seed 42
```
Run `python tools/texture_gen/generate.py --help` for all options. `tools/texture_gen/CLAUDE.md` describes every noise type and parameter.

**Using a hand-painted heightmap:** copy your greyscale PNG to `terrain/heightmap.png` and create a minimal manifest next to it so the tool knows where the file lives:
```json
{ "type": "custom", "output": "assets/projects/my_game/terrain/heightmap.png" }
```
The engine only reads the PNG — the JSON is for tooling only.

### UI Elements (`UiNodeDef`) ✅

UI elements are rendered by Bevy UI inside the WebGPU canvas. They are **not** DOM elements — clicks in browser automation must use canvas pixel coordinates.

Each element is a typed RON enum variant. Typos in field names fail at parse time with a clear error message.

#### `Button((...))`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `text` | `String` | required | Button label text |
| `action` | `String` | `""` | Trigger string; `"ui."` prefix is stripped (e.g. `"ui.dance"` → `"dance"`) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(120.0, 32.0)` | Width and height in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.15,0.15,0.15,1)` | Background colour as linear RGBA |
| `align` | `UiTextAlign` | `Center` | Text alignment: `Left`, `Center`, `Right` |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

#### `Label((...))`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `text` | `String` | `""` | Static display text (overridden at runtime when `bind` is set) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(120.0, 32.0)` | Width and height in pixels |
| `align` | `UiTextAlign` | `Center` | Text alignment: `Left`, `Center`, `Right` |
| `bind` | `Option<String>` | `None` | `GameVariables` key — when set, label text is replaced each frame with the variable's value |
| `format` | `Option<String>` | `None` | Template for `bind`; `"{}"` is replaced by the value (e.g. `"Score: {}"`). Raw value used when omitted. |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

#### `Rect((...))`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(120.0, 32.0)` | Width and height in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.15,0.15,0.15,1)` | Fill colour as linear RGBA |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

Click coordinates for browser tests: **center = `(position.x + size.w/2, position.y + size.h/2)`**.

#### `StatBar((...))` ✅

A bar that fills proportionally to `current / max` of a named stat from `LoadedStats`. Updates automatically every frame — no event wiring or `GameVariables` binding needed.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `stat_key` | `String` | required | Key of the stat to display (must match a key in `stats.ron`) |
| `orientation` | `BarOrientation` | `Horizontal` | `Horizontal` (left→right) or `Vertical` (bottom→top) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(200.0, 20.0)` | Width and height in pixels |
| `fill_color` | `(f32,f32,f32,f32)` | red | Colour of the filled portion as linear RGBA |
| `background_color` | `(f32,f32,f32,f32)` | dark red | Colour of the unfilled portion |
| `show_value` | `bool` | `false` | Overlay `"current / max"` text centred on the bar |
| `color_bands` | `Vec<ColorBand>` | `[]` | Threshold-based colour overrides. Each band: `( above_percent: f32, color: (r,g,b,a) )`. The highest `above_percent` ≤ current fill ratio is selected. |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

```ron
StatBar((
  id: "health_bar",
  stat_key: "player_health",
  position: (16.0, 60.0),
  size: (200.0, 18.0),
  fill_color:       (0.85, 0.15, 0.15, 1.0),
  background_color: (0.20, 0.06, 0.06, 1.0),
  show_value: true,
  color_bands: [
    ( above_percent: 0.5,  color: (0.85, 0.15, 0.15, 1.0) ),  // red   (normal)
    ( above_percent: 0.25, color: (1.0,  0.55, 0.0,  1.0) ),  // orange (low)
    ( above_percent: 0.0,  color: (0.6,  0.0,  0.0,  1.0) ),  // dark red (critical)
  ],
  absolute: true,
)),
```

**Stat not found:** If `stat_key` is not present in `LoadedStats`, the bar renders as empty (0 % fill). A warning is logged in debug builds. No panic occurs.

#### `StatSpread((...))` ✅

A panel that lists multiple stats as labelled minibar rows. Each row shows the stat name, a minibar fill, and optionally the numeric value.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `stats` | `Vec<String>` | required | Ordered list of stat keys to display |
| `layout` | `StatSpreadLayout` | `Rows` | `Rows` (one row per stat) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `label_width` | `f32` | `80.0` | Width of the stat-name label column in pixels |
| `bar_width` | `f32` | `120.0` | Width of the minibar column in pixels |
| `row_height` | `f32` | `22.0` | Height of each row in pixels |
| `row_gap` | `f32` | `4.0` | Vertical gap between rows in pixels |
| `label_color` | `(f32,f32,f32,f32)` | near-white | Colour of the stat-name and value text |
| `bar_fill_color` | `(f32,f32,f32,f32)` | blue | Minibar fill colour |
| `bar_background_color` | `(f32,f32,f32,f32)` | dark blue | Minibar background colour |
| `show_values` | `bool` | `true` | Show `"current / max"` text after each minibar |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

```ron
StatSpread((
  id: "stat_panel",
  stats: ["player_health", "player_mana", "player_stamina"],
  position: (16.0, 88.0),
  label_width: 110.0,
  bar_width: 160.0,
  row_height: 24.0,
  row_gap: 5.0,
  bar_fill_color:       (0.35, 0.75, 0.35, 1.0),
  bar_background_color: (0.08, 0.18, 0.08, 1.0),
  show_values: true,
  absolute: true,
)),
```

### UI Panel (`UiPanelDef`) ✅

When a scene includes a `ui_panel` block, all `ui` elements are arranged in a vertically-flowing centered panel instead of using absolute positioning. Elements with `absolute: true` are still positioned relative to the panel's top-left corner.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `background_color` | `(f32,f32,f32,f32)` | `(0.1,0.1,0.1,0.95)` | Background colour as linear RGBA (0.0–1.0) |
| `padding` | `f32` | `20.0` | Inner padding around panel contents in pixels |
| `gap` | `f32` | `12.0` | Vertical gap between child elements in pixels |
| `width` | `Option<f32>` | `None` | Fixed panel width in pixels; auto-sized when omitted |
| `height` | `Option<f32>` | `None` | Fixed panel height in pixels; auto-sized when omitted (required for panels with absolutely-positioned children such as maps) |

```ron
ui_panel: (
  background_color: (0.08, 0.08, 0.08, 0.95),
  padding: 24.0,
  gap: 14.0,
  width: 380.0,
),
```

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
    "click": (path: "shared/audio/menu-button-click.wav"),
    "bg_music": (path: "shared/audio/theme.ogg", volume: 0.6),
  },
  effects: {
    "hit_spark": (
      particle_count: 12,
      lifetime_secs: 0.45,
      speed: 3.5,
      speed_jitter: 0.8,
      spread_deg: 180.0,
      offset: (0.0, 1.0, 0.0),
      size: 0.055,
      size_end: Some(0.0),
      color_start: (1.0, 0.8, 0.2, 1.0),
      color_end: (1.0, 0.1, 0.0, 0.0),
      gravity: -5.0,
    ),
  },
  materials: {
    "wood_crate": (
      kind: Standard((
        base_color_texture: "shared/textures/wood_crate_albedo.png",
        metallic: 0.0,
        perceptual_roughness: 0.85,
      )),
      alpha_mode: Opaque,
      double_sided: false,
    ),
  },
)
```

**EffectDef fields** (used inside `effects: { "key": ( … ) }`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `particle_count` | `u32` | `12` | Number of particles per burst. Must be ≤ 256 (validated at catalog load). Unused when `layers` is non-empty. |
| `lifetime_secs` | `f32` | `1.0` | How long each particle lives. Provide a meaningful value for single-layer effects; unused when `layers` is non-empty. |
| `speed` | `f32` | `0.0` | Initial outward speed in metres/second along each particle's direction. |
| `speed_jitter` | `f32` | `0.0` | Per-particle speed variation in `[−jitter, +jitter]` — deterministic, index-based. |
| `spread_deg` | `f32` | `180.0` | Cone half-angle in degrees (0 = straight up column, 90 = hemisphere, 180 = full sphere). |
| `offset` | `(f32, f32, f32)` | `(0.0, 1.0, 0.0)` | World-space offset from the entity origin or explicit position (e.g. chest height). |
| `size` | `f32` | `0.06` | Particle radius in metres at birth. |
| `size_end` | `Option<f32>` | `None` | If set, particle radius lerps from `size` to `size_end` over the lifetime. |
| `size_jitter` | `f32` | `0.0` | Per-particle size variation in `[−jitter, +jitter]` at birth. |
| `color_start` | `(f32, f32, f32, f32)` | `(1,1,1,1)` | RGBA colour at birth (linear, 0.0–1.0). Provide a value for single-layer effects; unused when `layers` is non-empty. |
| `color_mid` | `Option<(f32,f32,f32,f32)>` | `None` | Optional midpoint colour for a three-stop gradient (start → mid → end). |
| `color_end` | `(f32, f32, f32, f32)` | `(1,1,1,0)` | RGBA colour at death. Provide a value for single-layer effects; unused when `layers` is non-empty. |
| `gravity` | `f32` | `0.0` | Vertical acceleration in m/s² (negative = falls, positive = rises). |
| `turbulence` | `f32` | `0.0` | Per-frame lateral noise strength; creates billowing and swirling motion. |
| `sprite` | `Option<String>` | `None` | Asset key for a sprite texture (from `AssetCatalog.textures`). When set, particles are camera-facing quads instead of sphere meshes. |
| `sprites` | `Vec<String>` | `[]` | Multiple sprite keys; each particle picks one by deterministic hash. Takes precedence over `sprite`. |
| `additive` | `bool` | `false` | `true` → `AlphaMode::Add` (fire, glow); `false` → `AlphaMode::Blend` (smoke). Has no effect without a sprite. |
| `uv_distort` | `f32` | `0.0` | UV distortion for the flame shader. Non-zero switches the particle to `PoolFlameMaterial` (animated WGSL). Range 0.0–1.0; typical: 0.4–0.6. |
| `uv_scroll_speed` | `f32` | `0.0` | Upward UV scroll in texture-heights per second. Combine with `uv_distort` for flowing flame. |
| `layers` | `Vec<LayerDef>` | `[]` | Multi-layer emitter list — see section below. When non-empty, all flat fields above are unused. |

**Multi-layer effects (`layers`)**

When `layers` is non-empty, each entry is spawned independently at the same origin. All flat fields on the parent `EffectDef` are ignored. Use this to compose complex effects — fire body + hot core, smoke + rising sparks — in a single catalog key and a single `SpawnEffect` call.

Each `LayerDef` accepts every field in the table above except `layers` itself. Fields work identically inside a layer. Example:

```ron
"campfire_fire": (
    layers: [
        // body — 4 large orange flame quads
        ( particle_count: 4, lifetime_secs: 1.00, spread_deg: 0.0, emit_radius: 0.16,
          offset: (0.0, 0.22, 0.0), size: 0.65, size_jitter: 0.08,
          color_start: (1.0, 0.52, 0.08, 0.0), color_mid: (1.0, 0.42, 0.05, 1.0),
          color_end: (0.55, 0.06, 0.0, 0.0),
          sprites: ["particle/flame_01", "particle/flame_02"], additive: true,
          uv_distort: 0.50, uv_scroll_speed: 0.55 ),
        // core — 2 bright white-hot quads
        ( particle_count: 2, lifetime_secs: 0.80, spread_deg: 0.0, emit_radius: 0.06,
          offset: (0.0, 0.26, 0.0), size: 0.28,
          color_start: (1.0, 1.0, 0.88, 0.0), color_mid: (1.0, 0.80, 0.18, 1.0),
          color_end: (1.0, 0.28, 0.0, 0.0),
          sprites: ["particle/flame_05", "particle/flame_06"], additive: true,
          uv_distort: 0.35, uv_scroll_speed: 1.00 ),
    ],
),
```

Full canonical example: `assets/projects/particles_demo/assets.ron` → `"campfire_fire"`.

Particles use `AlphaMode::Add` (additive blending) by default when no sprite is set — overlapping spheres glow brighter, no depth-sorting artefacts in WASM. Directions are sampled deterministically via a spherical-cap golden-angle spiral so the same effect always produces the same pattern.

**Particle material paths** — the engine selects one of three implementations at spawn time:

| Condition | Material | Notes |
|-----------|----------|-------|
| `sprite` is `None` | `StandardMaterial` + `AlphaMode::Add` | Sphere mesh, colour gradient only |
| `sprite` set, `uv_distort == 0` and `uv_scroll_speed == 0` | `StandardMaterial` + configurable alpha | Quad billboard, static sprite |
| `sprite` set, `uv_distort > 0` or `uv_scroll_speed > 0` | `FlameParticleMaterial` | Quad billboard, animated WGSL shader (`custom_flame_particle.wgsl`) |

`FlameParticleMaterial` is an engine-internal material — it is not available as a `Custom(…)` shader key. Its uniforms (`color`, `elapsed_time`) are updated every frame by the particle system.

**Audio format recommendations:**

| Format | Use for | Notes |
|--------|---------|-------|
| `.wav` | Short SFX (jumps, clicks, pickups) | Uncompressed PCM — zero decode overhead, instant playback |
| `.ogg` | Music and long ambient loops | Compressed; smaller files, minor decode cost acceptable for long audio |
| `.mp3` | Music only (avoid for new work) | Worse quality/size ratio than OGG; use OGG instead |

Do not use `.aiff` or `.flac` — these formats are not supported and will produce a warning at load time with no audio playing.

Trim any leading silence from SFX files before exporting — silence baked into the file adds perceived latency on every play.

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

**`TerrainMaterialDef` fields** (used inside `kind: Terrain(…)`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `splatmap` | `String` | required | Path to RGBA splatmap (one channel per layer) |
| `layers` | `Vec<String>` | `[]` | Texture paths for terrain layers (R, G, B channels). Fewer than 3 logs a warning and missing slots render as magenta. A 4th path (A channel) is accepted but unused by the current shader. |
| `uv_scale` | `f32` | `10.0` | UV tiling scale for layer textures. Higher values tile textures more finely. |

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
| `components.inputs` | `Option<InputMap>` | Key bindings for the player character. Only read for `"player"` prefabs. Omit to use WASD defaults. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.flycam` | `Option<FlyCamDef>` | Speed and sensitivity tuning for the free-fly camera. Only read for `"flycam"` prefabs. Omit to use defaults. See [Special tag: `"flycam"`](#special-tag-flycam-) below. |
| `components.camera` | `Option<CameraConfig>` | Orbit camera settings (offset, zoom, orbit speed, radius limits). Only read for `"player"` prefabs. Omit to use engine defaults. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.npc` | `Option<NpcDef>` | NPC AI configuration. When set, the entity gets a dynamic physics body and an NPC behaviour driver. See [NPC behaviour](#npc-behaviour-componentsnpc-) below. |
| `components.sounds` | `HashMap<String, String>` | Informational map from event name to `AssetCatalog` audio key. Not auto-wired — reference these keys in `state_machine.ron` to bind sounds to events (e.g. `player.jumped → PlaySound(key: "sfx_jump")`). |
| `primitive` | `Option<PrimitiveParams>` | Shape dimensions and appearance; only used when `kind: "primitive"` |
| `children` | `Vec<ChildPrimitiveDef>` | Sub-meshes composing a composite primitive (e.g. lamp post + orb). Only used when `kind: "primitive"`. See below. |
| `colliders` | `Vec<ColliderDef>` | One or more static physics colliders for `kind: "actor"` / `kind: "prop"`. All shapes are combined into a single Rapier compound body — use multiple entries to approximate curved geometry or multi-part shapes. Empty list = no physics. See below. |
| `behavior` | `Option<String>` | Path to a `.behavior.ron` file relative to the project root. Loads an independent per-entity FSM; `{self}` in event patterns and action keys is replaced with the entity's spawn ID. Works for all `kind` values, including composite `"primitive"` prefabs with `children`. See `docs/30_runtime_events_and_logic.md`. |
| `trigger_zone` | `Option<TriggerZoneDef>` | Spawns a Rapier sphere sensor. Emits `entity.entered:{id}` / `entity.exited:{id}` when the player overlaps. Field: `radius: f32`. Works on all prefab kinds, including composite primitives (`model: ""` + non-empty `children`). |
| `interactable` | `Option<InteractableDef>` | Emits `entity.interacted:{id}` when the player is within `radius` metres and presses the interact key (default `"KeyF"`). Field: `radius: f32`. |
| `stat_templates` | `Vec<StatTemplateDef>` | Per-entity stat shapes. Every spawned instance gets an independent `StatMap` component; stats are addressed as `"spawn_id.stat_name"` in `ModifyStat`/`SetStat`. See [Instance stats](#instance-stats-stat_templates-) below. |
| `stat_label` | `Option<StatLabelDef>` | Floating world-space numeric stat label above the entity. Tracks a live stat and updates every frame. See [World-space stat widgets](#world-space-stat-widgets-stat_label-and-world_stat_bar-) below. |
| `world_stat_bar` | `Option<WorldStatBarDef>` | Floating world-space stat bar above the entity. Style is configurable: `Ascii` (two overlapping `Text2d` entities) or `Pixel` (a `Mesh2d` quad hierarchy rendered by the 2D camera). Both update every frame. See [World-space stat widgets](#world-space-stat-widgets-stat_label-and-world_stat_bar-) below. |

### Special tag: `"flycam"` ✅

A prefab with `components.tags: ["flycam"]` and any `kind` spawns a free-flying camera instead of a model. The `model` field is ignored. The engine creates a `Camera3d` + `FlyCamera` component at the entity's transform.

**Controls:**
- **W/S** — forward / back
- **A/D** — strafe left / right
- **E / Space** — ascend
- **Q / LCtrl** — descend
- **LShift / RShift** — fast mode
- **Hold LMB or RMB + move mouse** — rotate view (mouse is free for UI when no button is held)

**`FlyCamDef` fields** (`components.flycam`, all optional — defaults apply when omitted):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `speed` | `f32` | `100.0` | Normal movement speed in units/second |
| `fast_speed` | `f32` | `200.0` | Movement speed while Shift is held, in units/second |
| `sensitivity` | `f32` | `0.002` | Mouse look sensitivity in radians per pixel |
| `forward` | `String` | `"KeyW"` | Key for moving forward |
| `backward` | `String` | `"KeyS"` | Key for moving backward |
| `left` | `String` | `"KeyA"` | Key for strafing left |
| `right` | `String` | `"KeyD"` | Key for strafing right |
| `up` | `String` | `"Space"` | Key for ascending |
| `down` | `String` | `"KeyQ"` | Key for descending |
| `look_button` | `String` | `"Either"` | Mouse button that activates look mode: `"Left"`, `"Right"`, or `"Either"` |

To display the camera's world position in the UI, add a label element with `id: "flycam_position"` to the scene's `ui` array. The engine will update it every frame.

```ron
// In prefabs/prefabs.ron — minimal (all defaults)
"flycam": (
  kind: "prop",
  model: "",
  components: ( tags: ["flycam"] ),
),

// In prefabs/prefabs.ron — with custom speed tuning
"flycam_slow": (
  kind: "prop",
  model: "",
  components: (
    tags: ["flycam"],
    flycam: (
      speed:       20.0,
      fast_speed:  80.0,
      sensitivity: 0.001,
    ),
  ),
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

A prefab with `components.tags: ["player"]` spawns a third-person character controller with an orbit camera. Works on both `kind: "actor"` (GLB model) and `kind: "primitive"` (capsule shape). Movement is tuned via `components.movement`; key bindings are tuned via `components.inputs`.

**`InputMap` fields** (`components.inputs` — omit the entire block to use WASD defaults):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `forward` | `String` | `"KeyW"` | Move forward |
| `backward` | `String` | `"KeyS"` | Move backward |
| `left` | `String` | `"KeyA"` | Rotate/strafe left |
| `right` | `String` | `"KeyD"` | Rotate/strafe right |
| `strafe_left` | `String` | `"KeyQ"` | Strafe left (camera-relative) |
| `strafe_right` | `String` | `"KeyE"` | Strafe right (camera-relative) |
| `jump` | `String` | `"Space"` | Jump |
| `run` | `String` | `"ShiftLeft"` | Hold to run |
| `interact` | `String` | `"KeyF"` | Interact with nearby `interactable` entities |
| `strafe_mouse_button` | `Option<String>` | `Some("Left")` | Mouse button that enables strafe-mode (A/D strafe instead of rotate): `"Left"`, `"Right"`, or `None` to disable entirely |

**Valid key name strings** — both the canonical form (`"KeyW"`) and the shorthand (`"W"`) are accepted for letters and digits:

| Category | Valid strings |
|----------|--------------|
| Letters | `"KeyA"`–`"KeyZ"` (or bare `"A"`–`"Z"`) |
| Digits | `"Digit0"`–`"Digit9"` (or bare `"0"`–`"9"`) |
| Function | `"F1"`–`"F12"` |
| Modifiers | `"ShiftLeft"`, `"ShiftRight"`, `"ControlLeft"`, `"ControlRight"`, `"AltLeft"`, `"AltRight"` |
| Common | `"Space"`, `"Escape"`, `"Enter"`, `"Tab"`, `"Backspace"`, `"Delete"` |
| Arrows | `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"` |

Invalid key strings produce a `warn!` at load time and that binding has no effect. Case is significant — `"space"` and `"shiftleft"` are not valid.

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
| `idle_drag` | `f32` | `0.8` | Velocity decay multiplier on the XZ plane per physics tick when no input is given (0 = instant stop, 1 = no friction) |
| `linear_damping` | `f32` | `0.5` | Rapier `linear_damping` on the player capsule rigid body |
| `angular_damping` | `f32` | `0.5` | Rapier `angular_damping` on the player capsule rigid body |
| `ground_cast_length` | `f32` | `0.3` | Distance (metres) the ground-detection sphere is swept downward each frame — increase for uneven terrain or fast vertical movement |

**`JumpConfig` variants:**
- `Fixed(height: <f32>)` — absolute world-space height in metres (e.g. `Fixed(height: 2.5)`)
- `RelativeToHeight(percent: <f32>)` — fraction of the player's own height (e.g. `RelativeToHeight(percent: 100)`)

**`CameraConfig` fields** (`components.camera` — omit the entire block to use engine defaults):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `offset` | `(f32, f32, f32)` | `(0, 5, 10)` | Camera position relative to the player (right, up, back) |
| `look_at_offset` | `(f32, f32, f32)` | `(0, 2, 0)` | Point the camera looks at, relative to the player origin (use `(0, 1.5, 0)` to look at chest height) |
| `zoom_speed` | `f32` | `10.0` | Scroll-wheel zoom speed |
| `orbit_speed` | `f32` | `0.5` | Mouse orbit speed (radians per pixel) |
| `min_radius` | `f32` | `2.0` | Minimum zoom distance in metres |
| `max_radius` | `f32` | `20.0` | Maximum zoom distance in metres |
| `min_pitch` | `f32` | `0.1` | Minimum pitch in radians (looking up limit) |
| `max_pitch` | `f32` | `1.5` | Maximum pitch in radians (looking down limit) |
| `orbit_button` | `String` | `"Either"` | Mouse button that orbits the camera: `"Left"`, `"Right"`, or `"Either"` |
| `character_rotate_button` | `Option<String>` | `Some("Right")` | Mouse button that also rotates the character yaw while orbiting; set to `None` to disable |
| `initial_pitch` | `f32` | `0.5` | Camera pitch at scene start in radians |
| `initial_yaw` | `f32` | `0.0` | Camera yaw at scene start in radians |

**Jump sound** — the player system emits `GameEvent::Trigger("player.jumped")` on every jump. Wire a sound to it in `logic/state_machine.ron`:
```ron
on: [
  (event: "player.jumped", do_actions: [PlaySound(key: "sfx_jump")]),
]
```

### NPC behaviour (`components.npc`) ✅

Set `components.npc` on any prefab to attach NPC AI. The engine spawns a dynamic Rapier capsule body and runs the behaviour system each physics tick. Events emitted:

- `npc.player_spotted:{id}` — player entered detection range and the alerted pause has elapsed
- `npc.player_reached:{id}` — NPC is within `approach_distance` of the player
- `npc.player_lost:{id}` — player left `chase_radius`; NPC enters the Return state

**`NpcDef` fields** (`components.npc`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `faction` | `NpcFaction` | — | `Friendly`, `Hostile`, or `Neutral` |
| `on_player_near` | `NpcOnPlayerNear` | — | `Chase`, `Interact`, `Flee`, or `Alert` |
| `detection_radius` | `f32` | — | Metres inside which the NPC enters the Alerted state |
| `chase_radius` | `f32` | — | Metres beyond which the NPC gives up and returns to patrol |
| `fov_degrees` | `Option<f32>` | `None` (360°) | Half-angle forward cone in degrees; `None` = no blind spot |
| `requires_los` | `bool` | `false` | Rapier ray cast must confirm unobstructed line of sight before detecting the player |
| `approach_distance` | `f32` | `2.0` | Stop approaching at this distance (interact / attack range) |
| `patrol_speed` | `f32` | `2.0` | m/s while walking the patrol route or returning to origin |
| `chase_speed` | `f32` | `4.5` | m/s while chasing or fleeing |
| `patrol_waypoints` | `Vec<(f32,f32,f32)>` | `[]` | Offsets relative to spawn position; empty = idle in place |
| `eye_height` | `f32` | `0.9` | Metres above the entity origin used for LOS ray origin (tune for short/tall NPCs) |
| `alerted_duration` | `f32` | `0.3` | Seconds the NPC pauses in the Alerted state before acting |
| `drag` | `f32` | `0.8` | Velocity decay multiplier per physics tick when not actively moving (0 = instant stop, 1 = no decay) |
| `waypoint_reach_radius` | `f32` | `0.5` | Metres from a waypoint at which the NPC advances to the next one |
| `interact_leave_factor` | `f32` | `1.5` | Multiplier on `approach_distance` defining the leave-interact threshold (`distance > approach_distance * factor` exits Interact state) |
| `home_arrival_radius` | `f32` | `0.5` | Metres from spawn origin at which the NPC considers itself home and ends Return state |
| `linear_damping` | `f32` | `0.5` | Rapier `linear_damping` on the NPC capsule rigid body |
| `angular_damping` | `f32` | `0.5` | Rapier `angular_damping` on the NPC capsule rigid body |

```ron
// Hostile patrol guard — full configuration
"orc_guard": (
  kind: "actor",
  model: "orc_glb",
  components: (
    npc: (
      faction: Hostile,
      on_player_near: Chase,
      detection_radius: 8.0,
      chase_radius: 20.0,
      fov_degrees: 110.0,
      requires_los: true,
      approach_distance: 1.5,
      patrol_speed: 2.5,
      chase_speed: 5.0,
      patrol_waypoints: [
        (5.0, 0.0, 0.0),
        (5.0, 0.0, 10.0),
      ],
      // alerted_duration / drag / waypoint_reach_radius omitted — use defaults
    ),
  ),
),

// Small creature — custom physics feel and tight waypoint radius
"rat": (
  kind: "primitive",
  model: "Capsule3d",
  primitive: ( radius: 0.15, height: 0.3 ),
  components: (
    npc: (
      faction: Neutral,
      on_player_near: Flee,
      detection_radius: 3.0,
      chase_radius: 8.0,
      eye_height: 0.15,
      drag: 0.5,
      waypoint_reach_radius: 0.2,
    ),
  ),
),
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
      primitive: (
        size: (2.0, 2.0, 2.0),
        // color omitted — uses project primitive_default_color
        roughness: 0.4,
      ),
    ),
    "beacon_sphere": (
      kind: "primitive",
      model: "Sphere",
      components: (),
      primitive: (
        radius: 1.5,
        color: (0.9, 0.2, 0.2),  // red override
        roughness: 0.2,
        metallic: 0.3,
      ),
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
| `rotation_euler_deg` | `(f32,f32,f32)` | `(0,0,0)` | Euler rotation in degrees (XYZ order) for this shape's local orientation |

```ron
// Simple single-shape prop
"barrel": (
  kind: "prop",
  model: "barrel",
  components: (),
  colliders: [
    (shape: "Cylinder", radius: 0.35, height: 0.9),
  ],
),

// Multi-shape prop: chest with separate base and lid colliders
"chest_01": (
  kind: "prop",
  model: "chest_01",
  components: (tags: ["loot"]),
  colliders: [
    (shape: "Cuboid", size: (0.70, 0.55, 1.00), offset: (0.0, -0.125, 0.0)),
    (shape: "Cuboid", size: (0.68, 0.28, 0.98), offset: (0.0,  0.275, 0.0)),
  ],
),

// Archway approximated with three boxes — diagonal brace uses rotation_euler_deg
"archway": (
  kind: "prop",
  model: "archway",
  components: (),
  colliders: [
    (shape: "Cuboid", size: (0.4, 3.0, 0.4), offset: (-1.5, 1.5, 0.0)),
    (shape: "Cuboid", size: (0.4, 3.0, 0.4), offset: ( 1.5, 1.5, 0.0)),
    (shape: "Cuboid", size: (3.4, 0.4, 0.4), offset: ( 0.0, 3.2, 0.0)),
    (shape: "Cuboid", size: (0.3, 2.0, 0.3), offset: ( 0.0, 1.5, 0.0), rotation_euler_deg: (0.0, 0.0, 45.0)),
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
      material: "mat_stone_cobble",
      primitive: (size: (18.0, 0.02, 14.0)),
      offset: (0.0, 0.01, 0.0),
    ),
    // Nested composite prefab (kind: "primitive" with children)
    (
      prefab: "well",
      offset: (5.0, 0.0, 0.0),
      rotation_euler_deg: (0.0, 45.0, 0.0),
    ),
    // Nested GLB prop (kind: "prop" — loads a .glb file)
    (
      prefab: "rock_deco",
      offset: (3.0, 0.0, -2.0),
      rotation_euler_deg: (0.0, 35.0, 0.0),
    ),
    // Nested single-shape primitive (kind: "primitive" with no children, just a model)
    (
      prefab: "beacon",
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
    default_transition_ms: 150,

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
            stop_action: "stop_dance",
        ),
        (
            id: "attack_light",
            clip: "Sword_Attack",
            priority: 100,
            looping: false,
            duration: 0.6,
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
| `Spawn { prefab, id, position, spawn_point, yaw_deg }` | Enqueue a prefab spawn (max 2/frame); `id` auto-generated if omitted; `spawn_point` looks up a scene-defined named point; `yaw_deg` rotates around Y axis |
| `PreloadPrefab("key")` | Load a prefab's GLB early and cache the handle; fire on `scene.ready` to eliminate the first-spawn WASM decode stall |
| `Despawn("id")` | Remove a previously spawned entity by its spawn ID |
| `PlayAnimation("id")` | Play an animation by semantic ID (see AnimationPolicy) |
| `PlaySound(key: "key")` | Play a sound by audio catalog key (`.wav`, `.ogg`, `.mp3`); warns on missing key or unsupported format; optional `volume: f32` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume |
| `PlayMusicLoop(key: "key")` | Start a looping background track by audio catalog key; stops any currently playing music; optional `volume: f32` multiplies the per-entry catalog volume |
| `Log("message")` | Emit an `info!` log line |
| `Quit` | Exit the application |
| `PreloadScene("path")` | Warm the asset cache for a `.scene.ron` before it is needed |
| `EnterState("name")` | Transition the interpreter to a named logic state; `""` returns to stateless |
| `SetVariable("key", "value")` | Write a named string variable into `GameVariables`; readable by data-bound UI labels |
| `IncrementVariable("key", i32)` | Parse the variable as `i32` and add the delta; missing or unparseable values default to `0` |
| `ModifyStat(key: "key", delta: f32)` | Add `delta` to a stat and clamp. **Dot-routing:** `"spawn_id.stat_name"` targets that entity's `StatMap`; no dot targets global `LoadedStats`. In behavior files, `{self}` in `key` is substituted with the entity's spawn ID. |
| `SetStat(key: "key", value: f32)` | Set a stat to an absolute value and clamp. Same dot-routing and `{self}` substitution as `ModifyStat`. |
| `ShowDamagePopup(entity: "id", amount: f32)` | Spawns a floating `+N` / `-N` label above the entity with the given spawn ID. Positive amounts show in heal colour, negative in damage colour. Uses `{self}` substitution in behavior files. Style (font size, duration, colours) is set via `damage_popup_style` in `.project.ron`. |
| `SetEntityVisible(entity: "id", visible: bool)` | Shows (`true`) or hides (`false`) a spawned entity by its spawn ID. The entity stays in the ECS — colliders and behavior FSM keep running. World-space labels tracking that entity (stat bar, stat label) auto-hide automatically. Uses `{self}` in behavior files. |
| `EmitEventAfterDelay(event: "name", delay_secs: f32)` | Fires a `GameEvent::Trigger("name")` after `delay_secs` seconds. One-shot — fires once then is removed. Cleared on `Action::LoadScene` so delayed events do not leak across scene transitions. Uses `{self}` substitution in behavior files. |

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
| `from` | `Option<String>` | Source state; omit to match any current state |
| `on` | `String` | Event that triggers this transition |
| `to` | `String` | Target state |

Execution order on transition: `exit_actions` of old state → state change → `entry_actions` of new state.

---

## `stats.ron` — StatCatalog ✅

Named stat definitions for a project. Referenced via `stats_path` in `{name}.project.ron`. Optional — omitting it means no stat system for that project.

Stats persist across scene transitions (the `LoadedStats` resource is not cleared on scene load).

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | `u32` | ✅ | Must be `1` |
| `stats` | `Map<String, StatDef>` | ✅ | Named stats keyed by stat ID |

**`StatDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `base` | `f32` | ✅ | Starting value (must be within `[min, max]`) |
| `min` | `f32` | `0.0` | Minimum allowed value |
| `max` | `f32` | ✅ | Maximum allowed value |
| `regen_rate` | `f32` | `0.0` | Units per second added when regen is active. `0` = no regen |
| `regen_delay` | `f32` | `0.0` | Seconds after a decrease before regen resumes |
| `thresholds` | `Vec<StatThreshold>` | `[]` | Events to emit when a threshold is crossed |

**`StatThreshold` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `when` | `ThresholdCondition` | Condition that triggers the event |
| `emit` | `String` | Event name emitted as `GameEvent::Trigger` on false→true crossing |

**`ThresholdCondition` variants:**

| Variant | Example | Fires when… |
|---------|---------|-------------|
| `BelowOrEqual(f32)` | `BelowOrEqual(0.0)` | `current <= value` |
| `AboveOrEqual(f32)` | `AboveOrEqual(80.0)` | `current >= value` |
| `BelowPercent(f32)` | `BelowPercent(0.25)` | `current / max < fraction` |
| `AtOrAbovePercent(f32)` | `AtOrAbovePercent(1.0)` | `current / max >= fraction` |

Threshold events are **edge-triggered**: they fire once when the condition transitions from false to true. They do not re-fire every frame while the condition remains true.

**Example:**
```ron
// stats/stats.ron
(
    schema_version: 1,
    stats: {
        "health": (
            base: 100.0,
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: [
                ( when: BelowOrEqual(0.0),    emit: "stat.health.depleted" ),
                ( when: BelowPercent(0.25),   emit: "stat.health.low" ),
                ( when: AtOrAbovePercent(1.0),emit: "stat.health.full" ),
            ],
        ),
        "mana": (
            base: 50.0,
            min: 0.0,
            max: 50.0,
            regen_rate: 2.0,
            regen_delay: 3.0,
            thresholds: [
                ( when: AtOrAbovePercent(1.0), emit: "stat.mana.full" ),
            ],
        ),
    },
)
```

**Project config reference:**
```ron
// {name}.project.ron
(
    schema_version: 2,
    ...
    stats_path: "stats/stats.ron",
)
```

**Reacting to threshold events in rules:**
```ron
// logic/rules.ron
( on: "stat.health.depleted", do_actions: [ LoadScene("scenes/game_over.scene.ron") ] ),
( on: "stat.health.low",      do_actions: [ PlaySound(key: "heartbeat") ] ),
```

---

## Instance stats (`stat_templates`) ✅

While `stats.ron` holds **global** stats that persist across scene transitions (e.g. player health, score), `stat_templates` on a `PrefabDef` declare **per-entity** stats. Every spawned instance gets its own independent `StatMap` component — there is no shared state between instances of the same prefab.

### Authoring

```ron
// prefabs/prefabs.ron
"goblin_guard": (
  kind: "primitive",
  model: "Capsule3d",
  behavior: "behaviors/goblin_guard.behavior.ron",
  stat_templates: [
    (
      key: "health",
      base: 60.0,
      min:  0.0,
      max:  60.0,
      thresholds: [
        ( when: BelowOrEqual(0.0), emit: "stat.{self}.health.depleted" ),
      ],
    ),
  ],
),
```

`{self}` in `emit` strings is replaced with the entity's spawn ID at spawn time, so two instances `goblin_01` and `goblin_02` get independent events `"stat.goblin_01.health.depleted"` and `"stat.goblin_02.health.depleted"`.

### StatTemplateDef fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `key` | `String` | — | Stat name within the entity (the key inside `StatMap`) |
| `base` | `f32` | — | Starting value |
| `min` | `f32` | `0.0` | Minimum allowed value |
| `max` | `f32` | — | Maximum allowed value |
| `regen_rate` | `f32` | `0.0` | Units per second added when regen is active |
| `regen_delay` | `f32` | `0.0` | Seconds after a decrease before regen resumes |
| `thresholds` | `Vec<StatThreshold>` | `[]` | Events to emit on threshold crossings (same schema as `stats.ron`; `{self}` is substituted) |

### Addressing instance stats in rules

Use **dot notation** to address instance stats: `"spawn_id.stat_name"`.

```ron
// In a behavior file — {self} is substituted with the entity's spawn ID
( event: "entity.interacted:{self}", do_actions: [
    ModifyStat(key: "{self}.health", delta: -35.0),
]),
( event: "stat.{self}.health.depleted", do_actions: [
    Despawn("{self}"),
    IncrementVariable("score", 50),
]),
```

```ron
// In state_machine.ron — address a specific instance by ID
ModifyStat(key: "goblin_01.health", delta: -10.0)
```

**Routing rules for `ModifyStat` / `SetStat`:**

| Key format | Routed to |
|---|---|
| `"player_health"` (no dot) | `LoadedStats` resource (global stat) |
| `"goblin_01.health"` (dot present) | `StatMap` component on the entity with `SpawnId("goblin_01")` |

---

## World-space stat widgets (`stat_label` and `world_stat_bar`) ✅

These fields on `PrefabDef` attach floating UI to a spawned entity. Both widgets track a live stat and update every frame via `resolve_stat` — the same routing that drives `StatBar` and `StatSpread`.

**Auto-hide:** When the tracked entity is hidden (`SetEntityVisible(visible: false)`), both widgets automatically hide. They restore automatically when the entity is shown again.

**Stat key routing:**
- `"{self}.health"` — entity-local stat (requires `stat_templates`; `{self}` is resolved to the spawn ID at scene load)
- `"player_health"` — global stat (from `stats.ron`)

### `StatLabelDef` fields (`stat_label`)

A numeric text label (e.g. `"85 / 100"`).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stat_key` | `String` | — | Stat key; supports `{self}` substitution |
| `offset` | `(f32,f32,f32)` | `(0, 2.5, 0)` | World-space offset from the entity's origin in metres |
| `font_size` | `f32` | `16.0` | Screen-space font size in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.2, 0.9, 0.2, 1.0)` | Linear RGBA text colour |
| `show_max` | `bool` | `true` | When `true`, shows `"current / max"`; when `false`, shows `"current"` |

```ron
stat_label: (
  stat_key: "{self}.health",
  offset: (0.0, 2.1, 0.0),
  font_size: 14.0,
  color: (0.8, 0.8, 0.8, 0.85),
  show_max: true,
),
```

### `WorldStatBarDef` fields (`world_stat_bar`)

A floating stat bar above an entity. The visual style is set by the `style` field — either `Ascii` (text characters) or `Pixel` (solid-colour mesh quads). Both styles update every frame and auto-hide when the tracked entity is hidden.

**Shared top-level fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stat_key` | `String` | — | Stat key; supports `{self}` substitution |
| `offset` | `(f32,f32,f32)` | `(0, 2.8, 0)` | World-space offset from the entity origin in metres |
| `fill_color` | `(f32,f32,f32,f32)` | bright green | Base fill colour; used when `color_bands` is empty or no band matches |
| `bg_color` | `(f32,f32,f32,f32)` | dark red-brown | Background track colour |
| `color_bands` | `Vec<(f32,(f32,f32,f32,f32))>` | `[]` | Threshold-based fill colour overrides. Each entry is `(above_ratio, (r,g,b,a))`. The **highest** `above_ratio` ≤ the current fill ratio wins. Ratios are 0.0–1.0. When empty, the `Ascii` style falls back to built-in adaptive green/yellow/red; the `Pixel` style uses `fill_color` directly. |
| `style` | `WorldStatBarStyle` | `Ascii()` | Visual style; see variants below |

**`WorldStatBarStyle::Ascii` fields** (use `style: Ascii(...)` or omit `style` entirely for the default):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cells` | `u8` | `10` | Total character width. Practical range: 1–32. |
| `font_size` | `f32` | `14.0` | Screen-space font size in pixels |

**`WorldStatBarStyle::Pixel` fields** (use `style: Pixel(...)`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `size` | `(f32,f32)` | `(64.0, 8.0)` | Bar width × height in screen pixels. Size is constant at all camera distances. |
| `border` | `f32` | `1.5` | Border thickness in pixels. Set to `0.0` to disable. |
| `border_color` | `(f32,f32,f32,f32)` | near-black `(0.05,0.05,0.05,1.0)` | Border quad colour |

> **Pixel bar depth scaling:** Pixel bars render at a fixed screen-pixel size regardless of camera distance. Depth-based scaling is not yet implemented for the Pixel style.

```ron
// Minimal — omit style to get the default Ascii bar.
world_stat_bar: (
  stat_key: "{self}.health",
),

// Ascii bar — full configuration.
world_stat_bar: (
  stat_key: "{self}.health",
  offset: (0.0, 2.4, 0.0),
  fill_color: (0.15, 0.85, 0.15, 0.95),
  bg_color: (0.25, 0.08, 0.08, 0.75),
  color_bands: [
    (0.6,  (0.15, 0.85, 0.15, 0.95)),  // ≥ 60 % → green
    (0.3,  (1.0,  0.75, 0.10, 1.0)),   // ≥ 30 % → yellow
    (0.0,  (0.85, 0.10, 0.10, 1.0)),   // ≥  0 % → red
  ],
  style: Ascii( cells: 12, font_size: 14.0 ),
),

// Pixel bar — size and border are in screen pixels.
world_stat_bar: (
  stat_key: "{self}.health",
  offset: (0.0, 2.5, 0.0),
  fill_color: (0.15, 0.85, 0.15, 1.0),
  bg_color:   (0.20, 0.05, 0.05, 0.85),
  color_bands: [
    (0.0, (0.85, 0.12, 0.12, 1.0)),
    (0.3, (0.95, 0.75, 0.10, 1.0)),
    (0.6, (0.15, 0.85, 0.15, 1.0)),
  ],
  style: Pixel( size: (48.0, 6.0) ),
),
```

> **Tip:** Use `stat_label` for a compact numeric readout, `world_stat_bar` for a graphical bar. Both styles can coexist on the same prefab (useful for demos), but in production most designs pick one style per entity.

**Migration from the pre-style schema:** The old flat fields `cells` and `font_size` at the top level of `WorldStatBarDef` are no longer supported. Move them inside `style: Ascii(...)`:

```ron
// Old (no longer valid)
world_stat_bar: ( stat_key: "…", cells: 10, font_size: 16.0 )

// New
world_stat_bar: ( stat_key: "…", style: Ascii( cells: 10, font_size: 16.0 ) )
```

Entries with no `style` field continue to parse correctly — the default is `Ascii` with `cells: 10` and `font_size: 14.0`.

---

## `DamagePopupStyle`

Optional block in `{name}.project.ron` that controls how `Action::ShowDamagePopup` popups look. All fields have built-in defaults — omit the block entirely to use them.

```ron
// {name}.project.ron
(
    schema_version: 3,
    ...
    damage_popup_style: (
        font_size:     22.0,             // default
        duration_secs:  1.2,             // default
        rise_speed:     1.5,             // default — metres/second the label rises
        spawn_offset:  (0.0, 1.2, 0.0), // default — world-space offset from entity origin
        damage_color:  (0.95, 0.25, 0.20, 1.0),  // default red (RGBA)
        heal_color:    (0.20, 0.90, 0.20, 1.0),  // default green (RGBA)
    ),
)
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font_size` | `f32` | `22.0` | Screen-space font size in pixels |
| `duration_secs` | `f32` | `1.2` | Seconds the popup is visible before fully fading |
| `rise_speed` | `f32` | `1.5` | Metres per second the label rises during its lifetime |
| `spawn_offset` | `(f32,f32,f32)` | `(0.0, 1.2, 0.0)` | World-space offset from the entity origin where the popup appears. Increase Y for tall entities. |
| `damage_color` | `(f32,f32,f32,f32)` | `(0.95, 0.25, 0.20, 1.0)` | Linear RGBA colour for negative amounts (damage) |
| `heal_color` | `(f32,f32,f32,f32)` | `(0.20, 0.90, 0.20, 1.0)` | Linear RGBA colour for positive amounts (healing) |

---

## Global environment lighting ✅

Can be set on `ProjectConfig.global_environment` and overrides per-scene lighting if a scene has no environment block.

```ron
global_environment: (
    intensity: 400.0,
    fallback: (
        top_color: (0.1, 0.2, 0.4),
        bottom_color: (0.01, 0.01, 0.01),
    ),
),
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
