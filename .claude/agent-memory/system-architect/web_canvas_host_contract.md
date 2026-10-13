---
name: web-canvas-host-contract
description: start_app hardcodes Window.canvas="#ironhold-canvas" (responsive_ui Phase 1); bevy_winit 0.18 PANICS if the selector is missing, so every host page is now coupled to that id
metadata:
  type: project
---

Since responsive_ui Phase 1 (reviewed 2026-10-11), `ironhold_core::start_app` sets `Window { canvas: Some("#ironhold-canvas"), fit_canvas_to_parent: true }` unconditionally. Both fields are cfg(wasm32)-only inside bevy_winit (`winit_windows.rs` ~L277), so native is unaffected.

Verified fact: bevy_winit 0.18 does `panic!("Cannot find element: {selector}.")` when the selector matches nothing. So a NEW pkg on a page without that canvas panics before frame 1; an OLD pkg on the new play.html just appends its own canvas (the case the docs mention). Only play.html hosts the engine today (index.html is a gallery).

Since prerelease_render_fixes phase C (2026-10-13): play.html wraps the canvas in `#ironhold-scaler` (CSS `transform: scale(k)`, 1/k size) to cap backing-store pixels at 3,686,400 with a 1280x720 logical floor; `window.ironholdRenderScale` exposes k, `?px_budget=off|<MP>` overrides. Core untouched. A second host page renders uncapped unless it copies the wrapper + classic-script JS (which must run before the module `init()`).

**Why:** the selector is a host-page (ironhold_web/play.html) contract living in core; any future embedder page or second HTML host inherits a silent hard requirement.

**How to apply:** when reviewing new HTML hosts or embedding work, check for `<canvas id="ironhold-canvas">`; preferred long-term fix is to pass the selector from ironhold_web into start_app (or a pub const in core documented as the host contract). Related: [[demo-project-baseline-determinism]] (harness assumes 1280x720 canvas under ?testing=1).
