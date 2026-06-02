# Feature: Live Project Editor

_Status: Draft_
_Planned at: `dbcdf02` (2026-05-31) — revised `44d65f3` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Add `Serialize` derives to all schema structs.** RON ↔ JSON round-trip requires `serde::Serialize` on every schema type that the editor reads or writes. This is shared work with the save/load feature (`SaveState` also needs `Serialize`). Do it in one pass across `schema/catalog.rs`, `schema/scene_v2.rs`, `schema/stats.rs`, `schema/project.rs`, and `schema/actions.rs` before building the editor backend.
>
> - [ ] **Add `schemars` derive behind an `editor` feature flag.** `schemars::JsonSchema` is needed only when building the editor binary. Gate it with `#[cfg_attr(feature = "editor", derive(JsonSchema))]` on every schema struct. The `ironhold_editor` crate enables `ironhold_core/editor`. This keeps `ironhold_core`'s WASM and native builds clean.
>
> - [ ] **Decide: RJSF vs. custom form components.** `react-jsonschema-form` (RJSF) auto-renders a form from any JSON Schema — zero per-field component work for the base case. The output is functional but generic. Custom components can override specific field types (a `(f32, f32, f32)` color field → a color picker; a texture catalog key → an image thumbnail picker). Recommendation: **RJSF as the base, custom `uiSchema` overrides for high-value fields.** Full custom forms are a v2 polish pass.
>
> - [ ] **Decide: how the preview iframe communicates with the editor.** The editor and WASM game run on the same origin (same axum server). On save, the editor can either: (a) reload the iframe (`iframe.src = iframe.src`); (b) send `postMessage({ type: "reload_file", path })` to the iframe and have the WASM runtime handle it. For v1, **full iframe reload** — simple and correct. When the engine hot-reload ships (icebox), switch to `postMessage` so the preview keeps its session state.
>
> - [ ] **Decide: project selection UX.** The editor starts with `cargo run -p ironhold_editor -- --project 3rd_person_game_demo` (a CLI argument). The UI can also list available projects from `assets/projects/` and let the designer switch without restarting. Recommendation: **CLI arg required; in-app switch is a v2 comfort feature.**
>
> - [ ] **Decide: RON serialisation library.** Currently `ironhold_core` uses `ron` for deserialisation only. `ron::ser::to_string_pretty()` handles serialisation with configurable formatting. Verify it produces RON that passes `ironhold_cli validate` on round-trip before building the editor write path. Do a spike first.

---

## What

A local-first web application for non-programmer game designers to edit a project's RON files through structured forms. The designer opens the editor alongside the running game in a split-pane view — left panel shows the file tree and form, right panel shows a live WASM preview of the game.

On save, the editor writes valid RON to disk and reloads the preview. No raw RON editing; no compile step; no terminal knowledge required.

**Primary user**: human game designers who understand game concepts but not Rust or RON syntax.

**Not** a general-purpose RON editor — it understands the ironhold schema specifically and enforces it.

---

## Why

The data-driven design goal ("designers configure games entirely through RON files without recompiling") is only fully realised when designers can actually edit those files without help from a programmer. The CLI validates files after the fact; the editor prevents invalid data from being entered in the first place.

It also lowers the floor for AI-assisted game design — an AI can call the editor's REST API to read and write project data using structured JSON, rather than generating raw RON text that may contain syntax errors.

---

## Architecture

### Crate

```
crates/ironhold_editor/
  Cargo.toml           ← binary crate; depends on ironhold_core/editor + axum
  src/
    main.rs            ← axum server entry point; CLI args (--project, --port)
    api/
      mod.rs
      projects.rs      ← GET /api/projects, GET /api/projects/{name}/files
      files.rs         ← GET + PUT /api/projects/{name}/file?path=...
      schema.rs        ← GET /api/schema/{type_name}
    ron_bridge.rs      ← RON ↔ JSON conversion using ironhold_core schema types
    schema_export.rs   ← schemars JSON Schema generation per schema type
  frontend/
    package.json       ← React + TypeScript + Vite + RJSF
    src/
      App.tsx
      components/
        FileTree.tsx
        FormPanel.tsx       ← RJSF form + uiSchema overrides
        PreviewPane.tsx     ← WASM iframe + reload trigger
        ValidationBadge.tsx
      hooks/
        useProjectFile.ts   ← load/save file via REST + WebSocket reload
        useSchema.ts        ← fetch JSON Schema for current file type
```

