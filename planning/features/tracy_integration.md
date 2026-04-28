# Feature: Tracy CPU Profiling Integration

_Status: Ready_
_Planned at: `91cd464` (2026-04-27)_

## What
Enable Bevy's built-in Tracy profiler support on native builds via an optional Cargo
feature flag (`trace_tracy`). When active, every Bevy system is automatically
instrumented and its per-frame CPU timing is streamed to the Tracy profiler application
in real time. Zero extra code beyond the feature flag and one Cargo.toml line.

Tracy is a free, open-source frame profiler: https://github.com/wolfpld/tracy

## Why
The diagnostics HUD shows aggregate FPS and frame time but cannot answer "which
system is taking 8 ms this frame?" Tracy answers that question precisely, showing each
Bevy system as a coloured bar on a timeline with sub-millisecond resolution. It is the
right tool for diagnosing CPU bottlenecks: slow terrain generation, expensive physics
ticks, animation resolver cost, etc.

Bevy's Tracy integration is mature and requires no in-engine code beyond enabling a
feature flag — shipping it is almost entirely a documentation and process task.

## Approach

### Cargo feature
Add a `trace_tracy` feature to `ironhold_native/Cargo.toml` that forwards to Bevy's
own Tracy feature:

```toml
# ironhold_native/Cargo.toml
[features]
trace_tracy = ["ironhold_core/trace_tracy"]

# ironhold_core/Cargo.toml
[features]
trace_tracy = ["bevy/trace_tracy"]
```

No code changes anywhere else — Bevy's internal span instrumentation activates
automatically when `bevy/trace_tracy` is compiled in.

### Workflow
1. Download and launch Tracy (the standalone profiler app).
2. Run the engine with the feature enabled:
   ```bash
   cargo run -p ironhold_native --features trace_tracy -- --project terrain_demo
   ```
3. Tracy connects automatically on localhost and begins streaming spans.

### What you see in Tracy
- Every Bevy system shown as a labelled span (e.g. `spawn_scene_v2`,
  `terrain_generation_task_poll`, `npc_behavior_system`).
- The full `FixedUpdate` / `Update` / `PostUpdate` schedule breakdown per frame.
- Spikes are instantly visible as tall bars on the timeline.
- Click any span to see its exact duration, call depth, and source location.

### Constraints
- **Native only** — Tracy is a native profiler; no WASM support.
- **Debug or `--profile dev`** — release builds can be profiled but symbols may be
  stripped; `opt-level = 1` in a custom profile gives a good balance.
- **Not for production builds** — `trace_tracy` should never be in a shipping binary;
  it opens a network port and adds per-frame overhead (~0.1–0.5 ms).

## Tasks
- [ ] Add `trace_tracy` feature to `ironhold_native/Cargo.toml` and `ironhold_core/Cargo.toml`
- [ ] Verify spans appear in Tracy for key systems: terrain, physics, animation, scene loader
- [ ] Document the workflow in `docs/70_profiling.md` with step-by-step instructions
- [ ] Add a note to `CLAUDE.md` so Claude knows to suggest `--features trace_tracy` for CPU perf questions

## Open questions
- Should a `profile_native` Cargo profile (opt-level=1, debug=true) be added to
  `Cargo.toml` to make profiling more ergonomic? Likely yes — avoids the choice between
  slow debug builds and unreadable release symbols.

## Acceptance criteria
- Given `cargo run -p ironhold_native --features trace_tracy`, Tracy receives spans and
  shows per-system timing on its timeline.
- Given no `--features trace_tracy`, the binary has no Tracy dependency and opens no
  network ports.
- Given the profiling doc, a developer unfamiliar with Tracy can set it up and see their
  first flame chart in under 10 minutes.
