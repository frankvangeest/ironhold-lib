# Feature: Diagnostics HUD

_Status: Ready_
_Planned at: `91cd464` (2026-04-27)_

## What
A toggleable on-screen overlay that shows real-time engine statistics: FPS, frame time,
entity count, draw calls, and triangle count. On native builds it also shows CPU and RAM
usage. Toggled by a configurable key binding (default `F3`), works on both native and
web, and is only compiled in debug / `diagnostics` feature builds.

## Why
Currently there is no in-engine way to see whether a scene is CPU-bound, GPU-bound,
or generating excessive entities/draw calls. The only way to spot a regression is to
notice the frame rate feels wrong. A simple HUD closes this loop without requiring an
external tool for the common case.

## Approach

### Bevy diagnostic plugins
Bevy's `bevy_diagnostic` crate is already available. Add these plugins behind a
`diagnostics` feature flag on `ironhold_core`:

| Plugin | Metric | Web |
|---|---|---|
| `FrameTimeDiagnosticsPlugin` | FPS, frame time (ms), frame count | ✅ |
| `EntityCountDiagnosticsPlugin` | Total ECS entity count | ✅ |
| `RenderDiagnosticsPlugin` | Draw calls, triangle count | ✅ |
| `SystemInformationDiagnosticsPlugin` | CPU %, RAM (MB) | Native only |

`SystemInformationDiagnosticsPlugin` requires the `bevy/sysinfo_diagnostics` Cargo
feature; gate it with `#[cfg(not(target_arch = "wasm32"))]`.

### Feature flag
Add `diagnostics` to `ironhold_core`'s `[features]` (alongside the existing
`inspector`). `ironhold_native` exposes it as `--features diagnostics`.

```toml
# ironhold_core/Cargo.toml
[features]
inspector    = ["dep:bevy_inspector_egui"]
diagnostics  = []
```

The four plugins above are registered inside:
```rust
#[cfg(feature = "diagnostics")]
fn add_diagnostics_plugins(app: &mut App) { ... }
```

### HUD overlay
A new `DiagnosticsHudPlugin` (compiled only with the `diagnostics` feature) spawns an
absolute-positioned UI panel in the top-right corner. It reads `Diagnostics` resource
each frame and updates label text. The panel is hidden by default; the `F3` key binding
(or `ui.toggle_diagnostics` action) toggles its `Visibility`.

Layout (single column, monospace-style labels):
```
FPS       144.2
Frame      6.9 ms
Entities   312
Draw calls  48
Triangles  82k
CPU         12 %    ← native only
RAM        340 MB   ← native only
```

### Key binding
Register `F3 → ui.toggle_diagnostics` in the engine's default global key bindings when
the `diagnostics` feature is active. Projects can remap it in `project.ron` like any
other binding.

### No-op on web / release
When compiled without `--features diagnostics`, zero code is emitted — no
`DiagnosticsHudPlugin`, no registered systems, no UI entities. This keeps the release
and WASM builds clean.

## Tasks
- [ ] Add `diagnostics` feature to `ironhold_core/Cargo.toml`; gate `sysinfo_diagnostics` to native
- [ ] Register the four diagnostic plugins inside `add_diagnostics_plugins`
- [ ] `DiagnosticsHudPlugin`: spawn HUD panel, update labels each frame from `Diagnostics`
- [ ] Register `F3 → ui.toggle_diagnostics` default binding when feature is active
- [ ] Wire `ui.toggle_diagnostics` action to toggle panel `Visibility` in action executor
- [ ] Expose `--features diagnostics` in `ironhold_native`
- [ ] Tests: HUD panel entity exists when feature enabled; absent when disabled
- [ ] Docs: add usage note to `docs/70_profiling.md`

## Open questions
- Should the HUD position (top-right vs. top-left) be configurable, or is a fixed
  corner enough for v1? Fixed is simpler and easier to avoid overlapping with project UI.
- Should `diagnostics` imply `inspector`, or remain independent? Keep them independent —
  they serve different purposes (metrics vs. ECS browsing).

## Acceptance criteria
- Given `cargo run -p ironhold_native --features diagnostics`, pressing F3 shows the
  HUD panel with FPS, frame time, entity count, draw calls, and triangles.
- Given a native build, CPU % and RAM usage also appear.
- Given a WASM build with diagnostics, CPU/RAM rows are absent; all other rows present.
- Given `cargo run -p ironhold_native` (no feature), F3 has no effect and no HUD entity exists.
