---
name: web-host-page-contract
description: Reviewing web canvas/host-page changes (Window.canvas selector, fit_canvas_to_parent, play.html, data-ready) — what counts as designer surface vs host contract
metadata:
  type: project
---

responsive_ui Phase 1 (reviewed 2026-10-11) hardcodes `Window.canvas = "#ironhold-canvas"` + `fit_canvas_to_parent: true` in `start_app` (ironhold_core/src/lib.rs). Judged ALIGNED: the HTML host page is a deployment contract, not game design, so no RON knob is needed; the fields are no-ops on native so no platform-code violation.

**Why:** designers shape games via RON; the canvas element lives in play.html, which a designer doesn't author per project.

**How to apply:** don't demand RON surfaces for host-page plumbing or test-only DebugState fields (`window_size`, `ui_scale`). DO flag (a) any custom host page (itch.io embed) must contain that canvas id, since bevy_winit errors on a missing selector — document it; (b) designer-visible consequences such as "play area is no longer fixed 1280x720, absolute-px UI can clip" need a line in designer docs, not only docs/dev/browser_tests.md. Phase 2 (`ui_scale` block with reference/fit/snap) IS designer surface — review it with the full schema/CLI checklist. Related: [[ui-flex-group-walker]], [[diagnostic-only-feature]].
