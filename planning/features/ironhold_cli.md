# Feature: Ironhold CLI

_Status: Done — Phase 1 (validate), Phase 2 (query prefabs/effects/scenes/rules), Phase 3 (inspect glb/texture/audio), and Phase 4 (CI integration via Rust tests) all shipped_
_Planned at: `b4c988e` (2026-05-30)_

## What

A native CLI tool (`ironhold`) in a new `crates/ironhold_cli` crate that provides:
- **Validation** — parse and validate all RON files in a project, report errors with file/line context
- **Schema introspection** — print field definitions and valid values for any schema type
- **Query** — list and filter prefabs, effects, scenes, and rules in a project

V1 is read-only. Write/patch support is deferred.

## Why

AI agents authoring RON files need programmatic feedback loops: "did I write this correctly?",
"what fields exist on this type?", "what effect keys are already defined?". The existing RON
tests answer these in CI but not interactively. Human developers benefit from the same tool
in their edit-validate loop without spinning up a full native build.

## Primary Users

Both AI agents (Claude Code, MCP tools) and human developers. Design rule:
- **Default output**: human-readable, colored, clear error messages
- **`--json` flag**: machine-readable structured output with deterministic exit codes

---

## Crate Structure

New crate `crates/ironhold_cli` parallel to `ironhold_native` and `ironhold_web`:

```
crates/
  ironhold_cli/
    Cargo.toml          ← binary crate; depends on ironhold_core
    src/
      main.rs
      commands/
        validate.rs
        schema.rs
        query.rs
      output.rs         ← human vs JSON output formatting
      error.rs          ← structured error types
```

`ironhold_core` gains a new public module `validation` (extracted/shared with test suite)
that the CLI calls. No duplication of schema definitions — CLI uses the same `Deserialize`
impls already in `ironhold_core::schema`.

---

## Command Surface

```
ironhold [--json] <COMMAND>

Commands:
  validate   Parse and validate all RON files in a project directory
  schema     Show schema definition for a named type
  query      List or filter data from a project
```

### `ironhold validate <project_dir>`

Parse every RON file in `<project_dir>` using the same structs as `ironhold_core`.
Report per-file parse errors with line numbers, then run cross-file consistency checks.

**Exit codes**: `0` = fully valid, `1` = validation errors, `2` = tool/IO error.

**Cross-file checks (v1)**:
- Effect keys referenced in `rules.ron` / `state_machine.ron` / `*.behavior.ron` exist in `assets.ron`
- Prefab keys referenced in scenes exist in `prefabs/prefabs.ron`
- Scene paths referenced in rules / state machine exist on disk
- Behavior file paths on `PrefabDef` exist on disk
- Stat template keys on entity instances exist in `stats/stats.ron` (when present)

**Human output** (default):
```
Validating: assets/projects/particles_demo/

  assets.ron                      OK
  prefabs/prefabs.ron             OK
  scenes/main.scene.ron           OK
  scenes/particles2.scene.ron     OK
  logic/rules.ron                 OK

  Cross-file checks               OK

5 files checked — all valid.
```

**Human output** (errors):
```
  prefabs/prefabs.ron             ERROR

    line 42: unknown field `partical_count`
              ^^^^^^^^^^^ did you mean `particle_count`?

  Cross-file checks               1 error

    logic/rules.ron:15: effect key "star_burst" not found in assets.ron
```

**JSON output** (`--json`):
```json
{
  "valid": false,
  "project": "particles_demo",
  "files": [
    { "path": "assets.ron", "valid": true, "errors": [] },
    {
      "path": "prefabs/prefabs.ron",
      "valid": false,
      "errors": [
        { "type": "parse_error", "message": "unknown field `partical_count`", "line": 42, "col": 5 }
      ]
    }
  ],
  "cross_file_errors": [
    {
      "type": "missing_reference",
      "ref_type": "effect_key",
      "key": "star_burst",
      "source": "logic/rules.ron",
      "line": 15,
      "message": "effect key 'star_burst' not found in assets.ron"
    }
  ]
}
```

---

### `ironhold schema show <Type>`

Print the field list and valid values for a named schema type.

```
ironhold schema show PrefabDef
ironhold schema show EffectDef
ironhold schema show Action
ironhold schema show GameSceneV2
```

**V1 approach**: hand-written static descriptors in `commands/schema.rs` for the ~12 top-level
designer-facing types. Each descriptor lists field name, type, required/optional, default,
and a one-line description. (Future: derive from `schemars` JSON Schema.)

