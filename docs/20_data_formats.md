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

