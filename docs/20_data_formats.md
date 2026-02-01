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

Future additions (planned):
- `global_logic: "logic/global.ron"`
- `input_profiles: {...}`
- `networking: { mode: "offline|client|server", tick_rate: 60 }`

## assets/scenes/*.ron (GameLevel)

Purpose:
- Declaratively defines entities to spawn: models, UI, player, camera config.

Recommended stable subset:
- `schema_version: 1`
- `models: [{ path, position, rotation?, scale? }]`
- `ui: [UiElement]`
- `player: PlayerConfig?`

Future additions (planned):
- `entities: [...]` (generic entity definitions)
- `behaviors: [...]` (per-entity behavior machine references)
- `triggers: [...]`

## UI

### Current (implemented)
- Buttons with action `LoadScene("scenes/main.ron")`
- Buttons with action `Quit`

Example:
```ron
ui: [
  Button(
    text: "Start Game",
    action: LoadScene("scenes/main.ron"),
  ),
  Button(
    text: "Quit",
    action: Quit,
  )
]
```

### Planned
- UI emits richer `UiMessage` events with stable IDs.
- Global logic decides what actions happen in response.

## Action Examples (v0.2)

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