### Single-origin server (port 3001)

One `axum` server serves everything on one port — no CORS configuration, no proxy, no cross-origin `postMessage` restrictions.

```
GET  /                            → editor React SPA (frontend/dist/index.html)
GET  /assets/*                    → Vite build assets
GET  /game/*                      → WASM game files (serves pkg/)
                                     with COOP/COEP headers for WebGPU
GET  /api/projects                → list project directories under assets/projects/
GET  /api/projects/{name}/files   → file tree of .ron files in project
GET  /api/projects/{name}/file    → read RON file → parse → JSON
PUT  /api/projects/{name}/file    → JSON → validate → RON → write to disk
GET  /api/schema/{type}           → JSON Schema for a named schema type
WS   /ws                          → WebSocket: server pushes file_changed events
```

### Split-pane layout

```
┌─────────────────────────────────────────────────────────────────┐
│  Ironhold Editor — 3rd_person_game_demo          [● Saved]      │
├──────────────┬──────────────────────────┬───────────────────────┤
│ File Tree    │   Form Panel             │   Preview             │
│              │                          │                       │
│ ▾ scenes/    │  prefabs.ron › orc_enemy │  ┌─────────────────┐ │
│   main.ron   │  ─────────────────────── │  │                 │ │
│ ▾ prefabs/   │  kind     [Actor      ▼] │  │  WASM iframe    │ │
│   prefabs.ron│  model    [creatures/orc]│  │                 │ │
│ assets.ron   │  display  [Orc Warrior ] │  │  (reloads on   │ │
│ groups.ron   │                          │  │   save)         │ │
│              │  ▾ components            │  └─────────────────┘ │
│              │    ▾ npc                 │                       │
│              │      faction [Hostile ▼] │                       │
│              │      detect  [12.0     ] │                       │
│              │                          │                       │
│              │  ▾ stat_templates        │                       │
│              │    [+ Add template]      │                       │
│              │                          │                       │
│              │  ✓ Valid                 │                       │
└──────────────┴──────────────────────────┴───────────────────────┘
```

---

## Schema generation (`schemars`)

```toml
# ironhold_core/Cargo.toml
[features]
editor = ["dep:schemars"]

[dependencies]
schemars = { version = "0.8", optional = true }
```

```rust
// schema/catalog.rs
#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(feature = "editor", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrefabDef { ... }
```

The `schema_export.rs` module in `ironhold_editor` builds a registry of named schemas:

```rust
pub fn schema_for(type_name: &str) -> Option<schemars::schema::RootSchema> {
    match type_name {
        "PrefabCatalog"  => Some(schema_for!(PrefabCatalog)),
        "AssetCatalog"   => Some(schema_for!(AssetCatalog)),
        "GameSceneV2"    => Some(schema_for!(GameSceneV2)),
        "StatCatalog"    => Some(schema_for!(StatCatalog)),
        "GroupCatalog"   => Some(schema_for!(GroupCatalog)),
        "ItemCatalog"    => Some(schema_for!(ItemCatalog)),
        "QuestCatalog"   => Some(schema_for!(QuestCatalog)),
        _ => None,
    }
}
```

`GET /api/schema/PrefabCatalog` returns the JSON Schema for the whole prefab catalog, which RJSF renders as a nested form with one section per prefab entry.

---

## RON ↔ JSON bridge (`ron_bridge.rs`)

