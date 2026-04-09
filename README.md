# Ironhold-lib

Data-driven, cross-platform game runtime built on **Bevy 0.18**. Games are authored in **RON files** — no recompiling required for most content changes.

## Play it live

**Project Gallery:** https://frankvangeest.github.io/ironhold-lib/index.html

---

## What you can build today — without recompiling

Everything below is controlled by `.ron` files and assets. You write data, the runtime does the rest.

### Project & scene setup
- Define a project entry point, initial scene, and global key bindings in `{name}.project.ron`
- Each scene is a `{name}.scene.ron` that spawns entities, configures lighting, and declares UI
- Reference all assets (models, textures, audio, materials) by key from a central `assets.ron` catalog
- Define reusable entity templates (prefabs) in `prefabs/prefabs.ron`

### Entities
- Spawn **GLB models** by asset catalog key, with full transform (position, rotation, scale)
- Spawn **primitive shapes** (Cuboid, Sphere, Cylinder, Capsule3d, Cone, Torus, ConicalFrustum) — no models required
- Build **composite objects** from multiple primitives using child prefabs: trees, houses, fences, lamps
- Add **static physics colliders** to any primitive prefab with `physics: true`
- Define named **spawn points** in the scene for scripted placement

### Players & cameras
- Add a **3rd-person player** by tagging a capsule prefab with `"player"`:
  - Physics-based character controller (WASD + orbit camera)
  - Walk speed, run speed, rotation speed configurable per prefab
  - Animation policy: map semantic clip IDs to your glTF animation names
- Add a **free-flying camera** by tagging any prefab with `"flycam"` (WASD + mouse look, no model needed)

### Terrain
- Heightmap-based terrain from a greyscale PNG
- Splatmap material blending across up to 4 texture layers
- Chunk size and world scale configurable per scene

### Lighting
- Per-scene **ambient light** (color + brightness)
- Per-scene **directional sun** (color, intensity, rotation, shadow toggle)
- **Tonemapping** per scene: `AcesFitted` (default), `Reinhard`, `ReinhardLuminance`, `None`, `SomewhatBoringDisplayTransform`
- Project-level **fallback environment**: procedural gradient sky or HDR cubemap (`.ktx2`)

### UI
- **Buttons** — position, size, label, and action trigger; all data-driven
- **Labels** — static or dynamically updated (e.g. the `flycam_position` label updates every frame)
- **Panels** — centered layouts with background color, padding, gap, and width for menus and overlays

### Game logic — no Rust required

**FSM workflow** (`logic/state_machine.ron`) — recommended for any multi-scene game:
- Declare named states: `menu`, `playing`, `paused`, etc.
- Each state has `entry_actions`, `exit_actions`, and in-state event bindings
- `transitions` list drives state changes (any-state or from a specific state)

**Rules workflow** (`logic/rules.ron`) — simpler projects or a single scene:
- Map events directly to action sequences
- Optional `when:` guard restricts a rule to a named logic state

**Events you can react to:**

| Event | When it fires |
|-------|--------------|
| `ui.button_pressed:<id>` | A UI button is clicked |
| `scene.ready:<name>` | Scene fully spawned |
| `scene.requested:<name>` | Load initiated |
| `scene.loaded:<name>` | RON deserialized, before entity spawn |
| `scene.unloading:<name>` | Before a scene is replaced |
| Any key in `global_key_bindings` | Key pressed (e.g. `"Escape": "toggle_pause"`) |

**Actions you can trigger:**

| Action | Effect |
|--------|--------|
| `LoadScene("scenes/main.scene.ron")` | Replace the current scene |
| `LoadSceneOverlay("scenes/pause.scene.ron")` | Open an overlay (pause menus) |
| `UnloadOverlay` | Close the active overlay |
| `PlayAnimation("dance")` | Play a named animation on the player |
| `PlaySound("click")` | Fire-and-forget audio by catalog key |
| `PlayMusicLoop("bg_music")` | Start a looping background track |
| `StopMusic` | Stop the background track |
| `SetVolume(75)` | Set global volume (0–100) |
| `Spawn { prefab: "barrel", id: "barrel_01" }` | Spawn a prefab at runtime |
| `Despawn("barrel_01")` | Remove a spawned entity |
| `Preload("scenes/next.scene.ron")` | Warm the asset cache in advance |
| `Quit` | Exit the application |
| `Log("message")` | Emit an info log line |