**Supported types (v1)**:
`PrefabDef`, `EffectDef`, `LayerDef`, `GameSceneV2`, `ProjectConfig`, `AssetCatalog`,
`PrefabCatalog`, `LogicRules`, `StateMachineDef`, `StatDef`, `Action`, `Condition`

**Human output**:
```
PrefabDef — prefabs/prefabs.ron

  Field              Type                  Required   Default    Description
  ─────────────────────────────────────────────────────────────────────────
  key                String                yes        —          Unique identifier for this prefab
  kind               PrefabKind            yes        —          "actor" | "prop" | "primitive" | "composite"
  model              String                no         ""         Asset catalog key for the GLB model
  scale              (f32, f32, f32)        no         (1,1,1)    Uniform or non-uniform scale
  ...
```

**JSON output** (`--json`): array of `{ field, type, required, default, description }` objects.

---

### `ironhold inspect <SUBCOMMAND> <path>`

Inspect individual asset files without needing a full project directory. Replaces the need
to run `tools/glb_inspector/inspect_glb.py` for day-to-day authoring.

```
ironhold inspect glb     <path.glb>
ironhold inspect texture <path.png|jpg|avif|webp>
ironhold inspect audio   <path.ogg|mp3|wav>
```

#### `ironhold inspect glb <path.glb>`

Reports everything a designer needs to author RON for a model.

**Human output**:
```
assets/shared/characters/orc_warrior.glb

  Animations (4)
    idle          2.08 s   loopable
    run           0.96 s   loopable
    attack        1.20 s
    death         1.80 s

  Meshes (3)
    Armature      verts: 2 048   tris: 3 840
    Weapon_R      verts:   312   tris:   580
    Cape          verts:   540   tris:   960

  Materials (2)
    OrcBody
    OrcWeapon

  Root nodes
    Armature
```

**JSON output** (`--json`):
```json
{
  "path": "assets/shared/characters/orc_warrior.glb",
  "animations": [
    { "name": "idle",   "duration_secs": 2.08 },
    { "name": "run",    "duration_secs": 0.96 },
    { "name": "attack", "duration_secs": 1.20 },
    { "name": "death",  "duration_secs": 1.80 }
  ],
  "meshes": [
    { "name": "Armature", "vertex_count": 2048, "triangle_count": 3840 },
    { "name": "Weapon_R", "vertex_count": 312,  "triangle_count": 580  },
    { "name": "Cape",     "vertex_count": 540,  "triangle_count": 960  }
  ],
  "materials": ["OrcBody", "OrcWeapon"],
  "root_nodes": ["Armature"]
}
```

This replaces `tools/glb_inspector/inspect_glb.py` for everyday use. The Python tool remains
for the `--preview` render path which needs Blender.

---

#### `ironhold inspect texture <path>`

Reports image metadata without fully decoding the pixel data.

**Human output**:
```
assets/shared/textures/terrain_grass.png

  Dimensions   1024 × 1024
  Format       PNG
  Channels     RGBA
  File size    412 KB
```

**JSON output**:
```json
{
  "path": "assets/shared/textures/terrain_grass.png",
  "width": 1024,
  "height": 1024,
  "format": "PNG",
  "channels": "RGBA",
  "file_size_bytes": 421888
}
```

Useful for catching oversized textures before they land in WASM builds (no mip generation
at runtime — what you ship is what the GPU uploads).

---

#### `ironhold inspect audio <path>`

Reports audio metadata useful for timing RON events.

**Human output**:
```
assets/shared/audio/explosion_large.ogg

  Format       OGG Vorbis
  Duration     2.34 s
  Sample rate  44 100 Hz
  Channels     Stereo
  File size    86 KB
```

**JSON output**:
```json
{
  "path": "assets/shared/audio/explosion_large.ogg",
  "format": "OGG",
  "duration_secs": 2.34,
  "sample_rate_hz": 44100,
  "channels": 2,
  "file_size_bytes": 88064
}
```

Duration is the key output — designers need it to set correct `delay_secs` values in
`EmitEventAfterDelay` after a sound plays.

---

### `ironhold query <SUBCOMMAND> <project_dir>`

```
ironhold query prefabs <project_dir> [--filter key=value] [--keys-only]
ironhold query effects <project_dir> [--filter key=value] [--keys-only]
ironhold query scenes  <project_dir>
ironhold query rules   <project_dir>
```

Lists items parsed from the project. `--keys-only` prints just the string keys (useful for
piping or AI agent context).

