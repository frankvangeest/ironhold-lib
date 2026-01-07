# Ironhold-lib Walkthrough

This document guides you through building and running the Ironhold-lib project on Native and Web platforms.

## Prerequisites
- Rust and Cargo
- `wasm-pack` (for web build)
- A generic HTTP server (e.g. `python -m http.server`, `simple-http-server`)

## Project Structure
The project is a Cargo Workspace:
- `crates/ironhold_core`: Shared game logic and asset loading.
- `crates/ironhold_native`: Native executable entry point.
- `crates/ironhold_web`: Web logic entry point.
- `assets/`: Shared assets root folder.

## 1. Native Build (Windows/Linux)

To run the application natively:

```bash
cargo run -p ironhold_native
```

The application automatically checks for assets in both the `assets/` root folder and `../../assets/` relative to the crate execution path.

## 2. Web Build (WASM)

To build for the web:

```bash
# Install wasm-pack if you haven't
cargo install wasm-pack

# Build the web crate targeting the web
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
```

### Running the Web Version
1. Serve the project root directory:
   ```bash
   # Example using python
   python -m http.server
   ```
2. Open `http://localhost:8000` in your browser.
3. The `index.html` will load the WASM and start the Bevy app, loading `assets/scenes/main.ron`.

## Assets
- The application looks for assets in the `assets/` folder at the project root.
- `main.ron` defines the scene layout.
- The default setup expects a `model.glb` at `assets/models/model.glb`. **Note:** You need to add a valid GLB file there.

## Configuration
- Modify `assets/scenes/main.ron` to change the scene composition.
- Modify `assets/scenes/start-menu.ron` to change the start button.
- Create new scenes and link them via buttons in the `ui` list of a `GameLevel`.
- **Configure Player**: Add a `player` block to your scene RON file to define model, camera settings, and inputs.

> [!NOTE]
> The project uses `bevy_common_assets` (git) for Bevy 0.17 compatibility.