### Materials & shaders
- **Standard PBR** — base color, texture maps, metallic, roughness; embedded in your GLB or overridden per-prefab
- **Custom WGSL shaders** — author a `.wgsl` fragment shader, declare uniforms in `assets.ron`, reference it from a prefab; the engine handles the rest
- **Per-model transform corrections** — `overrides/model_fixes.ron` fixes pivot offsets, axis rotations, or scale mismatches without touching the asset file

---

## Example projects

| Project | What it shows |
|---------|---------------|
| `quick_scene` | Minimal starting point: GLB model, start-menu, single scene load |
| `3rd_person_game_demo` | Animated 3D character, full FSM (menu → playing → paused), orbit camera |
| `terrain_demo` | Heightmap terrain, splatmap texture blending, fly camera |
| `custom_materials` | Custom WGSL shaders on primitive shapes via `assets.ron` |
| `primitive_world` | Entire world from geometric primitives: trees, cottages, fences, a pond, a village |

---

## Quick Start

### Native (Windows / Linux / macOS)

```bash
# Default project (quick_scene)
cargo run -p ironhold_native

# Specific project
cargo run -p ironhold_native -- --project primitive_world

# With Bevy inspector overlay
cargo run -p ironhold_native --all-features -- --project 3rd_person_game_demo
```

### Web / WASM

Prerequisites: `wasm-pack`, `rustup target add wasm32-unknown-unknown`

```bash
# Build
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg

# Serve (no-cache, port 8000)
python serve.py
```

Open `http://localhost:8000` for the gallery, or `http://localhost:8000/play.html?project=primitive_world` to run a specific project.

Or skip the build and **[play live on GitHub Pages](https://frankvangeest.github.io/ironhold-lib/index.html)**.

### Tests

```bash
# All unit + integration tests
cargo test -p ironhold_core

# Full browser test suite (builds WASM, starts server, runs headless Chromium)
python test_web.py

# Re-test without rebuilding WASM
python test_web.py --skip-build
```

---

## Creating a new project

Minimum file structure:

```
assets/projects/{name}/
  {name}.project.ron          ← entry point (schema v2 or v3)
  assets.ron                  ← model / texture / audio / material catalog
  prefabs/prefabs.ron         ← named entity templates
  scenes/{scene}.scene.ron    ← one file per scene
  logic/state_machine.ron     ← FSM game logic  (or logic/rules.ron for simpler projects)
  overrides/model_fixes.ron   ← optional GLB transform corrections
```

Full schema reference: [`docs/20_data_formats.md`](docs/20_data_formats.md)

---

## Documentation

| File | Contents |
|------|----------|
| [`docs/20_data_formats.md`](docs/20_data_formats.md) | Full RON schema reference — the main authoring guide |
| [`docs/25_custom_shaders.md`](docs/25_custom_shaders.md) | Custom WGSL shaders: authoring, bindings, uniform packing |
| [`docs/30_runtime_events_and_logic.md`](docs/30_runtime_events_and_logic.md) | Events, actions, FSM semantics |
| [`docs/10_architecture.md`](docs/10_architecture.md) | Crate structure, runtime pipeline, asset discovery |
| [`docs/00_overview.md`](docs/00_overview.md) | Goals and vision |
| [`docs/40_determinism_and_networking.md`](docs/40_determinism_and_networking.md) | Future: determinism + multiplayer |
| [`docs/50_roadmap_and_milestones.md`](docs/50_roadmap_and_milestones.md) | Milestones and roadmap |
| [`docs/STATUS.md`](docs/STATUS.md) | Implementation status matrix |
| [`docs/browser_tests.md`](docs/browser_tests.md) | Browser test suite |
| [`docs/60_contributing.md`](docs/60_contributing.md) | Contributing guidelines |

---

## Contributing

Contributions welcome — especially around documentation, new example projects, new event/action types, and capability systems. Please keep behaviors data-driven, capabilities modular, and web/native parity in mind.

See [`docs/60_contributing.md`](docs/60_contributing.md).

---

## License

MIT — see `LICENSE-MIT.txt`.
Apache 2.0 — see `LICENSE-APACHE.txt`.
CC0 — see `LICENSE-ASSETS-CC0.txt`.
