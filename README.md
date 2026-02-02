# Ironhold-lib (Bevy 0.18) — Data‑Driven, Cross‑Platform Game Runtime (Native + Web/WASM)

**Ironhold-lib** is a **cross-platform** (Windows/Linux + WebAssembly) game runtime built on **Bevy 0.18**.  
Games are defined by **data files** (`.ron`) and assets (models, textures, audio). Game creators can build new projects and scenes **without recompiling** the engine.

> **Core idea:** the engine ships “capability building blocks” (controller, camera, animation, UI, etc.) and the **project + scene data** decides what gets used.

---

## ✨ What you can do today

- Load a **project** from `assets/project.ron`
- Load a **scene** from `assets/scenes/*.ron`
- Spawn **models** from `.glb`
- Optional **player** with:
  - configurable input mapping (WASD etc.)
  - orbit camera
  - configurable animation mapping
- UI **Button** that triggers `LoadScene(...)`

---

## Repository Layout

```
ironhold-lib/
├─ assets/
│  ├─ project.ron
│  ├─ scenes/
│  │  ├─ start-menu.ron
│  │  └─ main.ron
│  └─ models/
│     ├─ character-01.glb
│     ├─ anvil.glb
│     └─ treasure-chest-*.glb
├─ crates/
│  ├─ ironhold_core/    # shared engine runtime + data schemas
│  ├─ ironhold_native/  # desktop runner
│  └─ ironhold_web/     # wasm runner
└─ index.html           # simple web bootstrap
```

---

## 🚀 Quick Start

### 1) Native (Windows / Linux / macOS)

```bash
cargo run -p ironhold_native
```

or with inspector enabled

```bash
cargo run -p ironhold_native --all-features
```


This runs the Bevy app using the shared runtime in `ironhold_core` and loads:
- `assets/project.ron`
- the configured initial scene (e.g. `assets/scenes/start-menu.ron`)

#### Custom Project Config
You can specify a custom project file as a command-line argument:

```bash
cargo run -p ironhold_native -- project_02.ron
```

> `project_02.ron` should be in the `assets` directory.

---

### 2) Web / WASM

#### Prerequisites
- Rust toolchain installed
- `wasm-pack` installed
- WASM target installed

```bash
rustup target add wasm32-unknown-unknown
```

#### Build
```bash
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
```

#### Serve (any static server)
From the repo root:

```bash
python -m http.server 8000
```

Open:
- `http://localhost:8000`

> `index.html` loads the generated WASM package and starts the engine.

---

## 🎮 Creating a Game (Data‑Driven)

### Project file: `assets/project.ron`

Minimal project config selects the initial scene and defines global rules:

```ron
(
  schema_version: 1,
  initial_scene: "scenes/start-menu.ron",
  // Optional: Global per-asset corrections
  model_fixes: {
    "models/character-01.glb#Scene0": (
      pivot_offset: (0.0, -0.9, 0.0),
      rotation_deg: (0.0, 180.0, 0.0),
      scale: (0.1, 0.1, 0.1),
    ),
  },
  // Optional: Map events (e.g. UI triggers) to engine actions
  rules: [
    (
      on: "ui.button_pressed:start_game",
      do_actions: [ Log("Starting Game"), LoadScene("scenes/main.ron") ],
    ),
    (
      on: "ui.button_pressed:quit",
      do_actions: [ Quit ],
    ),
  ],
)
```

### Scene files: `assets/scenes/*.ron`

A scene defines:
- `models`: list of `.glb` models to spawn
- `ui`: UI elements (e.g. buttons)
- `player`: optional player config (model + camera + inputs + animation policy)

Example:

