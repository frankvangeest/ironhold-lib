---
name: mousewheel-unit-platform-reality
description: What winit 0.30/bevy_winit 0.18 actually put in MouseWheel.y per platform — native Windows is always ±1.0 per notch (Line), web is the browser's raw deltaY × devicePixelRatio (Chrome=Pixel ~100-200, Firefox=Line 3)
metadata:
  type: project
---

Verified against `~/.cargo/registry/src/*/winit-0.30.12` and `bevy_winit-0.18.0` (bevy_winit
`state.rs:338-350` passes both units through verbatim — no scaling of its own):

- **Native Windows** (`winit/src/platform_impl/windows/event_loop.rs:1729`): raw `WM_MOUSEWHEEL`
  delta is divided by `WHEEL_DELTA` (120) → `MouseScrollUnit::Line`, `y == 1.0` for one notch,
  **always**. The OS "lines to scroll per notch" setting is never consulted. High-resolution /
  precision wheels send sub-120 deltas → legitimate **fractional** Line values (0.25, 0.5).
  So "Line y can be inflated far past 1.0 on Windows" is false — do not accept that as a root cause.
- **Web / WASM** (`winit/src/platform_impl/web/web_sys/event.rs:146-161`): the browser's
  `WheelEvent.deltaY` is negated and passed through. `DOM_DELTA_LINE` → `Line` unscaled;
  `DOM_DELTA_PIXEL` → `Pixel` **multiplied by `devicePixelRatio`**. Chrome/Edge report
  `DOM_DELTA_PIXEL` with `deltaY ≈ 100` per notch → `Pixel y = 150` at 150% Windows display
  scaling, `200` at 200%. Firefox/Windows reports `DOM_DELTA_LINE` with `deltaY = 3`.

**Why:** Frank playtests in the browser (workflow step 9/13, `serve.py`), so wheel bugs reported
"on this platform" are almost always the **web Pixel** path, not the native Line path — and the
magnitude is DPI-scaling- and browser-dependent, never a fixed per-notch constant.

**How to apply:** when a scroll/zoom bug is reported, work out which unit branch actually fires on
the reproduction surface before believing a fix that only bounds the other branch. Any code that
claims to produce a "platform-independent notches" value must handle the DPI factor on the Pixel
branch, or Chrome and Firefox will differ several-fold on identical hardware. Related:
[[dt-scaled-discrete-impulse]].
