# Documentation index

**Making a game (no Rust needed)?** Start with [Getting started in 5 minutes](00_overview.md#getting-started-in-5-minutes),
then read [20_data_formats.md](20_data_formats.md) (the RON reference) and, if you use the command-line tool,
[15_authoring_tools.md](15_authoring_tools.md). Dig into [30_runtime_events_and_logic.md](30_runtime_events_and_logic.md)
(see "Project logic: `state_machine.ron`" and "Entity FSM") when you wire up game logic, and into
[25_custom_shaders.md](25_custom_shaders.md) / [05_art_style.md](05_art_style.md) as needed.

**Working on the engine itself?** Everything for contributors is under [`dev/`](dev/60_contributing.md) — workflow,
tests, architecture, determinism, profiling.

| Doc | Audience | Purpose |
|---|---|---|
| [00_overview.md](00_overview.md) | Mixed | "Getting started in 5 minutes" (designer). The goals, repository layout and implementation snapshot are developer material and partly stale. |
| [05_art_style.md](05_art_style.md) | Designer / artist | Visual style, palette, texture and asset guidance |
| [15_authoring_tools.md](15_authoring_tools.md) | Designer (the CLI is optional) | Check, watch and inspect your project with the `ironhold` tool; the canonical list of what `validate` catches |
| [20_data_formats.md](20_data_formats.md) | Designer | The full RON reference: every file, field, action and event |
| [25_custom_shaders.md](25_custom_shaders.md) | Technical artist | Writing WGSL shaders (the alignment and uniform-packing sections are engine-level detail) |
| [30_runtime_events_and_logic.md](30_runtime_events_and_logic.md) | Mixed | Designers: "Project logic: `state_machine.ron`" and "Entity FSM". The rest is design vision, planned features and the execution model. |
| [STATUS.md](STATUS.md) | Mixed | What works today (designer). "Engine ABI" and the milestones are developer material. |
| [dev/10_architecture.md](dev/10_architecture.md) | Developer | Crate layout and runtime pipeline |
| [dev/40_determinism_and_networking.md](dev/40_determinism_and_networking.md) | Developer | Determinism and multiplayer design |
| [dev/50_roadmap_and_milestones.md](dev/50_roadmap_and_milestones.md) | Developer | Milestones and roadmap |
| [dev/60_contributing.md](dev/60_contributing.md) | Developer | Workflow, tests, branching, how to add a validate check |
| [dev/70_profiling.md](dev/70_profiling.md) | Developer (one designer section) | Profiling tools; the "Browser DevTools - GPU timing (web)" section is usable by designers |
| [dev/browser_tests.md](dev/browser_tests.md) | Developer | The headless browser test suite |
