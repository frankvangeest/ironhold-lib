# Ironhold Library - Agent Onboarding & Project Rules

Welcome! If you are an AI assistant or an agent working on this project, please read this document first. It contains critical context, architectural choices, and hard-learned lessons about this codebase.

> **CRITICAL**: For in-depth architectural details, planning workflows, Python tooling, and instructions on adding new projects, you **must** also read the `CLAUDE.md` files (starting with the one in the root).

## 1. Technology Stack
- **Language**: Rust
- **Game Engine**: Bevy `0.18.0` `(CRITICAL: Always rely on 0.18 API changes, such as AsBindGroup behaviors and resource initialization.)`
- **Targets**: Native (Desktop) and Web/WASM (WebGPU)
- **UI / Debug**: `bevy_egui` for the inspector/editor GUI.
- **Serialization**: `ron` (Rusty Object Notation) for scenes and configurations.

## 2. Project Architecture & Patterns
This is a Cargo workspace with the following core crates:
- `ironhold_core/`: The main library containing game logic, rendering pipelines, terrain generation, and the `scene_manager`. This code must remain platform-agnostic.
- `ironhold_native/`: The desktop executable runner.
- `ironhold_web/`: The WebAssembly (WASM) runner.

### Web Architecture (Multi-Page)
- **`index.html`**: The project gallery and dark-themed selection dashboard.
- **`play.html`**: The dedicated game runner (accepts `?project=<name>` as a parameter).
Ensure you do not break this routing when modifying the web build.

### Data-Driven Game Loop
Game behavior is authored in RON files (`logic/rules.ron`, `logic/state_machine.ron`). Do not hardcode logic in Rust if it belongs in the data schema. The engine uses a strict pipeline: **Message → Interpreter → Action → Executor**.

## 3. Critical Coding Rules & Quirks

### Graphics & WebGPU Alignment
- **WebGPU 16-Byte Alignment**: When creating custom shader materials or structs that are bound to the GPU (e.g., `TerrainMaterial`), you **must** adhere to WebGPU's strict 16-byte alignment rules for uniform buffers. Even if using smaller types like `Vec4`, ensure padding is correctly handled to avoid `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` validation panics.
- **Shaders in WebBuilds**: When updating shaders, verify that `AsBindGroup` mappings correctly differentiate between Uniform and Storage buffers based on Bevy 0.18's expected layout.

### Gameplay Physics & Movement
- **Use `FixedUpdate`**: All player movement, physics processing, and camera-following logic that relies on physics bodies must be scheduled in `FixedUpdate`. Do not use `Update` for physics movement, as it causes stuttering.

### Asynchronous Operations
- **Terrain Generation**: Terrain mesh generation involves heavy computations. Do not block the main thread. Always defer heavy generation logic to background tasks using Bevy's `AsyncComputeTaskPool` and poll the `Task` components on entities.

### UI & Render Layers
- **Inspector Isolation**: The `bevy_egui` inspector and game UI must be strictly separated. The inspector should be rendered by its own camera and layer on top of the 3D scene without bleeding into the main game UI.

### Integration Tests
- **Missing Plugins**: When writing or updating integration tests inside `ironhold_core`, always ensure that the test environment sets up the required resources. Specifically:
  - You must include the `PhysicsPlugin` to prevent tests from panicking due to missing physics resources.
  - If the test involves messaging, ensure the custom `Message` framework (Writer/Reader resources) is correctly initialized before execution.

## 4. Workflows & Commands
If executing commands, use these general patterns:
- **Test Native Build**: `cargo run -p ironhold_native`
- **Run Tests**: `cargo test -p ironhold_core --test '*' -- --nocapture`
- **Test Web Build**: Web builds generally target `wasm32-unknown-unknown`. When debugging the web build, ensure alignment and WGPU validation fixes are verified by compiling against it.

---

*(Note to Agents: Reference these rules immediately when asked to debug a rendering validation error, fix player stuttering, or write passing tests!)*