```rust
/// Read a .ron file, deserialise it to the matching schema type, return as serde_json::Value.
pub fn ron_to_json(path: &Path, schema_type: &str) -> Result<serde_json::Value, BridgeError> {
    let ron_text = std::fs::read_to_string(path)?;
    match schema_type {
        "PrefabCatalog" => {
            let catalog: PrefabCatalog = ron::from_str(&ron_text)?;
            Ok(serde_json::to_value(catalog)?)
        }
        // ...
    }
}

/// Receive serde_json::Value from the editor, deserialise to the matching type,
/// validate, then serialise to RON and write to disk.
pub fn json_to_ron(path: &Path, schema_type: &str, value: serde_json::Value) -> Result<(), BridgeError> {
    match schema_type {
        "PrefabCatalog" => {
            let catalog: PrefabCatalog = serde_json::from_value(value)?;
            catalog.validate().map_err(BridgeError::Validation)?;
            let ron_text = ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default())?;
            std::fs::write(path, ron_text)?;
        }
        // ...
    }
    Ok(())
}
```

**Validation gates the write.** A `PUT` that produces a validation error returns `400 Bad Request` with the error message — the RON file on disk is never touched.

---

## Preview integration

The axum server serves the WASM build from `pkg/` under `/game/` with the required headers:

```rust
// In the axum router for /game/* routes:
.layer(SetResponseHeaderLayer::overriding(
    header::HeaderName::from_static("cross-origin-opener-policy"),
    HeaderValue::from_static("same-origin"),
))
.layer(SetResponseHeaderLayer::overriding(
    header::HeaderName::from_static("cross-origin-embedder-policy"),
    HeaderValue::from_static("require-corp"),
))
```

The React `PreviewPane` embeds this as an iframe:

```tsx
// PreviewPane.tsx
const iframeRef = useRef<HTMLIFrameElement>(null);

// WebSocket message from server on successful save
useEffect(() => {
    ws.onmessage = (e) => {
        const msg = JSON.parse(e.data);
        if (msg.type === "file_changed") {
            // v1: full reload
            if (iframeRef.current) iframeRef.current.src = `/game/?project=${project}`;
            // v2: postMessage to running WASM for hot-reload without session reset
        }
    };
}, []);

return <iframe ref={iframeRef} src={`/game/?project=${project}`} />;
```

---

## Build and run

```bash
# Build frontend (one-time or after frontend changes)
cd crates/ironhold_editor/frontend && npm install && npm run build

# Run the editor (serves on http://localhost:3001)
cargo run -p ironhold_editor -- --project 3rd_person_game_demo

# Optional: different port
cargo run -p ironhold_editor -- --project primitive_world --port 3001
```

The frontend build output in `frontend/dist/` is served as static files. For Docker, `npm run build` runs in the container image build step; the Rust binary is the only runtime artifact.

---

## v1 scope (edit only)

- Browse all `.ron` files in the selected project
- Click any file → opens its structured form (schema determined by file name convention: `prefabs.ron` → `PrefabCatalog`, `*.scene.ron` → `GameSceneV2`, etc.)
- Edit any field through the form
- Inline validation errors displayed before save
- Save writes RON to disk and reloads the preview iframe
- No create / no delete / no rename (v2)

## v2 scope (create / delete)

- New prefab entry in `prefabs.ron` (typed `+` button, fills required fields with defaults)
- Delete a prefab entry or scene entity
- Create a new `.scene.ron` from a blank template
- Drag-and-drop entity reordering in scene entity list
- Asset picker for model/texture/audio fields (thumbnails from `assets_manifest.json`)

---

## New Rust changes

