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
| `audio` | `AudioConfig` | — | Project-level audio settings. Omit for defaults (`max_volume: 1.0, mute_on_start: false`). See [AudioConfig](#audioconfig) below. |
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
| `particle_budget` | `Option<u32>` | Maximum live particle count for this scene. Default: `2000`. `Ambient` effects are dropped when full; `Npc` effects are halved; `Player` effects always fire. |

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

### Scene entities (`SceneEntityDef`) ✅

Each entry in the `entities` list places one prefab instance into the scene.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique spawn ID for this instance (used in rules, behaviors, and stat addressing) |
| `prefab` | `String` | required | Key into `PrefabCatalog.prefabs` |
| `transform` | `SceneTransformV2` | `()` | Position, rotation, and scale. All sub-fields optional; defaults to origin, no rotation, scale `(1,1,1)`. |
| `label` | `Option<EntityLabelDef>` | `None` | Floating text annotation above the entity. See [Label depth scaling](#label-depth-scaling-labeldepthscaledef) for per-label `depth_scale` options. |
| `stat_overrides` | `Map<String, f32>` | `{}` | Override the initial value for named stats from the prefab's `stat_templates`. Keys are stat names; unknown keys log a warning. `min`/`max`/`regen`/`thresholds` are unchanged — only the starting value differs. |

**`stat_overrides` example** — place a wounded enemy and an inactive shrine from the same prefabs, without forking them:

```ron
entities: [
    // Full-health enemy
    ( id: "orc_01", prefab: "enemy_orc_melee", transform: ( translation: (10.0, 0.0, 5.0) ) ),

    // Already-wounded enemy — same prefab, starts at 30 HP instead of 100
    ( id: "orc_wounded", prefab: "enemy_orc_melee", transform: ( translation: (15.0, 0.0, 5.0) ),
      stat_overrides: { "health": 30 } ),

    // Inactive shrine — health starts at 0 so its "depleted" threshold fires immediately
    ( id: "shrine_inactive", prefab: "shrine", transform: ( translation: (0.0, 0.0, 10.0) ),
      stat_overrides: { "health": 0 } ),
],
```

> **Note:** `stat_overrides` only applies to scene-placed entities. Dynamically spawned entities via `Action::Spawn` always start at the template `base` value.

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

**GameVariables auto-written by capabilities** (bind a `Label` to these — no rule wiring needed):

| Key | Written by | Value |
|-----|-----------|-------|
| `target_display` | targeting | `"<prefab> <id>"` of the current target (e.g. `"enemy_orc_melee orc_01"`); empty string when no target |
| `target_name` | targeting | prefab catalog key of the current target (e.g. `"enemy_orc_melee"`) |
| `target_id` | targeting | spawn id of the current target (e.g. `"orc_01"`) |
| `score` | action executor | running score, derived from `IncrementVariable("score", …)` |

The targeting variables update on every selection change (click, Tab, or `SetTarget`) and blank on clear/`LoadScene`. Example: `Label((id: "target_label", bind: "target_display", format: "Target: {}"))` — see `assets/projects/3rd_person_game_demo`.

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

#### `ActionBar((...))` ✅

A row of up to 9 skill slots bound to keyboard keys 1–9. Pressing a key fires the slot's `do_actions` through the existing `Action` pipeline. Slots show a cooldown fill overlay while on cooldown and dim when the cost stat is insufficient. Always positioned absolutely.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier |
| `position` | `(f32, f32)` | `(0.0, 0.0)` | Top-left corner in pixels (always absolute) |
| `slot_size` | `f32` | `64.0` | Width and height of each slot square in pixels |
| `slot_gap` | `f32` | `4.0` | Pixel gap between slots |
| `background_color` | `(f32,f32,f32,f32)` | near-black 70 % | Bar container background as linear RGBA |
| `slots` | `Vec<ActionSlotDef>` | required | Ordered list of slot definitions |

**`ActionSlotDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `key` | `String` | required | Key that activates the slot: `"1"` through `"9"` |
| `icon` | `String` | `""` | Asset catalog texture key for the icon (reserved for future rendering) |
| `do_actions` | `Vec<Action>` | required | Actions fired through the pipeline on activation |
| `cooldown_secs` | `Option<f32>` | `None` | Seconds before the slot can activate again |
| `cost` | `Option<SlotCost>` | `None` | Stat cost checked and deducted at activation time |
| `label` | `Option<String>` | `None` | Tooltip label (future use) |

**`SlotCost` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `stat` | `String` | Key of the stat to check and deduct from (matches a key in `stats.ron`) |
| `amount` | `f32` | Amount to deduct. Slot blocks if `current < amount` |

**Pipeline events emitted by the action bar:**

| Event | When fired |
|-------|-----------|
| `action_bar.activated:{key}` | Slot fired successfully |
| `action_bar.on_cooldown:{key}` | Key pressed while slot is on cooldown |
| `action_bar.insufficient_resource:{key}` | Key pressed but cost stat too low |
| `action_bar.no_target:{key}` | `{target}` used in `do_actions` but no target is selected |

**`{target}` substitution:** Any occurrence of `{target}` in a slot's `do_actions` (and in all rule / FSM `do_actions`) is replaced with the spawn ID of the entity in `CurrentTarget`. For action bar slots, if `CurrentTarget` is `None` the slot emits `action_bar.no_target:{key}` and does not fire. `CurrentTarget` is populated by the targeting system — set `click_selectable: true` or `targetable: true` on a `PrefabDef` to enable.

```ron
ActionBar((
  id: "skill_bar",
  position: (16.0, 580.0),
  slot_size: 64.0,
  background_color: (0.05, 0.05, 0.08, 0.85),
  slots: [
    (
      key: "1",
      do_actions: [
        SpawnEffect(key: "heal_burst", entity: "player_01"),
        ModifyStat(key: "player_health", delta: 30.0),
      ],
      cooldown_secs: 5.0,
      cost: (stat: "player_mana", amount: 15.0),
      label: "Heal",
    ),
    (
      key: "2",
      do_actions: [ ApplyModifier(modifier_key: "speed_boost") ],
      cooldown_secs: 12.0,
      cost: (stat: "player_mana", amount: 20.0),
    ),
  ],
))
```

Wire feedback events in `rules.ron` or `state_machine.ron` to surface cooldown or low-mana messages:

```ron
( event: "action_bar.on_cooldown:1",           do_actions: [ SetVariable("status", "Skill on cooldown") ] ),
( event: "action_bar.insufficient_resource:1", do_actions: [ SetVariable("status", "Not enough mana") ] ),
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
| `emit_radius` | `f32` | `0.0` | Disc scatter radius. Deprecated in favour of `emitter: Disc(radius: …)` — kept for backward compatibility. |
| `rotation_start_deg` | `f32` | `0.0` | Billboard quad rotation at spawn, in degrees. |
| `rotation_end_deg` | `f32` | `0.0` | Rotation at end of lifetime (lerped from `rotation_start_deg`). Ignored when `rotation_speed_deg` is non-zero. |
| `rotation_speed_deg` | `f32` | `0.0` | Constant angular velocity in degrees/second. When non-zero, overrides `rotation_start_deg` / `rotation_end_deg`. |
| `size_x` | `Option<f32>` | `None` | Independent billboard width in metres. Overrides `size` for the X axis. Use `size_x < size_y` for tall narrow shapes. |
| `size_y` | `Option<f32>` | `None` | Independent billboard height in metres. Overrides `size` for the Y axis. |
| `size_x_end` | `Option<f32>` | `None` | End-of-life billboard width. Falls back to `size_end` when not set. |
| `size_y_end` | `Option<f32>` | `None` | End-of-life billboard height. Falls back to `size_end` when not set. |
| `emitter` | `EmitterShape` | `Point` | Spawn-position distribution — see "Emitter shapes" section below. |
| `velocity_curve` | `VelocityCurve` | `Linear` | Speed envelope over lifetime — see "Velocity curves" section below. |
| `layers` | `Vec<LayerDef>` | `[]` | Multi-layer emitter list — see section below. When non-empty, all flat fields above are unused. |
| `light` | `Option<EffectLightDef>` | `None` | Dynamic point light spawned at the effect origin — see section below. |
| `priority` | `EffectPriority` | `Npc` | Budget shedding bucket. `Ambient` = dropped when full; `Npc` = halved (min 1); `Player` = always fires. See "Quality & Budget" below. |
| `quality` | `Option<QualityOverride>` | `None` | Explicit per-tier counts. Bypasses the global multiplier when set. `high` is optional — omit to use `particle_count` at High. Example: `quality: (minimal: 1, low: 3, medium: 6)`. |
| `flipbook` | `Option<FlipbookDef>` | `None` | Sprite-sheet animation — see "Flipbook animation" section below. Cannot be combined with `uv_distort > 0`. |

**Emitter shapes (`emitter`)**

Controls where particles are spawned relative to the effect origin. All shapes are deterministic (no RNG).

| Variant | Example RON | Description |
|---------|-------------|-------------|
| `Point` | `emitter: Point` | All particles at origin (with optional `emit_radius` disc). Default. |
| `Disc` | `emitter: Disc(radius: 0.5)` | Uniform disc — particles scattered across a horizontal circle. |
| `Ring` | `emitter: Ring(radius: 1.5)` | All particles evenly spaced around the ring circumference. |
| `Sphere` | `emitter: Sphere(radius: 0.3)` | Uniform sphere surface via Fibonacci point distribution. |
| `Line` | `emitter: Line(length: 2.0, axis: Y)` | Particles spaced along a segment. `axis` is `X`, `Y` (default), or `Z`. |
| `Arc` | `emitter: Arc(radius: 1.0, angle_deg: 120.0)` | Partial ring subtending `angle_deg` degrees, centred on the origin. |

```ron
// orbiting rune particles — Ring emitter + rotation_speed_deg, flat single-layer effect
// (emitter: and rotation_* work at the top level, not only inside layers:[])
"magic_orbit": (
    particle_count: 14,
    lifetime_secs: 1.8,
    speed: 0.4,
    speed_jitter: 0.10,
    spread_deg: 12.0,
    emitter: Ring(radius: 0.8),
    offset: (0.0, 0.4, 0.0),
    size: 0.16,
    size_end: 0.0,
    rotation_speed_deg: 180.0,
    color_start: (0.80, 0.55, 1.0, 1.0),
    color_end:   (0.25, 0.08, 0.70, 0.0),
    gravity: 0.08,
    turbulence: 0.3,
    sprite: "particle/magic_03",
    additive: true,
),
```

**Velocity curves (`velocity_curve`)**

Scales the per-frame position delta over the particle's lifetime. The stored velocity vector is unmodified; only the step size changes.

| Variant | Scale at t=0 | Scale at t=1 | Use for |
|---------|-------------|-------------|---------|
| `Linear` | 1.0 | 1.0 | Constant speed (default). |
| `EaseOut` | 1.0 | 0.0 | Fast burst that decelerates to a stop — impact shards, explosions. |
| `EaseIn` | 0.0 | 1.0 | Slow start that accelerates — rising energy, charge-up. |
| `Pulse` | 1.0 | 1.0 | Fast → slow → fast (trough at mid-life) — orbit-like bob. |

```ron
// explosion shards: fast burst that coasts to a stop
"explosion_burst": (
    particle_count: 60, lifetime_secs: 1.1, speed: 7.0, spread_deg: 180.0,
    velocity_curve: EaseOut,
    color_start: (1.0, 0.98, 0.55, 1.0), color_end: (0.55, 0.04, 0.0, 0.0),
),

// tall narrow ice shards that tumble as they fly
"frost_shard": (
    particle_count: 20, lifetime_secs: 1.2, speed: 4.0, spread_deg: 75.0,
    size_x: 0.07, size_y: 0.30, size_y_end: 0.0,
    velocity_curve: EaseOut,
    rotation_speed_deg: 270.0,
    color_start: (0.85, 0.95, 1.0, 1.0), color_end: (0.25, 0.55, 1.0, 0.0),
    sprite: "particle/trace_01", additive: true,
),
```

**Field interactions**

These rules apply to both flat `EffectDef` fields and fields inside a `LayerDef` entry.

| Rule | Detail |
|------|--------|
| `rotation_speed_deg` overrides start/end | When non-zero, `rotation_start_deg` and `rotation_end_deg` are silently ignored. Only `rotation_speed_deg` takes effect. |
| `size_x` / `size_y` override `size` per axis | `size_x` replaces `size` for the X axis only; `size_y` replaces `size` for Y only. Unset axes still use the uniform `size`. |
| `size_x_end` / `size_y_end` fall back to `size_end` | If `size_x_end` is omitted it falls back to `size_end`; if that is also omitted the axis holds constant at its birth size. Same rule applies to `size_y_end`. |
| Non-`Point` emitter overrides `emit_radius` | When `emitter` is `Disc`, `Ring`, `Sphere`, `Line`, or `Arc`, `emit_radius` is ignored. Only `emitter: Point` (the default) honours `emit_radius` as a legacy disc-scatter fallback. |
| `layers:` makes all flat fields unused | When `layers` is non-empty, every field at the `EffectDef` level is ignored **except `light`**. Each `LayerDef` entry carries its own complete set of fields. |

**Dynamic effect lights (`light`)**

When `light` is set, a temporary `PointLight` is spawned at the effect origin the moment the effect fires. It fades in over `fade_in_secs`, holds at `intensity`, then fades out over `fade_out_secs` and despawns automatically. `LevelEntity` ensures cleanup on scene transitions.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `color` | `(f32, f32, f32)` | — | RGB light colour (linear, 0.0–1.0). |
| `intensity` | `f32` | — | Peak luminous power in lumens. 4000 ≈ torch, 8000 ≈ campfire, 30000 ≈ explosion. |
| `range` | `f32` | — | Radius of influence in metres. |
| `fade_in_secs` | `f32` | — | Seconds to reach full intensity. Use `0.0` for an instant flash. |
| `fade_out_secs` | `f32` | — | Seconds to fade out before despawn. |
| `duration_secs` | `Option<f32>` | `None` | Total lifetime. When omitted, defaults to the longest layer lifetime (or `lifetime_secs` for single-layer effects). |

```ron
"explosion_burst": (
    // ... particle fields ...
    light: (
        color: (1.0, 0.85, 0.40),
        intensity: 30000.0,
        range: 12.0,
        fade_in_secs: 0.0,
        fade_out_secs: 0.60,
    ),
),
```

The engine caps simultaneous fading lights at 16. When the cap is full, new light spawns are silently skipped — the particles still fire. This keeps within WebGPU mobile cluster limits (~32 total lights including authored scene fixtures).

**Flipbook animation (`flipbook`)**

Animates a sprite sheet by selecting UV sub-rectangles from a single texture over the particle's lifetime. Each particle advances through frames independently based on its own `elapsed` time.

```ron
// In EffectDef or LayerDef:
"sheet_explosion": (
    particle_count: 8,
    lifetime_secs: 1.4,
    sprite: "particle/explosion_4x4",   // 512×512 sprite sheet, 4 cols × 4 rows
    additive: true,
    flipbook: (
        cols: 4,    // number of columns in the sheet
        rows: 4,    // number of rows
        fps: 12.0,  // frame rate; at 12 fps a 16-frame sheet plays in 1.33 s
        loop: false,  // false: hold last frame until lifetime_secs expires (default)
                      // true: loop continuously for the full lifetime
    ),
),
```

**Sheet authoring conventions:**
- Row order: top-to-bottom, left-to-right (standard Aseprite / Photoshop export).
- Power-of-two PNGs recommended (256×256, 512×512, 1024×1024).
- White-on-transparent; tint via `color_start`/`color_end` gradient as usual.
- Place shared sheets in `assets/shared/textures/particles/sheets/`; register in the project's `assets.ron` textures section.

**Constraint:** `flipbook` and `uv_distort > 0` cannot be combined on the same layer. `uv_distort` animates UVs in the flame shader; flipbook bakes UV sub-rects into vertex data — they are mutually exclusive. Validation raises an error at catalog load time.

**No new pipeline variant:** flipbook particles use the existing `Additive` or `Blend` group key (same `StandardMaterial` pipeline). The sprite sheet is just a texture, so no additional WebGPU pipeline warmup is needed beyond the standard pattern for any new texture.

**Quality tiers & particle budget**

The engine supports a global quality level and a per-scene live-particle cap to keep frame times stable across hardware.

*Quality level* — set via `Action::SetParticleQuality`:

| Level | Multiplier | RON example |
|-------|-----------|-------------|
| `High` | 1.0× (default) | `SetParticleQuality(High)` |
| `Medium` | 0.75× | `SetParticleQuality(Medium)` |
| `Low` | 0.50× | `SetParticleQuality(Low)` |
| `Minimal` | 0.25× (min 1) | `SetParticleQuality(Minimal)` |

The multiplier is applied to each `particle_count` at spawn time. When `quality: (minimal: N, low: N, medium: N)` is set on an `EffectDef` or `LayerDef`, those explicit counts are used instead of the multiplier. The optional `high: N` field overrides the count at High quality too; when omitted, High uses `particle_count` directly.

```ron
// rules.ron — downgrade quality on scene load for mobile-class builds
( on: "scene.ready:main", do_actions: [ SetParticleQuality(Low) ] ),
```

`SetParticleQuality` persists across scene transitions (the `ParticleQuality` resource is never reset on `LoadScene`). Explicitly call `SetParticleQuality(High)` to restore full counts.

*Particle budget* — configured in the scene file:

```ron
// scene.ron — raise the cap for a particle-heavy boss arena
(
    schema_version: 2,
    particle_budget: 5000,
    entities: [ … ],
)
```

Default is 2000 when `particle_budget` is omitted. The cap is re-applied from the scene file on every scene load.

When the live count approaches `max_count`, effects are shed by `priority`:
- `Ambient` — silently skipped. Use for background fog, ambient embers, non-critical atmosphere.
- `Npc` (default) — particle count halved (minimum 1). The effect still fires with reduced density.
- `Player` — always fires at full count; may briefly exceed the budget.

> **Migration note**: effects defined without a `priority` field inherit `Npc` (the default). In a scene with a tight `particle_budget`, existing effects may now be halved when the budget is under pressure. If you observe effects being cut unexpectedly after adding `particle_budget` to a scene, add `priority: Player` to player-driven burst effects or `priority: Ambient` to background emitters.

```ron
// assets.ron
effects: {
    "campfire_smoke": (
        priority: Ambient,       // shed first when budget is full
        particle_count: 6, …
    ),
    "hit_spark": (
        priority: Player,        // always fires
        particle_count: 12, …
    ),
    // per-tier explicit counts override the global multiplier
    "ability_burst": (
        priority: Player,
        particle_count: 20,
        quality: ( minimal: 3, low: 8, medium: 14 ),
        …
    ),
}
```

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

---

**Ground decals (`decals`)**

The `decals` map in `assets.ron` registers texture paths for flat ground-projected quads. Decals are spawned by `Action::ProjectDecal` from `rules.ron` or behavior files. All decal textures are white-on-transparent PNGs — colour comes from the `color` field in the action.

```ron
// assets.ron
decals: {
  "aoe_fire_circle": "shared/textures/decals/ring_thick.png",
  "cast_indicator":  "shared/textures/decals/circle_filled.png",
},
```

**`Action::ProjectDecal` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `key` | `String` | required | Decal key from the `decals` map in `assets.ron`. |
| `entity` | `Option<String>` | `None` | If set, the decal XZ position tracks this entity each frame. Wins over `position`. Use `"{self}"` in behavior files. |
| `position` | `Option<(f32, f32, f32)>` | `None` | World-space origin. Y is ignored; decals always float at y=0.02. |
| `radius` | `f32` | required | Decal radius in metres. |
| `duration_secs` | `f32` | required | Lifetime in seconds. The decal fades out over the last 20 % and then despawns. |
| `color` | `(f32, f32, f32, f32)` | `(1,1,1,1)` | RGBA tint in linear 0–1 range. |
| `pulse_speed` | `f32` | `0.0` | Opacity heartbeat cycles per second (`0.0` = no pulse). |

**Shared decal textures** (`assets/shared/textures/decals/`):

| Key prefix | File | Shape |
|------------|------|-------|
| `circle_filled` | `circle_filled.png` | Solid disc with hard edge |
| `ring_thin` | `ring_thin.png` | Thin 10 px ring |
| `ring_thick` | `ring_thick.png` | Thick 28 px ring |
| `splat_01` | `splat_01.png` | Soft-edged disc (feathered) |
| `shockwave` | `shockwave.png` | Two concentric thin rings |

Example rule:

```ron
( on: "entity.entered:explosion_pad_01", do_actions: [
    SpawnEffect(key: "explosion_burst", entity: "explosion_pad_01"),
    ProjectDecal(key: "aoe_fire_circle", entity: "explosion_pad_01",
                 radius: 3.5, duration_secs: 3.0,
                 color: (1.0, 0.40, 0.10, 0.75), pulse_speed: 0.6),
]),
```

Decals use `LevelEntity` — they are automatically cleaned up on scene transitions.

---

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
      kind: Actor,
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
      kind: Prop,
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
| `kind` | `PrefabKind` | `Actor`, `Prop`, `Primitive`, or `Foliage` (bare enum variant, no quotes) |
| `model` | `String` | Key into `AssetCatalog.models` for `Actor`/`Prop`. Must be `""` (empty) for `Primitive` and `Foliage` — use `shape` or `foliage.trunk` instead. |
| `shape` | `Option<PrimitiveShapeKind>` | Required for `Primitive` prefabs. Write the bare variant: `Cuboid`, `Sphere`, etc. (`implicit_some` is active; no `Some()` wrapper needed). Omit for `Actor`/`Prop`/`Foliage`. See [Primitive shapes](#primitive-shapes-) below. |
| `foliage` | `Option<FoliageDef>` | Required for `Foliage` prefabs. Defines the trunk model, cluster distribution, and leaf card material. See [kind: Foliage](#kind-foliage-) below. |
| `animation_policy` | `Option<String>` | Path to `.ron` animation policy, relative to project root |
| `material` | `Option<String>` | Key into `AssetCatalog.materials` to override the model's material |
| `components.tags` | `Vec<String>` | Runtime-meaningful tags: `"player"` and `"flycam"` affect spawning; others are design-time only |
| `components.movement` | `MovementConfig` | Movement tuning for player prefabs. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.inputs` | `Option<InputMap>` | Key bindings for the player character. Only read for `"player"` prefabs. Omit to use WASD defaults. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.flycam` | `Option<FlyCamDef>` | Speed and sensitivity tuning for the free-fly camera. Only read for `"flycam"` prefabs. Omit to use defaults. See [Special tag: `"flycam"`](#special-tag-flycam-) below. |
| `components.camera` | `Option<CameraConfig>` | Orbit camera settings (offset, zoom, orbit speed, radius limits). Only read for `"player"` prefabs. Omit to use engine defaults. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.npc` | `Option<NpcDef>` | NPC AI configuration. When set, the entity gets a dynamic physics body and an NPC behaviour driver. See [NPC behaviour](#npc-behaviour-componentsnpc-) below. |
| `components.sounds` | `HashMap<String, String>` | Informational map from event name to `AssetCatalog` audio key. Not auto-wired — reference these keys in `state_machine.ron` to bind sounds to events (e.g. `player.jumped → PlaySound(key: "sfx_jump")`). |
| `primitive` | `Option<PrimitiveParams>` | Shape dimensions and appearance; only used when `kind: Primitive` |
| `children` | `Vec<ChildPrimitiveDef>` | Sub-meshes composing a composite primitive (e.g. lamp post + orb). Only used when `kind: Primitive`. See below. |
| `colliders` | `Vec<ColliderDef>` | One or more static physics colliders for `kind: Actor` / `kind: Prop`. All shapes are combined into a single Rapier compound body — use multiple entries to approximate curved geometry or multi-part shapes. Empty list = no physics. See below. |
| `behavior` | `Option<String>` | Path to a `.behavior.ron` file relative to the project root. Loads an independent per-entity FSM; `{self}` in event patterns and action keys is replaced with the entity's spawn ID. Works for all `kind` values, including composite `Primitive` prefabs with `children`. See `docs/30_runtime_events_and_logic.md`. |
| `trigger_zone` | `Option<TriggerZoneDef>` | Spawns a Rapier sphere sensor. Emits `entity.entered:{id}` / `entity.exited:{id}` when the player overlaps. Field: `radius: f32`. Works on all prefab kinds, including composite primitives (`model: ""` + non-empty `children`). |
| `interactable` | `Option<InteractableDef>` | Emits `entity.interacted:{id}` when the player is within `radius` metres and presses the interact key (default `"KeyF"`). Field: `radius: f32`. |
| `click_selectable` | `bool` | `false` | When `true`, left-clicking near this entity on screen sets it as `CurrentTarget` and emits `target.clicked:{id}`, `target.changed:{id}`, and `target.changed`. Selection is screen-space proximity (the entity nearest the cursor within ~70px), so it works for animated/skinned GLB characters as well as primitives. Clicking empty space clears the target. |
| `targetable` | `bool` | `false` | When `true`, this entity participates in Tab-cycle targeting. Pressing Tab selects the nearest `targetable` entity within `target_range` units and emits `target.changed:{id}` and `target.changed`. |
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
  kind: Prop,
  model: "",
  components: ( tags: ["flycam"] ),
),

// In prefabs/prefabs.ron — with custom speed tuning
"flycam_slow": (
  kind: Prop,
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
| `target_next` | `String` | `"Tab"` | Key to cycle to the next nearest `targetable: true` entity. Hold Shift while pressing to cycle in reverse. **Note:** `"Tab"` is intercepted by browsers for focus navigation in WASM builds — prefer another key such as `"KeyT"` (as `3rd_person_game_demo` does). |
| `target_range` | `f32` | `30.0` | Maximum world-space distance (units) for Tab targeting. Entities beyond this range are excluded. |

> **Selection is proximity-based, not a pixel-perfect mesh hit.** Left-clicking selects the `click_selectable` entity whose on-screen position is nearest the cursor (within a fixed radius), resolved from the entity's transform — so thin or animated/skinned characters are easy to click and never "fall through" to the geometry behind them. Clicking with nothing nearby clears the current target. For combat-style play, set the player camera's `orbit_button: "Right"` so left-click is free for selection (see `3rd_person_game_demo`).

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
  kind: Actor,
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
  kind: Primitive,
  model: "",
  shape: Capsule3d,
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

When `kind: Primitive`, no GLB model is loaded. Instead the runtime generates a procedural Bevy mesh from the `shape` field and the optional `primitive` parameters block. The `model` field must be `""` (empty string) for primitive prefabs.

**Supported `PrimitiveShapeKind` variants:**

| `shape` value | Shape | Key dimension fields |
|---|---|---|
| `Cuboid` | Box | `size: (x, y, z)` |
| `Sphere` | Sphere | `radius` |
| `Cylinder` | Cylinder | `radius`, `height` |
| `Capsule3d` | Capsule | `radius`, `height` (used as half_length) |
| `Cone` | Cone | `radius`, `height` |
| `Torus` | Torus / donut | `radius` (outer), `radius_top` (inner) |
| `ConicalFrustum` | Truncated cone | `radius` (bottom), `radius_top` (top), `height` |
| `Plane` | Flat plane | `size: (x, _, z)` (Y ignored) |

> **Physics collider support:** `Cuboid`, `Sphere`, `Cylinder`, and `Capsule3d` support physics/sensor colliders. `Cone`, `Torus`, `ConicalFrustum`, and `Plane` are visual-only — `physics: true` / `sensor: true` is ignored for these shapes.

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
| `shape` | `ColliderShapeKind` | — | `Cuboid`, `Sphere`, or `Cylinder` (bare enum variant, no quotes) |
| `size` | `Option<(f32,f32,f32)>` | `(1,1,1)` | Full extents (width, height, depth) for Cuboid |
| `radius` | `Option<f32>` | `0.5` | Radius for Sphere / Cylinder |
| `height` | `Option<f32>` | `1.0` | Total height for Cylinder |
| `offset` | `(f32,f32,f32)` | `(0,0,0)` | Local-space offset of this shape from the entity origin |
| `rotation_euler_deg` | `(f32,f32,f32)` | `(0,0,0)` | Euler rotation in degrees (XYZ order) for this shape's local orientation |

```ron
// Simple single-shape prop
"barrel": (
  kind: Prop,
  model: "barrel",
  components: (),
  colliders: [
    (shape: Cylinder, radius: 0.35, height: 0.9),
  ],
),

// Multi-shape prop: chest with separate base and lid colliders
"chest_01": (
  kind: Prop,
  model: "chest_01",
  components: (tags: ["loot"]),
  colliders: [
    (shape: Cuboid, size: (0.70, 0.55, 1.00), offset: (0.0, -0.125, 0.0)),
    (shape: Cuboid, size: (0.68, 0.28, 0.98), offset: (0.0,  0.275, 0.0)),
  ],
),

// Archway approximated with three boxes — diagonal brace uses rotation_euler_deg
"archway": (
  kind: Prop,
  model: "archway",
  components: (),
  colliders: [
    (shape: Cuboid, size: (0.4, 3.0, 0.4), offset: (-1.5, 1.5, 0.0)),
    (shape: Cuboid, size: (0.4, 3.0, 0.4), offset: ( 1.5, 1.5, 0.0)),
    (shape: Cuboid, size: (3.4, 0.4, 0.4), offset: ( 0.0, 3.2, 0.0)),
    (shape: Cuboid, size: (0.3, 2.0, 0.3), offset: ( 0.0, 1.5, 0.0), rotation_euler_deg: (0.0, 0.0, 45.0)),
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
| `shape` | `Option<PrimitiveShapeKind>` | `None` | Inline primitive shape — write the bare variant: `Cuboid`, `Sphere`, etc. (`implicit_some` active). Leave omitted when using `prefab`. Mutually exclusive with `prefab`. |
| `primitive` | `PrimitiveParams` | defaults | Shape dimensions and colour. Only used when `shape` is set. |
| `offset` | `(f32,f32,f32)` | `(0,0,0)` | Translation offset from the parent entity's origin |
| `rotation_euler_deg` | `(f32,f32,f32)` | `(0,0,0)` | Euler rotation in degrees (XYZ order) |
| `scale` | `(f32,f32,f32)` | `(1,1,1)` | Scale for this child |
| `material` | `Option<String>` | `None` | Key into `AssetCatalog.materials` for the child's material (inline primitives only). |
| `prefab` | `Option<String>` | `None` | **Nested prefab reference** — key into `PrefabCatalog.prefabs`. Mutually exclusive with `shape`. See below. |

The `material` field accepts the same custom/standard/terrain keys as the top-level `material` field, including `Custom` materials with WGSL shaders.

### Nested prefab references ✅

A child can reference another named prefab by key instead of defining an inline shape. All three prefab kinds are supported as nested children — `kind: Primitive` (both composite and single-shape), `kind: Actor`, and `kind: Prop` (GLB meshes). Transforms compose **multiplicatively** (standard Bevy hierarchy), so rotation and scale inherit correctly at every nesting level.

```ron
"village": (
  kind: Primitive,
  model: "",
  components: (),
  children: [
    // Inline primitive
    (
      shape: Cuboid,
      material: "mat_stone_cobble",
      primitive: (size: (18.0, 0.02, 14.0)),
      offset: (0.0, 0.01, 0.0),
    ),
    // Nested composite prefab (kind: Primitive with children)
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
| `Primitive` | yes | Anchor + all children spawned recursively |
| `Primitive` | no (single `shape`) | Anchor + one mesh child |
| `Actor` / `Prop` | — | GLB loaded via `spawn_prefab_instance`; the GLB root entity sits at the child `offset` |

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

### kind: Foliage ✅

A procedural foliage prefab: no GLB geometry is loaded for the canopy. Instead the engine builds leaf-card cluster meshes at scene load from the parameters below. An optional trunk GLB is loaded as a child entity. Leaf cards are camera-facing billboards (always face the player) shaded with a cel/toon lighting model.

```ron
// Minimal foliage — bush with no trunk
"my_bush": (
    kind: Foliage,
    foliage: (
        clusters: (
            count: 5,
            emitter_radius: 0.7,
            leaves_per_cluster: 20,
            leaf_scale_min: 0.25,
            leaf_scale_max: 0.45,
        ),
        material: (
            leaf_texture: "textures/foliage/leaf_brush_01",
            color_highlight: (0.38, 0.65, 0.20),
            color_midtone:   (0.22, 0.48, 0.10),
            color_shadow:    (0.08, 0.25, 0.04),
            toon_bands: 3,
            ao_intensity: 0.4,
        ),
    ),
),

// Full tree with trunk
"oak_tree": (
    kind: Foliage,
    foliage: (
        trunk: "models/plants/trunk_with_branches_01",
        clusters: (
            count: 7,
            emitter_radius: 1.6,
            leaves_per_cluster: 28,
            leaf_scale_min: 0.35,
            leaf_scale_max: 0.65,
            height_bias: 0.75,
            seed: 42,
        ),
        material: (
            leaf_texture: "textures/foliage/leaf_brush_01",
            color_highlight: (0.45, 0.72, 0.25),
            color_midtone:   (0.28, 0.55, 0.15),
            color_shadow:    (0.10, 0.30, 0.06),
            toon_bands: 3,
            ao_intensity: 0.45,
        ),
    ),
),
```

**`FoliageDef` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `trunk` | `Option<String>` | `None` | Asset catalog model key for the trunk GLB. Omit for bushes or pure foliage. |
| `clusters` | `FoliageClustersDef` | see below | Controls how many clusters spawn and how leaf cards are sized. |
| `material` | `FoliageMaterialDef` | see below | Leaf card texture and toon shading colours. |
| `cast_shadows` | `bool` | `true` | `true` = alpha-clipped depth prepass produces leaf-shaped shadows. `false` = `NotShadowCaster` inserted — no shadows at all (cheaper; useful for dense bushes). |

**`FoliageClustersDef` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `count` | `u32` | `6` | Number of foliage clusters to spawn. |
| `emitter_radius` | `f32` | `1.2` | Crown radius in metres — how far apart clusters are spread (not a particle emitter). |
| `crown_height` | `f32` | `0.0` | Lifts the emitter sphere above the entity origin, in metres. Set this to roughly where the trunk meets the branches. Bushes use `0.0`; trees typically need `1.5`–`2.5`. |
| `leaves_per_cluster` | `u32` | `24` | Number of leaf cards baked into each cluster mesh. Must be > 0. |
| `leaf_scale_min` | `f32` | `0.3` | Minimum leaf card size in metres. |
| `leaf_scale_max` | `f32` | `0.6` | Maximum leaf card size in metres. Cards are randomly sized between min and max. |
| `height_bias` | `f32` | `0.7` | Biases cluster placement toward the top of the sphere. `0.0` = full sphere (clusters appear below the trunk base); `1.0` = upper hemisphere only. `0.7` is a good default for trees; `0.5` works well for bushes. |
| `seed` | `u32` | `0` | Rotates the Fibonacci placement pattern. Two trees of the same prefab with different `seed` values will have different cluster arrangements. Change the seed to get a different silhouette. |

**`FoliageMaterialDef` fields:**

| Field | Type | Description |
|---|---|---|
| `leaf_texture` | `String` | Asset catalog texture key. Must be an **RGBA PNG** where the leaf shapes are opaque and the background is transparent (alpha = 0). RGB images have no transparency and render as solid rectangles. |
| `color_highlight` | `(f32, f32, f32)` | RGB colour for the brightest lit areas (facing the sun). Linear sRGB 0–1. Note: **RGB, not RGBA** — no alpha component. |
| `color_midtone` | `(f32, f32, f32)` | RGB colour for the mid-tone transition zone. |
| `color_shadow` | `(f32, f32, f32)` | RGB colour for the darkest shadowed areas (facing away from the sun). |
| `toon_bands` | `u8` | Number of discrete shading bands. **Must be 2, 3, or 4.** 3 is the standard anime look; 2 is more graphic; 4 adds a subtle extra highlight band. |
| `ao_intensity` | `f32` | How much ambient occlusion darkens the shadow side. `0.0` = no AO; `1.0` = maximum darkening. Values around `0.4`–`0.5` look natural. |

> **Validation:** the engine rejects a Foliage prefab at load time if `leaf_texture` is empty, `leaves_per_cluster` is 0, or `toon_bands` is outside 2–4.

See `assets/projects/foliage_demo/` for a working demo with oak trees, autumn trees, and bushes.

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
| `ShowFloatingText(entity: "id", text: "msg")` | Spawns a floating text label above the entity with the given spawn ID. Rises and fades using the same animation as `ShowDamagePopup`. Colour is warm yellow; use `ShowDamagePopup` for numeric health feedback. Uses `{self}` and `{target}` substitution. |
| `SetEntityVisible(entity: "id", visible: bool)` | Shows (`true`) or hides (`false`) a spawned entity by its spawn ID. The entity stays in the ECS — colliders and behavior FSM keep running. World-space labels tracking that entity (stat bar, stat label) auto-hide automatically. Uses `{self}` in behavior files. |
| `EmitEventAfterDelay(event: "name", delay_secs: f32)` | Fires a `GameEvent::Trigger("name")` after `delay_secs` seconds. One-shot — fires once then is removed. Cleared on `Action::LoadScene` so delayed events do not leak across scene transitions. Uses `{self}` substitution in behavior files. |
| `SpawnEffect(key: "key", position/entity)` | Spawn a particle burst from `assets.ron effects`. Quality multiplier and budget gating are applied at spawn time. See the Particle System section. |
| `ProjectDecal(key: "key", …)` | Spawn a flat ground-projected texture quad. See the Ground Decals section. |
| `SetParticleQuality(Level)` | Set the global quality tier (`High`, `Medium`, `Low`, `Minimal`). Persists across scene transitions. Affects all subsequent `SpawnEffect` calls. |
| `SetVolume(0–100)` | Set the global audio volume (percent). Scales against the project's `max_volume` ceiling — `SetVolume(100)` equals `max_volume`. Emits `audio.volume_changed`. |
| `ToggleMute` | Toggle muted state. Muting emits `audio.muted`; unmuting restores the previous volume and emits `audio.unmuted`. |
| `SyncAudioState` | Re-emit the current mute state (`audio.muted` or `audio.unmuted`) without changing it. Use in state `entry_actions` to initialise bound audio labels on first load — combine with a `global_on` bridge that maps the event to `SetVariable`. |
| `ApplyModifier(modifier_key: "key")` | Apply a named stat modifier template to its target stat. |
| `RemoveModifier(modifier_key: "key")` | Remove all active instances of a named modifier. |
| `SetTarget("spawn_id")` | Set `CurrentTarget` to the given spawn ID. Emits `target.changed:{id}` and `target.changed`. |
| `ClearTarget` | Clear `CurrentTarget`. Emits `target.cleared`. Also cleared automatically on `LoadScene`. |

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

## `AudioConfig`

Optional block in `{name}.project.ron` that controls project-level audio volume. All fields have built-in defaults — omit the block entirely to use them.

```ron
// {name}.project.ron
(
    schema_version: 3,
    ...
    audio: (
        max_volume:    0.8,    // project ceiling; 0.0–1.0, default 1.0
        mute_on_start: false,  // default false
    ),
)
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_volume` | `f32` | `1.0` | Master volume ceiling (0.0–1.0). `SetVolume(100)` equals this value, not 1.0. Lets you tune overall project volume without touching individual audio source files. |
| `mute_on_start` | `bool` | `false` | Start the project muted. Equivalent to firing `ToggleMute` once immediately on project load. |

**Pipeline events** emitted by audio actions:

| Event | Trigger |
|-------|---------|
| `audio.muted` | `ToggleMute` transitions to muted, or `SyncAudioState` while already muted |
| `audio.unmuted` | `ToggleMute` transitions to unmuted, or `SyncAudioState` while not muted |
| `audio.volume_changed` | `SetVolume` changes the active fraction |

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

---

## Migrating `prefabs.ron` from schema_version 1 to 2

`PrefabCatalog` was bumped from version 1 to 2. Two sets of changes are required in every `prefabs.ron`:

### 1. `kind` field — quoted string → bare enum variant

```ron
// Before (v1)           // After (v2)
kind: "actor"      →     kind: Actor
kind: "prop"       →     kind: Prop
kind: "primitive"  →     kind: Primitive
```

### 2. Collider `shape` field — quoted string → bare enum variant

```ron
// Before (v1)           // After (v2)
shape: "Cuboid"    →     shape: Cuboid
shape: "Sphere"    →     shape: Sphere
shape: "Cylinder"  →     shape: Cylinder
```

### 3. Primitive shape — `model:` → `shape:` + `model: ""`

For top-level `kind: Primitive` prefabs that have a shape name in `model:`:

```ron
// Before (v1)
"my_cube": (
    kind: "primitive",
    model: "Cuboid",
    primitive: ( size: (2.0, 1.0, 2.0) ),
)

// After (v2)
"my_cube": (
    kind: Primitive,
    model: "",
    shape: Cuboid,
    primitive: ( size: (2.0, 1.0, 2.0) ),
)
```

### 4. Child `shape` — quoted string → bare enum variant

For `ChildPrimitiveDef` entries in `children:` lists:

```ron
// Before (v1)
( shape: "Sphere", primitive: (radius: 0.5), ... )

// After (v2)
( shape: Sphere, primitive: (radius: 0.5), ... )
```

Composite primitives (those that only have `children:`, no top-level `shape:`) do not need a `shape:` field — only single-mesh primitives do.

### 5. Bump `schema_version`

```ron
schema_version: 1  →  schema_version: 2
```
