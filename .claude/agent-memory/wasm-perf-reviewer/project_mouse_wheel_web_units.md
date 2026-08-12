---
name: project-mouse-wheel-web-units
description: MouseWheel on the WASM build reports MouseScrollUnit::Pixel scaled by devicePixelRatio (native Windows reports Line) — any wheel normalization must handle the Pixel branch DPI-relatively
metadata:
  type: project
---

`bevy::input::mouse::MouseScrollUnit` is fully available on `wasm32` (no cfg gating in
`bevy_input`), but **which variant the engine receives differs per platform**, verified in the
vendored sources for this project's pinned bevy 0.18 / winit 0.30.12:

- `winit-0.30.12/src/platform_impl/web/web_sys/event.rs::mouse_scroll_delta` maps browser
  `WheelEvent`: `DOM_DELTA_LINE` → `LineDelta`, `DOM_DELTA_PIXEL` → `PixelDelta`, and
  **`DOM_DELTA_PAGE` → `None` (event silently dropped)**.
- The `DOM_DELTA_PIXEL` path calls `.to_physical(scale_factor(window))`, i.e. the pixel delta is
  **multiplied by `devicePixelRatio`**. A Retina / 200%-scaled display therefore delivers double
  the delta for the same physical wheel notch.
- `bevy_winit-0.18.0/src/state.rs` (~line 338) then passes those straight through as
  `MouseScrollUnit::Line` / `MouseScrollUnit::Pixel`.

Practical consequence: Chrome/Edge on Windows normally report `DOM_DELTA_PIXEL` (~100 logical px
per notch), so the **web build takes the `Pixel` branch while native Windows takes the `Line`
branch** (winit's Windows backend divides by `WHEEL_DELTA` = 120, so a notch arrives as `y ≈ 1.0`).
Any per-event `Line` clamp is therefore a no-op on the dominant web configuration, and a fixed
pixels-per-line divisor makes web zoom sensitivity vary with browser zoom / display DPI.

**Why:** the "scroll-wheel orbit zoom snaps to min/max radius" bug was fixed natively with a
per-event `Line` clamp plus a fixed `Pixel` divisor; the DPI multiplier and the Chrome
Pixel-vs-Line split mean the web build's behaviour is not covered by that reasoning.

**How to apply:** when reviewing any wheel/zoom/scroll input code, check the `Pixel` branch
specifically — recommend clamping the **summed per-frame** delta (a browser/DPI-independent bound)
rather than relying only on per-event `Line` clamping, and treat a hardcoded pixels-per-line
constant as DPI-sensitive on web. Only two systems read `MouseWheel` in this engine
(`camera_orbit_system`, `party_camera_follow_system`, both in
`crates/ironhold_core/src/capabilities/camera.rs`); flycam does not. Related:
[[project_camera_modes_v1]], [[project_camera_modes_v2]], [[project_local_coop_input_camera]].

Cost note for future diffs on this path: `MessageReader::read()` yields zero items on non-scroll
frames, so any per-event work (match, clamp, divide) is free on idle frames — per-event branching
in these systems is never a per-frame concern.