- `ironhold_core/Cargo.toml` — `schemars` optional dependency behind `editor` feature; `Serialize` added to schema types (shared with save/load work).
- `crates/ironhold_editor/` (new crate) — `Cargo.toml`, `src/main.rs`, `api/`, `ron_bridge.rs`, `schema_export.rs`.
- `Cargo.toml` (workspace) — add `ironhold_editor` to workspace members.
- `crates/ironhold_editor/frontend/` — React + TypeScript Vite project.
- `test_web.py` / `serve.py` — unchanged; editor is a separate server.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `Serialize` derives added to all schema structs in `ironhold_core`
- [ ] `schemars` optional feature + `JsonSchema` derives in `ironhold_core`
- [ ] Verify RON round-trip spike: `ron::ser::to_string_pretty` output passes `ironhold_cli validate`
- [ ] `ironhold_editor` crate scaffolded; added to workspace
- [ ] `axum` server with all REST endpoints + WebSocket
- [ ] `ron_bridge.rs` — RON ↔ JSON for all catalogs + scene types
- [ ] `schema_export.rs` — `schema_for(type_name)` registry
- [ ] Static file serving: frontend under `/`, game under `/game/` with COOP/COEP headers
- [ ] React frontend scaffolded (Vite + TypeScript + RJSF)
- [ ] `FileTree` component — list `.ron` files, click to load
- [ ] `FormPanel` component — RJSF form, `uiSchema` overrides for common field types
- [ ] `PreviewPane` component — iframe embed + WebSocket-triggered reload
- [ ] `ValidationBadge` — display server validation errors inline in the form
- [ ] File type → schema type mapping (by filename convention)
- [ ] Color picker `uiSchema` override for `(f32, f32, f32, f32)` fields
- [ ] Enum dropdown working for `PrefabKind`, `NpcFaction`, `GridKind`, etc.
- [ ] Test: round-trip each example project through the editor; `ironhold_cli validate` passes after every save
- [ ] Docker/container: `Dockerfile` for `ironhold_editor` (Node build + Rust binary)
- [ ] Docs: `README.md` in `crates/ironhold_editor/`; add editor run command to root `CLAUDE.md`

---

## Open questions

- **File type detection by name convention**: `prefabs.ron` → `PrefabCatalog`, `assets.ron` → `AssetCatalog`, `*.scene.ron` → `GameSceneV2`. What about `stats/stats.ron`, `groups/groups.ron`, `quests/quests.ron`, `items/items.ron` — these need path-based disambiguation. The server resolves schema type by path pattern, not just filename. Document the full mapping.
- **`HashMap<String, PrefabDef>` UX**: RJSF renders a `HashMap` as a list of key-value pairs with an add button. The key is the prefab ID (e.g., `"orc_enemy"`). Confirm RJSF's `additionalProperties` support handles this acceptably for v1.
- **Large catalogs**: a `prefabs.ron` with 50 prefabs renders a very long form. Search/filter within the form is a v2 quality-of-life feature.
- **RON serialization fidelity**: `ron::ser` may produce slightly different whitespace/ordering than hand-authored RON. The output must be semantically identical (parses to the same struct) but may not be byte-identical. This is acceptable — document it so designers don't confuse formatting changes with content changes.
- **Preview project selection**: the WASM iframe loads `/?project={name}`. When the designer switches projects in the file tree, the iframe reloads with the new project. `serve.py` is not involved — the editor's axum server serves the WASM.

---

## Acceptance criteria

- Given `cargo run -p ironhold_editor -- --project 3rd_person_game_demo`, the editor opens at `http://localhost:3001`.
- Given clicking `prefabs/prefabs.ron` in the file tree, a structured form displays all prefab entries with correct field types (dropdowns for enums, number inputs for floats, toggles for bools).
- Given entering an invalid value (e.g., leaving `model` empty on an `Actor` prefab), the form shows a validation error and the Save button is disabled.
- Given saving a valid change, the RON file on disk is updated and `ironhold_cli validate` on that project exits 0.
- Given a successful save, the preview iframe reloads and shows the updated scene.
- Given loading any of the 9 example projects, all their `.ron` files open in the editor without errors.
- Given building a Docker image from `crates/ironhold_editor/Dockerfile`, the container starts and the editor is accessible on the configured port.
