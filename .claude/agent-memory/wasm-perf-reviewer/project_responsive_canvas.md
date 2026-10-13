---
name: responsive-canvas-render-cost
description: fit_canvas_to_parent cost (big HiDPI desktops are the fill-rate regression) + CSS-scale pixel-cap findings (winit offsetX mapping OK, logical-size shrink, Sample2 invalid on WebGPU, test_real_hidpi coupling)
metadata:
  type: project
---

Responsive UI Phase 1 (2026-10-11) set Window.canvas="#ironhold-canvas" + fit_canvas_to_parent in start_app.
Before, the canvas was Bevy-default 1280x720 logical (no CSS scaling), so backing store was 1280x720 x DPR.

- Phones got CHEAPER: old 1280x720 @DPR3 = 3840x2160 = 8.3 MP; full-viewport 390x800 @DPR3 ~ 2.8 MP.
- Large desktops got MORE expensive: 4K @DPR1.5 viewport ~3840x2094 = 8.0 MP vs old 1920x1080 = 2.07 MP (~3.9x).
- No Msaa override in crates → Bevy default Sample4; rough ~44 B/px of view targets (MSAA color+depth, 2 resolve ping-pong, swapchain) → ~350 MB at 8 MP, ~160 MB at 3.7 MP, ~92 MB at 2.1 MP.
- Shadow maps are fixed-size resources (scene_loader DirectionalLightShadowMap), NOT resize-dependent.
- sync_debug_state_to_dom (wasm, PostUpdate) is a per-frame format!+set_inner_html (pre-existing).

Pixel-cap via CSS `transform: scale(k)` wrapper (plan prerelease_render_fixes phase C, reviewed 2026-10-13):
- winit 0.30.12 pointer/touch positions = `offsetX/offsetY` (web_sys/event.rs:83-98), × window DPR. offsetX is in the
  target's untransformed local space in Blink/Gecko/WebKit → mapping is correct under an ancestor scale.
- Resize uses ResizeObserver DevicePixelContentBox (resize_scaling.rs:141-143), Safari fallback contentRect×DPR
  (:231-235); both are pre-transform → backing store = (W/k)·DPR as intended.
- Mouse-look: Chromium movementX is screen-space (unaffected); Gecko coalesced path diffs offsetX (event.rs:131-137)
  → delta shrinks by 1/k at k>1 (minor Firefox sensitivity drift).
- Cap shrinks the LOGICAL window by k: non-ui_scale projects' Val::Px UI appears k× larger; HiDPI laptops
  (1512x982@2) drop to ~1192x774 logical at 3.7 MP, ~899x584 at 2.1 MP (below 1280x720) → prefer 3.7 MP + a logical floor.
- WebGPU only allows sample counts 1 and 4 → Msaa::Sample2 is NOT a web option.
- test_real_hidpi (test_web.py ~L810) measures getBoundingClientRect (transformed) vs window_size/backing → breaks at k>1.

Implemented 2026-10-13 (feature/prerelease_render_fixes, post-impl review): play.html `.canvas-scaler` wrapper,
BUDGET_PX 3,686,400, k ceil to 0.05, logical floor 1280x720, `window.ironholdRenderScale`, `?px_budget=off|<MP>`.
bevy_winit 0.18 fit_canvas_to_parent only sets canvas style width/height 100% (system.rs:104) → parent = scaler, fine.
Only play.html applies the cap (index/other embeds uncapped). transform: scale(1) is non-none even at k=1 (stacking
context + containing block always). test_real_hidpi now runs with px_budget=off; test_render_budget covers DPR1 4K.
Stray-camera fix landed same batch: old `AmbientLight` entity spawn `#[require(Camera)]` → per-scene extra active
Camera (Frustum+VisibleEntities) — removing it is a per-frame visibility-pass win, and it leaked into Query<&Camera>.

**How to apply:** cap total physical pixels, not DPR; check any canvas-size test for getBoundingClientRect vs clientWidth.
See [[project-dynamic-split-screen]].