**Examples**:
```bash
ironhold query prefabs assets/projects/particles_demo/ --keys-only
# fire_pit
# torch_wall
# magic_shrine
# ...

ironhold query effects assets/projects/particles_demo/ --filter additive=true
# campfire_fire    layers:2  additive
# explosion_burst  count:20  additive

ironhold query scenes assets/projects/3rd_person_game_demo/
# scenes/main.scene.ron        name:main       player:true
# scenes/pause.scene.ron       name:pause      overlay
# scenes/start_menu.scene.ron  name:start_menu overlay
```

---

## Implementation Plan

### Phase 1 — Scaffold + Validate (ship first)
1. Create `crates/ironhold_cli/` with `Cargo.toml` and `main.rs` using `clap`
2. Extract validation helpers from `crates/ironhold_core/tests/ron_validation.rs` into
   `crates/ironhold_core/src/validation.rs` (public module, `#[cfg(not(target_arch = "wasm32"))]`)
3. Implement `validate` command: per-file parse errors + cross-file checks listed above
4. Human-readable output (no color library needed — ANSI codes directly or `colored` crate)
5. `--json` flag: serialize result to `serde_json::Value`
6. Add `ironhold_cli` to workspace `Cargo.toml`

### Phase 2 — Schema + Query
7. Implement `schema show` with hand-written descriptors for top-12 types
8. Implement `query prefabs`, `query effects`, `query scenes`
9. `--filter` support on query (simple `key=value` string match against serialized fields)

### Phase 3 — Asset Inspection
10. Add `inspect glb` using the `gltf` crate: animations (name + duration), meshes (name + vertex/tri count), materials, root nodes
11. Add `inspect texture` using `image` crate header-only read: dimensions, format, channels, file size
12. Add `inspect audio` using `symphonia` crate: format, duration, sample rate, channel count, file size
13. Wire all three into `--json` output

### Phase 4 — Polish
14. Add to CI (validate all example projects as a smoke check)
15. Document in `docs/60_contributing.md` and `CLAUDE.md`

---

## Dependencies (new)

| Crate | Reason |
|---|---|
| `clap` (4.x, `derive` feature) | Argument parsing |
| `serde_json` | `--json` output serialization |
| `colored` (optional) | ANSI color in human-readable output |
| `gltf` | GLB/GLTF parsing for `inspect glb` — animations, meshes, materials, nodes |
| `image` | Header-only texture metadata read for `inspect texture` |
| `symphonia` | Audio metadata (duration, sample rate) for `inspect audio` |

`ron`, `serde` already in `ironhold_core` — no new additions there.

All three asset-inspection crates are native-only (`ironhold_cli` does not target WASM).

---

## Open Questions

- [ ] Should `ironhold validate` also subsume `tools/asset_checker/check.py` (check that
      asset file paths in `assets.ron` resolve on disk)? Or keep them separate?
- [ ] Should the `validation` module in `ironhold_core` replace the current test helpers
      in `tests/ron_validation.rs`, or live alongside them?
- [ ] For `schema show`, is hand-written descriptors acceptable long-term, or should we
      invest in `schemars` derive now to keep them in sync automatically?
- [ ] Should `ironhold validate` have a `--strict` mode that also flags style issues
      (e.g., effects with no `priority` field that will default to `Npc`)?

---

## Acceptance Criteria

- `ironhold validate assets/projects/particles_demo/` exits `0`
- `ironhold validate <dir_with_typo>` exits `1` and prints file + line of each error
- `ironhold validate --json <dir>` outputs valid JSON matching the schema above
- `ironhold schema show PrefabDef` prints all fields with types and required/optional
- `ironhold query prefabs <dir> --keys-only` prints one prefab key per line
- `ironhold query effects <dir>` lists all effect keys with key metadata
- Cross-file check catches a missing effect key referenced in `rules.ron`
- Cross-file check catches a missing prefab key referenced in a scene
- All existing example projects pass `ironhold validate` (enforced in CI)
- `ironhold inspect glb <path>` lists animation clip names, durations, mesh names, and materials
- `ironhold inspect glb --json <path>` outputs valid JSON with the same data
- `ironhold inspect texture <path>` reports dimensions, format, channels, and file size
- `ironhold inspect audio <path>` reports format, duration, sample rate, and channel count
- Compiles on Windows and Linux (no WASM target — CLI is native-only)
- Shares `Deserialize` impls from `ironhold_core::schema` — no duplicate struct definitions