```ron
(
  schema_version: 1,
  models: [
    (path: "models/anvil.glb#Scene0", position: (2.0, 0.0, 0.0)),
  ],
  ui: [
    Button(
      text: "Start Game", 
      action: Trigger("start_game"),
      position: Some((100.0, 100.0)), // Optional (absolute px)
      width: Some(200.0),             // Optional (px)
      height: Some(80.0),             // Optional (px)
      font_size: Some(40.0),          // Optional
      background_color: Some((0.2, 0.2, 0.2, 0.8)), // Optional RGBA
      text_color: Some((1.0, 1.0, 1.0, 1.0)),       // Optional RGBA
    ),
  ],
  player: Some((
    model_path: "models/character-01.glb#Scene0",
    initial_position: (0.0, 0.0, 2.0),
    inputs: (
      forward: "KeyW",
      backward: "KeyS",
      left: "KeyA",
      right: "KeyD",
      strafe_left: "KeyQ",
      strafe_right: "KeyE",
      jump: "Space",
      run: "ShiftLeft", // Optional, defaults to "ShiftLeft"
    ),
    camera: (
      offset: (0.0, 2.0, 5.0),
      look_at_offset: (0.0, 1.0, 0.0),
      orbit_speed: 0.01,
      zoom_speed: 0.2,
      min_radius: 2.0,
      max_radius: 8.0,
    ),
    animation_policy: (
      base: (
        idle: "Idle",
        walk: "Walk",
        run: "Run",
      ),
      // Optional: semantic aliases
      clips: {
        "dance": "Dance_Loop",
      },
      // Optional: one-shot or looping overrides
      overrides: [
        (id: "dance", clip: "Dance_Loop", looping: true, cancel_on_move: true),
      ],
      default_transition_ms: Some(250),
    ),
  )),
)
```

---

## 🧪 Testing

We have integration tests to verify the UI flow and RON validation tests for configuration files.

### Running all tests
```bash
cargo test -p ironhold_core
```

> [!TIP]
> To see clean execution logs for the interaction tests, run with a single thread:
> `cargo test -p ironhold_core -- --test-threads=1`

### Running Integration Tests
```bash
cargo test -p ironhold_core --test integration_tests
```

### Running RON Validation Tests
```bash
cargo test -p ironhold_core --test ron_validation
```

---

## 🧠 Architecture Direction (Why + Where We’re Going)

Ironhold-lib is moving toward a stable runtime model that supports:
- global logic (menus, flow, quests)
- per-entity logic (interactables, NPC behaviors)
- future multiplayer (server-authoritative and/or rollback)

### Target runtime structure
**Messages (events) → Interpreter (data logic) → Actions → Executors**

- Capability systems emit messages (input, UI, triggers…)
- Data-defined logic interprets messages and outputs actions
- Capability executors apply actions (move, play animation, load scene…)

This keeps the engine generic and lets creators define behavior purely in `.ron`.

---

## 📚 Documentation

We are adding a docs folder to make architecture + decisions part of the repo:

- `docs/00_overview.md` — overview
- `docs/10_architecture.md` — architecture + reasoning
- `docs/20_data_formats.md` — project/scene schema guidance
- `docs/30_runtime_events_and_logic.md` — messages/actions + FSM plan
- `docs/40_determinism_and_networking.md` — determinism + multiplayer strategy
- `docs/50_roadmap_and_milestones.md` — beta milestones + implementation order
- `docs/60_contributing.md` — contributing guidelines

(See `plan.md` for current notes; these docs will become the canonical plan.)

---

## 🧭 Roadmap & Beta Milestones (Stable Foundations First)

We’re planning stable beta milestones that “freeze the foundations” before adding lots of new features:

- **Beta 0.1 — Baseline Runtime**  
  Current functionality stabilized + documented (native + web parity).
- **Beta 0.2 — Event/Action Bus**  
  Decouple systems via messages + actions (refactor, no behavior change).
- **Beta 0.3 — Global Logic (FSM v1)**  
  Project-level state machine in data (menus/flow).
- **Beta 0.4 — Entity Logic (FSM v1)**  
  Per-entity behaviors in data (triggers/interactions).
- **Beta 0.5 — Deterministic Tick + Replay**  
  Fixed tick core, deterministic RNG, input capture/replay.
- **Beta 0.6 — Networking Prototype**  
  Minimal multiplayer proving the architecture.

Full details live in `docs/50_roadmap_and_milestones.md`.

---

## 🛠 Development Notes

### Assets folder discovery
The runtime expects an `assets/` directory at or near the executable working directory.
The engine includes logic to locate the assets folder (including walking up parent folders),
so running from workspace root is usually fine.

### Data validation (planned)
We will add stricter schema validation (and schema versioning) so that
invalid scene/project configs produce actionable errors.

### Logging
Always use `bevy::log::info!`, `warn!`, or `error!` macros. Do not use `println!`.
This ensures logs are captured by the engine and displayed correctly in the browser console (WASM).

---

## 🤝 Contributing

Contributions are welcome — especially around:
- documentation improvements
- data schema validation & tooling
- event/action interpreter foundations
- deterministic tick foundations
- examples and test scenes

Please see `docs/60_contributing.md` (coming) and keep:
- behaviors data-driven
- capabilities modular
- cross-platform parity in mind

---

## License

MIT — see `LICENSE`.
