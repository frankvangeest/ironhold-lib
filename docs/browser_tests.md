# Browser Test Suite (`test_web.py`)

Runs headless Chromium tests against the built WASM package. Requires `playwright install chromium` (one-time).

## Test categories

| Category | Tests | What it checks |
|----------|-------|----------------|
| `smoke` | one per project | Page loads, `<canvas>` appears, `app_state` reaches `InGame`, no JS/Rust errors |
| `action` | `dance_button` | Clicking the Dance button (canvas coords) fires `PlayAnimation` via the rules pipeline |
| `transition` | `start_game` | Clicking Start Game transitions `start_menu.scene.ron` → `main.scene.ron` |
| `baseline` | one per project | Screenshot diff vs stored baseline stays under 2% changed pixels |
| `navigation` | `pause_menu_flow` | Full menu flow: start menu → main → Esc (pause) → Esc (close) → Esc (pause) → Resume; screenshot at each step |

## Screenshot layout

```
screenshot_baselines/scenes/      ← committed to git; scene baselines used in gallery
screenshot_baselines/pause_nav/   ← committed to git; navigation step baselines
screenshots/                      ← gitignored; current/comparison files written here
```

Run `python test_web.py --update-baselines` after any intentional rendering change.
Run `python test_web.py --update-baseline <name>` to update a single project or `pause_nav`.
Run `python test_web.py --project <name>` to restrict all test categories to one project (repeatable). Useful when iterating on a single project: `python test_web.py --project entity_logic_demo --update-baselines --skip-build`.

## `DebugState` resource

The test harness reads a hidden `<div id="debug-state">` updated every frame by the WASM runtime:
```json
{"frame": 42, "app_state": "InGame", "last_action": "PlayAnimation(\"dance\")", "scene": "projects/quick_scene/scenes/main.scene.ron"}
```
Written by `sync_debug_state_to_dom` (WASM-only, `PostUpdate`) in `ironhold_core/src/lib.rs`.

## URL project selection

The WASM build reads `?project=<name>` from the URL (e.g. `?project=terrain_demo`) and passes it to `start_app`. Implemented in `ironhold_web/src/lib.rs`.

## Canvas coordinate clicks

Bevy UI renders inside the WebGPU canvas, not as DOM elements. Button clicks in tests must use `page.mouse.click(x, y)` with coordinates derived from the scene's `position` + `size/2` fields.
