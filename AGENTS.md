# Ironhold Library - Agent Onboarding & Project Rules

Welcome! If you are an AI assistant or an agent working on this project, please read this document first. It contains critical context, architectural choices, and hard-learned lessons about this codebase.

## 1. Technology Stack
- **Language**: Rust
- **Game Engine**: Bevy `0.18.0` `(CRITICAL: Always rely on 0.18 API changes, such as AsBindGroup behaviors and resource initialization.)`
- **Targets**: Native (Desktop) and Web/WASM (WebGPU)
- **UI / Debug**: `bevy_egui` for the inspector/editor GUI.
- **Serialization**: `ron` (Rusty Object Notation) for scenes and configurations.

## 2. Project Architecture (Workspace)
This is a Cargo workspace with the following core crates:
- `ironhold_core/`: The main library containing game logic, rendering pipelines, terrain generation, and the `scene_manager`. This code must remain platform-agnostic.
- `ironhold_native/`: The desktop executable runner.
- `ironhold_web/`: The WebAssembly (WASM) runner.

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
