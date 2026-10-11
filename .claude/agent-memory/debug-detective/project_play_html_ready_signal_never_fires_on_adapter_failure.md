---
name: play-html-ready-signal-never-fires-on-adapter-failure
description: play.html's body[data-ready]/overlay-hide keys off the first #debug-state write (PostUpdate); on WebGPU-adapter failure no update ever runs, so the "Initializing engine" spinner stays forever with no error
metadata:
  type: project
---

Since feature/responsive_ui (2026-10-11), play.html authors `<canvas id="ironhold-canvas">` and hides the loading
overlay only when `sync_debug_state_to_dom` first writes `#debug-state`. Before, the trigger was "any <canvas> exists",
which fired once winit created the window even if rendering then failed (black page).

**Why it matters:** in Bevy's wasm winit runner, `app.update()` is skipped until all plugins are ready; RenderPlugin's
async renderer init never completes when no WebGPU adapter exists (Firefox/Safari, headless Chromium here, the
`--features webgpu` Pages build), so PostUpdate never runs, `data-ready` is never set, and the overlay spins forever.
Reasoned from the runner design, not probed in-browser.

**How to apply:** when a playtest report says "stuck on Initializing engine", check the console for an adapter/render
error first, not the project RON. Any future "engine failed" UI should hook the import/init promise rejection and a
timeout, not the debug-state observer. Related: [[project-bevy-wasm-all-logs-are-console-log]],
[[project-browser-pixel-probe-recipe]].
