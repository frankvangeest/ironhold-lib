# Data Formats

## Status
🧭 Spec Draft (not fully implemented)

## Versioning
All **top-level** data formats must include a schema version:
- `schema_version: 1` (integer)

This allows backward-compatible evolution and safe validation.

## assets/project.ron (ProjectConfig)

> [!NOTE]
> You can override the project file path on the command line:
> `cargo run -p ironhold_native -- project_02.ron`
> `project_02.ron` should be in the `assets` directory.

Purpose:
- Defines the initial scene.
- Defines project-level settings (future: global logic machines, input profiles, networking mode).

Minimum:
- `schema_version: 1`
- `initial_scene: "scenes/start-menu.ron"`

Optional:
- `global_environment: EnvironmentMapConfig` - Fallback environment map lighting across all scenes if they don't explicitly define one. (See Lighting section below).

Future additions (planned):
- `global_logic: "logic/global.ron"`
- `input_profiles: {...}`
- `networking: { mode: "offline|client|server", tick_rate: 60 }`

## assets/scenes/*.ron (GameLevel)

Purpose:
- Declaratively defines entities to spawn: models, UI, player, camera config.

Recommended stable subset:
- `schema_version: 1`
- `models: [{ path, position: (x, y, z) }]`
- `ui: [UiElement]`
- `player: PlayerConfig?`
- `lighting: LightingConfig?`

Future additions (planned):
- `entities: [...]` (generic entity definitions)
- `behaviors: [...]` (per-entity behavior machine references)
- `triggers: [...]`

## Lighting (Scene and Project)

The engine supports data-driven HDR lighting via the `LightingConfig` block in scene `.ron` files, and a fallback `global_environment` in `project.ron`.

### Scene Lighting (`lighting: LightingConfig`)
Configures the lights spawned when a scene loads:
- `ambient: Option<AmbientLightConfig>`
  - `color: (r, g, b)` (linear RGB)
  - `brightness: f32` (lux)
- `directional: Option<DirectionalLightConfig>`
  - `color: (r, g, b)`
  - `illuminance: f32` (lux, e.g., 50000.0 for sun)
  - `direction: (x, y, z)` (normalized vector pointing *towards* the light target)
  - `shadows: bool` (whether to cast shadows)
- `environment: Option<EnvironmentMapConfig>`
  - Overrides the project's global environment map for this specific scene.

### Environment Maps (`EnvironmentMapConfig`)
Provides realistic Image-Based Lighting (IBL) reflections and ambient fill.
- `asset_path: Option<String>` – Path to a `.ktx2` cubemap texture.
- `fallback: Option<EnvironmentFallbackConfig>` – Generates a procedural sky/environment map if `asset_path` is empty or the file fails to load.
  - `intensity: f32`
  - `sun_direction: (x, y, z)` (Affects procedural sky gradient)


## UI

### Current (implemented)
- Buttons with action `Trigger("string_id")`.
- Triggers are mapped to engine actions in `project.ron`.

#### Button Properties
- `text: String` – The text displayed on the button.
- `action: UiAction` – The action to trigger (e.g., `Trigger("id")`).
- `position: Option<(f32, f32)>` – Absolute pixel position `(x, y)`. Defaults to centered.
- `width: Option<f32>` – Width in pixels. Defaults to `200.0`.
- `height: Option<f32>` – Height in pixels. Defaults to `65.0`.
- `font_size: Option<f32>` – Font size. Defaults to `26.0`.
- `border_color: Option<(f32, f32, f32, f32)>` – RGBA tuple `(0.0-1.0)`. Defaults to Black.
- `background_color: Option<(f32, f32, f32, f32)>` – RGBA tuple `(0.0-1.0)`. Defaults to Dark Grey.
- `text_color: Option<(f32, f32, f32, f32)>` – RGBA tuple `(0.0-1.0)`. Defaults to Light Grey.

Example:
```ron
ui: [
  Button(
    text: "Start Game",
    action: Trigger("start_game"),
    position: Some((100.0, 100.0)),
    width: Some(300.0),
    height: Some(65.0),
    font_size: Some(26.0),
    background_color: Some((0.1, 0.1, 0.1, 0.9)),
  ),
  Button(
    text: "Quit",
    action: Trigger("quit"),
    position: Some((100.0, 200.0)),
    width: Some(300.0),
  )
]
```

### Mapping Triggers to Actions (`project.ron`)
UI triggers are handled by the `rules` list in your `project.ron` file.

```ron
rules: [
  (
    on: "ui.button_pressed:start_game",
    do_actions: [ Log("Start pressed"), LoadScene("scenes/main.ron") ],
  ),
  (
    on: "ui.button_pressed:quit",
    do_actions: [ Quit ],
  ),
]
```

## Action Examples

The following actions are available in the current engine ABI:

- `LoadScene(String)`
- `Quit`
- `Log(String)`
- `Spawn(String)`
- `PlayAnimation(String)`

Example (RON):

```ron
rules: [
  (
    on: Trigger("start_game"),
    do_actions: [
      Log("Start pressed"),
      Spawn("assets/models/character-01.glb"),
      PlayAnimation("idle"),
      LoadScene("assets/scenes/main.ron"),
    ],
  ),
]
```

Notes:
- `Spawn(String)` expects an asset identifier/path (e.g. a `.glb`) that the runtime can load.
- `PlayAnimation(String)` expects an animation/clip name that exists on the target entity.



# Model Fixups (ProjectConfig)

`model_fixes` provides per-asset transform corrections that are applied to **every instance** of the referenced model across all scenes. Use this to compensate for authoring issues such as off-center pivots, wrong up-axis, or unit scale mismatches.

- **Key**: asset path as referenced in scenes (e.g., `models/my.glb#Scene0`).
- **Fields**:
  - `pivot_offset: (f32, f32, f32)` – meters; applied as child local translation.
  - `rotation_deg: (f32, f32, f32)` – Euler degrees, order **YXZ**.
  - `scale: (f32, f32, f32)` – local scale.

At runtime, instances are spawned with a **parent (instance transform)** and **child (GLB scene)**. The fixup is applied to the child’s local transform so it persists across resets and keeps gameplay transforms clean.