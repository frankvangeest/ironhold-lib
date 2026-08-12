---
name: mouse-wheel-input-normalization
description: Verified per-platform winit/Bevy MouseWheel unit facts (Windows Line=±1.0 always, web Pixel=deltaY×devicePixelRatio) and why dt-scaling a discrete scroll impulse is the underlying zoom bug
metadata:
  type: project
---

**What winit 0.30.12 / bevy_winit 0.18 actually report for `MouseWheel`** (read from vendored
source, not inferred — re-verify if the winit version moves):

- **Native Windows**: `WM_MOUSEWHEEL` → `LineDelta(0.0, (wparam>>16) as i16 / WHEEL_DELTA)`
  (`winit/src/platform_impl/windows/event_loop.rs` ~1726-1737). `WHEEL_DELTA` is 120 and one
  physical notch is 120, so **`MouseWheel.y` is exactly ±1.0 per notch on native Windows.** The
  Windows "scroll N lines per notch" control-panel setting is *not* applied by winit — apps are
  expected to apply it themselves. `WM_MOUSEHWHEEL` puts its value in `x`, leaving `y == 0.0`.
  Raw-input (`handle_raw_input`) does the same `/WHEEL_DELTA`. **Consequence: any "large Line value
  on Windows" diagnosis is wrong**, and a `y.clamp(-1.0, 1.0)` on the `Line` branch is a no-op
  natively. High-resolution/precision wheels can send *fractional* (<1.0) values, also unaffected.
- **WASM/web**: `winit/src/platform_impl/web/web_sys/event.rs:153-158` switches on
  `WheelEvent::delta_mode`. `DOM_DELTA_LINE` → `LineDelta` (Firefox historically ~3 per notch).
  `DOM_DELTA_PIXEL` → `PixelDelta(LogicalPosition(-deltaX, -deltaY).to_physical(scale_factor))` —
  i.e. **the pixel value is multiplied by `devicePixelRatio`**. Chrome/Edge desktop send
  `DOM_DELTA_PIXEL` with `deltaY ≈ 100` per wheel notch, so a single click arrives as
  **100-150+ `Pixel` units, varying with the user's display scaling.** This is the practical
  root cause of any "one scroll click snaps the zoom to the extreme" report, because playtests
  here happen in the browser (`serve.py`), not natively.
- Bevy's own `AccumulatedMouseScroll` resource is not a fix for this — it has a single `unit`
  field for the whole frame and its own doc admits the accumulation is incorrect if the unit
  changes mid-frame. A per-event `match e.unit` helper is strictly better.

**The deeper design bug: the scroll wheel is a discrete impulse, but `camera.rs` multiplies it by
`time.delta_secs()`** (`orbit.radius -= zoom_delta * orbit.zoom_speed * time.delta_secs()`, same
shape in the `party.manual_zoom_offset` line). That makes zoom-per-notch frame-rate dependent, and
Bevy's `Time<Virtual>` only caps `max_delta` at **250 ms** (`bevy_time/src/virt.rs:85`) — 15x a
60 fps frame. So even after unit normalization, one notch during a WASM frame hitch can still jump
most of `min_radius..max_radius`. The units-correct model is an impulse
(`radius -= notches * zoom_step`), but that reinterprets the authored `CameraConfig.zoom_speed`
values in every project (~1/60 rescale) — a designer-visible semantic change to an existing schema
field, so it needs its own feature file, not a silent bugfix. Do this **before** shipping the
backlogged designer/player-facing zoom-speed slider, or the slider will feel non-deterministic.

**Two robust, cheap guards that are unit-, DPI- and framerate-agnostic:** divide `Pixel` deltas by
`Window.scale_factor()` (PrimaryWindow query — WASM-safe) to get back to CSS pixels, and clamp the
**per-frame summed** notch total (e.g. ±3.0) rather than only per-event. A per-event-only clamp
does not bound a fast trackpad flick (many `Pixel` events in one frame) at all.

There are exactly **two** `MouseWheel` consumers in the whole workspace, both in
`capabilities/camera.rs` (`camera_orbit_system`, `party_camera_follow_system`) — no UI/inventory
scroll consumer exists yet. Wheel normalization has no owning module; `runtime/input.rs` is the
natural home (it already owns key/gamepad binding resolution) if a third consumer appears.

See [[camera-architecture]] for the surrounding camera-system structure and
[[split-screen-and-shared-mouse]] for why `zoom_delta` is read once *above* the camera loop
(shared-mouse coupling — unrelated to unit normalization, but the same lines of code).
