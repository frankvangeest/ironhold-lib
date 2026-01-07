# Ironhold-lib Project Plan

## Overview
**Ironhold-lib** is a data-driven game development framework built on **Bevy 0.17.3**. It is designed to be write-once, run-everywhere, supporting both Native (Windows/Linux) and Web (WASM) platforms with a unified codebase.

## Architectural Design

The project will use a **Cargo Workspace** to separate the core logic from the platform-specific runners. This ensures the "main" logic remains identical across platforms.

### Workspace Structure
```
ironhold-lib/
├── Cargo.toml              # Workspace definitions
├── assets/                 # Shared game assets
│   ├── scenes/             # RON files (e.g., scene.ron)
│   └── models/             # GLB files (e.g., model.glb)
└── crates/
    ├── ironhold_core/      # (Lib) The primary game engine logic & unified API
    ├── ironhold_native/    # (Bin) Native desktop runner
    └── ironhold_web/       # (Lib/Bin) Web assembly entry point
```

## detailed Component Breakdown

### 1. `ironhold_core` (Library)
This is the heart of the project. It exports the `App` configuration and systems.
- **Dependencies**: `bevy`, `serde`, `ron`, `bevy_common_assets`.
- **Responsibilities**:
    - Define `GameConfig` and `SceneData` structs (deserializable from RON).
    - Implement the `GamePlugin` struct.
    - Expose a public entry point `pub fn start_app()`.
    - Handle Asset Loading (loading `scene.ron` triggers spawning of `model.glb`).

### 2. `ironhold_native` (Binary)
A thin wrapper for desktop platforms.
- **Dependencies**: `ironhold_core`, `bevy` (default features).
- **Main Entry**:
  ```rust
  fn main() {
      // Platform-specific setup (e.g. window icon, resolution overrides) can go here
      ironhold_core::start_app();
  }
  ```

### 3. `ironhold_web` (WASM Library)
A thin wrapper for the web.
- **Dependencies**: `ironhold_core`, `bevy`.
- **Main Entry**:
  ```rust
  use wasm_bindgen::prelude::*;

  #[wasm_bindgen(start)]
  pub fn start() {
      // Web-specific setup (e.g. canvas selector)
      ironhold_core::start_app();
  }
  ```

## Asset Management Strategy
The engine will be data-driven.
- **Formats**:
    - **Models**: `.glb` (glTF Binary).
    - **Scenes/Config**: `.ron` (Rusty Object Notation).
- **Loading Flow**:
    1. App starts.
    2. specific startup system loads `assets/scenes/main.ron`.
    3. `main.ron` contains paths to models and positioning data.
    4. Bevy spawns entities based on the loaded data.

## Javascript Interop
To meet the requirement of JS loading code looking similar to native:
- Use `wasm-bindgen` to expose the start function.
- The `index.html` will simply invoke the WASM init.
- If dynamic asset paths are needed from JS:
    - `ironhold_core::start_app_with_config(config: String)` can be exposed.
    - JS passes the JSON/RON string or path to the init function.

## Implementation Steps

### Phase 1: Workspace Setup
1. Initialize cargo workspace.
2. Create crates: `core`, `native`, `web`.
3. Configure `Cargo.toml` with Bevy `0.17.3`.

### Phase 2: Core Implementation
1. Define `Scene` struct in `core`.
2. Implement RON loading using `bevy_asset_loader` or `bevy_common_assets`.
3. Create a basic 3D setup system (Camera, Light).

### Phase 3: Platform Runners
1. Implement Native `main.rs`.
2. Implement Web `lib.rs` / `main.rs` and `index.html`.
3. Configure `wasm-server-runner` or `trunk` for web testing.

### Phase 4: Verification
1. `cargo run -p ironhold_native` -> Opens window, loads scene.
2. `trunk serve` (or `wasm-pack`) -> Opens browser, loads scene.
