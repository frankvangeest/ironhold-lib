# Profiling and Performance Analysis

> **Doc type:** Developer Guide
>
> **Status legend:**
> - ✅ **Implemented** — available today
> - 🧪 **Prototype / Partial** — partly implemented
> - 🧭 **Planned** — not yet implemented

This document describes how to identify CPU, GPU, memory, and IO bottlenecks in
Ironhold. Each tool addresses a different layer of the stack; choose the right one for
the problem at hand.

---

## At a glance

| Question | Tool | Platform |
|---|---|---|
| Is my frame rate acceptable? | Diagnostics HUD (F3) 🧭 | Native + Web |
| Which Bevy system is slow? | Tracy profiler 🧭 | Native |
| Which draw calls are expensive? | RenderDoc | Native |
| Which shader is the bottleneck? | RenderDoc / NSight / RGP | Native |
| What is the GPU doing per frame? | Browser DevTools Performance | Web |
| How much memory does a texture use? | RenderDoc (resource inspector) | Native |
| Why is scene loading slow? | Log timestamps (`info!`) ✅ | Native + Web |

---

## In-engine diagnostics HUD 🧭

> Planned — see `planning/features/diagnostics_hud.md`

When built with `--features diagnostics`, press **F3** to toggle an on-screen overlay:

```
FPS       144.2
Frame      6.9 ms
Entities   312
Draw calls  48
Triangles  82k
CPU         12 %    (native only)
RAM        340 MB   (native only)
```

Run the native build with the feature:

```bash
cargo run -p ironhold_native --features diagnostics -- --project terrain_demo
```

The HUD is absent from release and WASM builds unless the feature is explicitly enabled.

---

## Tracy — per-system CPU profiling 🧭

> Planned — see `planning/features/tracy_integration.md`

Tracy is a free, real-time frame profiler that shows every Bevy system as a labelled
bar on a timeline. It is the right tool for answering "which system is taking 8 ms?"

**Setup:**
1. Download Tracy from https://github.com/wolfpld/tracy/releases (the standalone profiler app).
2. Launch Tracy and leave it listening on the default port.
3. Run the engine with Tracy support enabled:
   ```bash
   cargo run -p ironhold_native --features trace_tracy -- --project terrain_demo
   ```
4. Tracy connects automatically and begins streaming spans.

**What to look for:**
- `spawn_scene_v2` — scene loading time; spikes indicate slow asset parsing.
- `poll_terrain_generation_system` — terrain mesh task completion; expect a spike on the first frame terrain is ready.
- `npc_behavior_system`, `animation_resolver_system` — scales with entity count.
- `FixedUpdate` frame budget — should stay well under your target tick interval.

Tracy is native-only. Not available in WASM builds.

---

## RenderDoc — GPU frame capture (native)

RenderDoc captures a complete GPU frame and lets you inspect every draw call, texture
binding, shader input/output, and pipeline state. Use it for:

- Identifying expensive draw calls (sort by GPU duration in the Event Browser).
- Inspecting shadow map contents and cascade coverage.
- Verifying texture formats, sizes, and mip counts.
- Debugging unexpected visual output (wrong normals, missing textures, z-fighting).

**Setup:**
1. Download from https://renderdoc.org (free, open source).
2. Launch RenderDoc, then open your native `ironhold_native` executable through it
   (File → Launch Application), passing `--project <name>` as the command line.
3. Press **F12** in the running window to capture a frame.
4. Inspect the capture in the RenderDoc UI.

**Tips for Ironhold:**
- Shadow map passes appear as early render passes before the main forward pass. Check
  the cascade count and map resolution matches what you configured in the scene RON.
- Custom material draw calls show up with your WGSL shader; the pipeline state panel
  shows which bind groups and uniforms are active.
- Sort events by GPU duration (`View → Sort by Duration`) to find the most expensive
  draw calls immediately.

---

## Browser DevTools — GPU timing (web)

For WASM builds, the browser's built-in Performance profiler shows rendering cost per
frame.

**Chrome / Edge:**
1. Open DevTools → **Performance** tab.
2. Click **Record**, interact with the scene for a few seconds, click **Stop**.
3. Look at the **GPU** row in the flame chart for rasterization cost per frame.
4. The **Frames** row shows frame duration and highlights dropped frames in red.

**Useful for:**
- Confirming the web build stays within the 60 fps budget.
- Spotting frames that spike due to asset loading or terrain generation.
- Checking that tab-hidden throttling is working (frames should stop when the tab is
  hidden).

**Not available in browser DevTools:**
- Per-draw-call GPU timing.
- Shader performance breakdown.
- GPU memory usage.

For deep GPU analysis on web, use the experimental `chrome://flags/#enable-webgpu-developer-features` flag which unlocks some additional WebGPU timing query support.

---

## Asset and IO load timing ✅

The scene loader and terrain system emit structured `info!()` log entries at key
milestones. Run with `RUST_LOG=info` to see them:

```bash
RUST_LOG=info cargo run -p ironhold_native -- --project terrain_demo
```

Key log lines to watch:
- `Scene V2 Loaded! name=..., N entities` — scene RON parse complete.
- `Heightmap loaded (...). Starting async generation...` — heightmap decode complete.
- `Terrain Generation Completed.` — mesh built, collider computed, ready to render.
- `Built N material(s) from asset catalog` — material compilation done.

The wall-clock gap between "Starting async generation" and "Terrain Generation
Completed" is the terrain build cost. If this is long (>1s), reduce the heightmap size
(currently 512×512 for all projects) or the chunk size.

---

## Quick reference: what to check first

**Scene loads slowly:**
1. Check the `info!` log timestamps — which phase is the bottleneck?
2. Is it the heightmap? Reduce `--size` in the texture gen tool.
3. Is it GLB loading? Check file size; large GLBs may need LOD or streaming.

**Frame rate is low in a complex scene:**
1. Enable the diagnostics HUD (F3) — is the frame time CPU or GPU bound?
2. If CPU: run with Tracy to find the slow system.
3. If GPU: capture with RenderDoc; check draw call count and shadow map passes.

**Web build is slow:**
1. Use browser DevTools Performance tab to identify frame spikes.
2. Check that `WinitSettings` unfocused throttle is working (tab-hidden = 0 fps).
3. Verify shadow `num_cascades` and `shadow_map_size` are set conservatively in the scene RON.
