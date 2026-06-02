# Feature: Consistent RON Enum Casing

_Status: Draft_
_Planned at: `992f0f1` (2026-06-02)_

> **Implementation note:** This feature and [Typed primitive shape field](typed_primitive_shape_field.md) both touch `PrefabDef` and require a `PREFAB_CATALOG_SCHEMA_VERSION` bump. Implement and commit them together in a single migration.

---

## What

Two fields in `PrefabDef` use quoted magic strings where typed enums should be:

| Field | Current (string) | Target (enum) |
|---|---|---|
| `PrefabDef.kind` | `"actor"` / `"prop"` / `"primitive"` | `Actor` / `Prop` / `Primitive` |
| `ColliderDef.shape` | `"Cuboid"` / `"Sphere"` / `"Cylinder"` | `Cuboid` / `Sphere` / `Cylinder` |

All other categorical fields in the schema already use bare RON enum variants (`NpcFaction: Hostile`, `NpcOnPlayerNear: Chase`, `WorldStatBarStyle: Ascii`, etc.). These two are the remaining inconsistencies.

---

## Why

1. **Typos produce silent failures.** `kind: "Actor"` (capital A) is silently rejected by the validator's string comparison, giving a cryptic error. `kind: Actor` fails at the RON parser level with the exact field name — earlier and clearer.
2. **Discoverability.** `cargo run -p ironhold_cli -- validate` and IDE tooling can enumerate valid enum variants. They cannot enumerate valid string values.
3. **Consistency.** Every other categorical field in the schema is already a typed enum. These two are outliers that create a mental split ("which fields need quotes?").

---

## Current vs. target RON

```ron
// BEFORE
"oak_tree": (
    kind: "prop",
    model: "vegetation/oak_tree",
    colliders: [
        ( shape: "Cuboid", size: (0.4, 3.0, 0.4) ),
    ],
)

// AFTER
"oak_tree": (
    kind: Prop,
    model: "vegetation/oak_tree",
    colliders: [
        ( shape: Cuboid, size: (0.4, 3.0, 0.4) ),
    ],
)
```

---

## Schema changes (`schema/catalog.rs`)

### New `PrefabKind` enum

```rust
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum PrefabKind {
    Actor,
    Prop,
    Primitive,
}
```

### New `ColliderShapeKind` enum

```rust
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum ColliderShapeKind {
    Cuboid,
    Sphere,
    Cylinder,
}
```

### `PrefabDef.kind` change

```rust
// Before:
pub kind: String,   // "actor", "prop", or "primitive"

// After:
pub kind: PrefabKind,
```

All code that matches on `kind` as a string is updated to match on `PrefabKind`:

```rust
// Before:
match prefab.kind.as_str() {
    "actor" | "prop" => { ... }
    "primitive" => { ... }
    other => { error!("unknown kind: {}", other); }
}

// After:
match prefab.kind {
    PrefabKind::Actor | PrefabKind::Prop => { ... }
    PrefabKind::Primitive => { ... }
    // No other arm needed — exhaustive enum
}
```

The `validate()` string match in `PrefabCatalog::validate()` is replaced by enum exhaustiveness — the `other =>` branch that currently returns an error is removed because unknown values now fail at deserialisation.

### `ColliderDef.shape` change

```rust
// Before:
pub shape: String,  // "Cuboid", "Sphere", or "Cylinder"

// After:
pub shape: ColliderShapeKind,
```

The executor arm that builds Rapier colliders matches on `ColliderShapeKind` instead of `shape.as_str()`.

---

## Files to update

### Rust source (`crates/ironhold_core/src/`)

- `schema/catalog.rs` — new enums; field type changes; remove string match validation for `kind`.
- `runtime/scene_manager/scene_loader.rs` — all `prefab.kind.as_str()` / `kind == "actor"` patterns → `PrefabKind::Actor` etc.
- `runtime/scene_manager/entity_spawner.rs` — same.
- `runtime/scene_manager/action_executor.rs` — collider builder: `shape.as_str()` → `ColliderShapeKind`.

