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
`play.html` shows a static navigation bar above the game canvas; the suite loads pages with `?testing=1`, which removes it (and the loading overlay) so canvas coordinates are exact. Add `?bar=hidden` to hide only the bar when playing by hand.
Run `python test_web.py --project <name>` to restrict all test categories to one project (repeatable). Useful when iterating on a single project: `python test_web.py --project entity_logic_demo --update-baselines --skip-build`.

## Rendering backend flags

By default the test suite runs headless Chromium with GL/ANGLE (WebGL2). Two flags select alternative backends:

| Flag | Backend | Headless | When to use |
|------|---------|----------|-------------|
| _(none)_ | GL/ANGLE (WebGL2) | yes | Everyday CI loop; fast, stable baselines |
| `--webgpu` | SwiftShader via Vulkan/Dawn | yes | Verify the WebGPU code path (e.g. after enabling deferred rendering); no GPU required |
| `--real-gpu` | Real D3D12/Vulkan GPU | **no** | Final hardware validation; requires a display |

`--webgpu` and `--real-gpu` are mutually exclusive.

> **Build note:** `--webgpu` in `test_web.py` selects the Chromium rendering backend only. The default WASM build already uses `--features webgpu`, so the Bevy and Chromium backends match out of the box:
> ```bash
> python test_web.py --webgpu --skip-build   # uses the existing pkg/ (built with --features webgpu)
> ```
> To test the WebGL2 fallback path, build without the feature flag first:
> ```bash
> wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev  # no --features webgpu
> python test_web.py --skip-build   # GL/ANGLE backend matches the WebGL2 WASM build
> ```
> WebGPU builds require Chrome 113+ or Edge 113+ — Firefox and Safari are not fully supported.

> **Baseline note:** `--real-gpu` (WebGPU in a visible Chromium window, using the standard `--features webgpu` dev build) is now the preferred way to capture new baselines and to run the suite interactively; headless GL/ANGLE with a WebGL2 build is the fallback / CI path. Baselines captured before 2026-10-06 (all except `ui_demo_main` and `3rd_person_game_demo_options`) came from headless GL/ANGLE, so a `--real-gpu` run may show pixel-level differences on scenes you did not change (observed: `pause_nav/02_main_scene` at 5.15% vs the 4% threshold, a stale baseline — it predates the loaded action-bar icons — on top of backend differences). Update only the baselines for what your change affected (`--project <name>` / `--update-baseline <name>`); regenerating *all* baselines under `--real-gpu` is a separate cleanup.
>
> **Machine note (2026-10-06):** headless `--webgpu` (SwiftShader) fails here with `No available adapters. Unable to find a GPU!` and the default GL mode needs a WebGL2 build, so on this machine use `--real-gpu`.
>
> **Ports:** the suite starts its own `serve.py` on port **8001** (`--port N` to change), separate from the default 8000 you use for `python serve.py` playtests. `test_web.py` does not detect a failed bind, so a leftover server on its port makes it silently use that server.

## `DebugState` resource

The test harness reads a hidden `<div id="debug-state">` updated every frame by the WASM runtime:
```json
{"frame": 42, "app_state": "InGame", "last_action": "PlayAnimation(\"dance\")", "scene": "projects/quick_scene/scenes/main.scene.ron", "logic_state": "", "score": 0, "window_size": [1280, 720], "ui_scale": 1}
```
`window_size` is the engine's logical window size and `ui_scale` its `UiScale`; the responsive tests wait on them to know a resize has landed.
Written by `sync_debug_state_to_dom` (WASM-only, `PostUpdate`) in `ironhold_core/src/lib.rs`.

## Responsive matrix (`--responsive`)

`python test_web.py --responsive --real-gpu --skip-build` runs **instead of** the main suite: one new browser context per viewport/device (`RESPONSIVE_MATRIX`: 1920x1080, 1024x768, 768x1024, 844x390 and 390x844 with mobile emulation + touch) for `ui_demo` and the `3rd_person_game_demo` main scene. Each entry asserts the engine's `window_size` equals the viewport, `ui_scale` is 1.0, there is exactly one canvas that fills the viewport with no page scroll, and diffs a screenshot against `screenshot_baselines/responsive/{project}_{scene}_{viewport}.png` (its own folder; the gallery reads `scenes/`). It also runs a resize-in-place test (one page, five viewport sizes) and a **touch** test (`touchscreen.tap` on quick_scene's Dance button). Baselines are GPU-captured, so use `--real-gpu`; `--update-baseline responsive` (all) or an entry key such as `ui_demo_main_390x844` re-captures, and `--project` limits the projects. Re-capture the `3rd_person_game_demo` entries when its HUD is retrofitted onto `Group`.

**HiDPI is tested with a real device scale factor, never with Playwright's `device_scale_factor`.** That emulation makes `ResizeObserver`'s `devicePixelContentBoxSize` report the CSS size (an 800x600 element reports `[800,600]` at 1x, 2x and 3x), which is what winit's web backend reads for the canvas's physical size, so the engine would see a window of CSS/dpr (UI 2x too large and cut off). A real display reports CSS x dpr and the engine is correct. `test_real_hidpi` therefore launches a separate Chromium with `--force-device-scale-factor=2` (and `=3`) and `no_viewport=True`, and asserts the engine `window_size` equals the canvas CSS size and the canvas backing store is CSS x dpr. Do not add a `device_scale_factor` entry to the matrix.

## Canvas host (`#ironhold-canvas`)

`play.html` is exactly one viewport tall: the 44px top bar (hidden by `?bar=hidden` and `?testing=1`) above a `.game-host` flex box that takes the rest, with the engine's `<canvas id="ironhold-canvas">` inside it. `start_app` sets `Window.canvas = "#ironhold-canvas"` and `fit_canvas_to_parent: true`, so Bevy sizes the canvas to 100% of the host and it follows browser resizes. Under `?testing=1` the host is the whole viewport, so the suite's fixed 1280x720 context still yields a 1280x720 canvas (any other size makes `compare_screenshots` return `1.0`). **`play.html` needs a `pkg/` built from the same commit:** a stale `pkg/` does not know the selector and appends its own fixed-size canvas to `<body>`, leaving the authored one empty. The reverse also holds: a **new** `pkg/` on a page without `<canvas id="ironhold-canvas">` panics before its first frame (`bevy_winit` 0.18: "Cannot find element: #ironhold-canvas"), so any other page that hosts the engine (an embed, a second runner page) must contain that canvas. Only `play.html` hosts the engine today. Rebuild before running the suite after pulling a change to either file.

## Readiness signal (`body[data-ready]`)

"The engine is ready" means its first frame has run, **not** that a `<canvas>` exists (the canvas can be in the DOM before the wasm has loaded). `play.html` sets a `data-ready` attribute on `<body>` the first time `#debug-state` has content (the first `sync_debug_state_to_dom` write), and hides its loading overlay at the same moment. `test_web.py`'s `open_project` waits on `body[data-ready]` (state `attached`) instead of `canvas`. If a run times out waiting for it, the engine never completed a frame (wasm load failure, no WebGPU adapter, a panic before the first frame); check the console errors the harness collected.

## URL project selection

The WASM build reads `?project=<name>` from the URL (e.g. `?project=terrain_demo`) and passes it to `start_app`. Implemented in `ironhold_web/src/lib.rs`.

## Canvas coordinate clicks

Bevy UI renders inside the WebGPU canvas, not as DOM elements. Button clicks in tests must use `page.mouse.click(x, y)` with coordinates derived from the scene's `position` + `size/2` fields.
