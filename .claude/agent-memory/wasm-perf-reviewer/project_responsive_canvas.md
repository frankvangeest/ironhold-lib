---
name: responsive-canvas-render-cost
description: Responsive UI Phase 1 (fit_canvas_to_parent) — backing store = CSS x DPR; big desktop monitors (not phones) are the real fill-rate regression; resize cost; debug-state DOM write
metadata:
  type: project
---

Responsive UI Phase 1 (2026-10-11) set Window.canvas="#ironhold-canvas" + fit_canvas_to_parent in start_app.
Before, the canvas was Bevy-default 1280x720 logical (no CSS scaling), so backing store was 1280x720 x DPR.

- Phones got CHEAPER: old 1280x720 @DPR3 = 3840x2160 = 8.3 MP; full-viewport 390x800 @DPR3 ~ 2.8 MP.
- Large desktops got MORE expensive: 4K @DPR1.5 viewport ~3840x2094 = 8.0 MP vs old 1920x1080 = 2.07 MP (~3.9x).
- No Msaa override in crates → Bevy default Sample4; rough ~44 B/px of view targets (MSAA color+depth, 2 resolve ping-pong, swapchain) → ~350 MB at 8 MP.
- Shadow maps are fixed-size resources (scene_loader DirectionalLightShadowMap), NOT resize-dependent.
- No WindowResized readers in crates; split_screen_viewport_system polls physical_size every frame (cheap).
- sync_debug_state_to_dom (wasm, PostUpdate) was ALREADY a per-frame format!+set_inner_html before Phase 1; Phase 1 only adds 3 floats.

**Why:** Phase 4 "mobile render cap" framing assumed phones are the worst case; it's actually large/HiDPI desktop.
**How to apply:** when render-scale cap lands, cap total physical pixels (e.g. ~2.1-3.7 MP budget), not just DPR on mobile. See [[project-dynamic-split-screen]].