A project-wide `grep -r '"actor"\|"prop"\|"primitive"'` in `src/` confirms all call sites are updated.

### RON assets (`assets/projects/`)

Every `prefabs.ron` file — update `kind:` and `colliders.shape:` fields. The full list:
- `assets/projects/quick_scene/prefabs/prefabs.ron`
- `assets/projects/3rd_person_game_demo/prefabs/prefabs.ron`
- `assets/projects/terrain_demo/prefabs/prefabs.ron`
- `assets/projects/particles_demo/prefabs/prefabs.ron`
- `assets/projects/entity_logic_demo/prefabs/prefabs.ron`
- `assets/projects/primitive_world/prefabs/prefabs.ron`
- `assets/projects/custom_materials/prefabs/prefabs.ron`
- `assets/projects/effect_mayhem_demo/prefabs/prefabs.ron`
- `assets/projects/integration_tests/prefabs/prefabs.ron`

A one-liner migration for each file:
```powershell
# kind field
(Get-Content prefabs.ron) -replace 'kind: "actor"','kind: Actor' `
                          -replace 'kind: "prop"','kind: Prop' `
                          -replace 'kind: "primitive"','kind: Primitive' |
  Set-Content prefabs.ron

# collider shape field
(Get-Content prefabs.ron) -replace 'shape: "Cuboid"','shape: Cuboid' `
                          -replace 'shape: "Sphere"','shape: Sphere' `
                          -replace 'shape: "Cylinder"','shape: Cylinder' |
  Set-Content prefabs.ron
```

Or write `tools/migrate_enum_casing.py` (parallel to the existing `tools/migrate_implicit_some.py` migration script).

---

## Schema version bump

Bump `PREFAB_CATALOG_SCHEMA_VERSION` from `1` to `2` (shared with the typed primitive shape migration — one bump covers both changes).

---

## Migration guide (for `docs/20_data_formats.md`)

```
## Migrating PrefabCatalog from schema_version 1 to 2

In every prefabs.ron:

1. Replace quoted kind strings with bare enum variants:
   kind: "actor"     → kind: Actor
   kind: "prop"      → kind: Prop
   kind: "primitive" → kind: Primitive

2. Replace quoted collider shape strings:
   shape: "Cuboid"   → shape: Cuboid
   shape: "Sphere"   → shape: Sphere
   shape: "Cylinder" → shape: Cylinder

3. Bump schema_version: 1 → schema_version: 2
```

---

## Tasks

- [ ] `PrefabKind` enum in `schema/catalog.rs`; `PrefabDef.kind` changed to `PrefabKind`
- [ ] `ColliderShapeKind` enum in `schema/catalog.rs`; `ColliderDef.shape` changed to `ColliderShapeKind`
- [ ] All Rust match arms updated (scene_loader, entity_spawner, action_executor)
- [ ] `PREFAB_CATALOG_SCHEMA_VERSION` bumped to 2 (shared with typed shape migration)
- [ ] All `assets/projects/*/prefabs/prefabs.ron` files migrated (9 files)
- [ ] Migration script `tools/migrate_enum_casing.py` (optional; useful if more projects accumulate)
- [ ] `cargo test -p ironhold_core` passes with no regression
- [ ] `cargo run -p ironhold_cli -- validate` passes on all example projects
- [ ] `docs/20_data_formats.md` — migration guide; update all prefab examples
- [ ] `crates/ironhold_core/src/CLAUDE.md` — update field syntax examples

---

## Acceptance criteria

- Given `kind: Actor` in a prefab, the entity spawns as a GLB actor with no warnings.
- Given `kind: "actor"` (old quoted form, schema_version 2 file), RON deserialization fails with a clear parse error.
- Given `colliders: [ ( shape: Cuboid, ... ) ]`, the Rapier collider is built correctly.
- Given `shape: "Cuboid"` (old quoted form), RON deserialization fails.
- `cargo run -p ironhold_cli -- validate assets/projects/primitive_world` exits 0 after migration.
- No unknown-variant fallback branch exists in Rust code — exhaustive enum match confirmed by `cargo check`.
