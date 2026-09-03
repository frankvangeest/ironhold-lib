# Feature: Typed Primitive Shape Field

_Status: Draft_
_Planned at: `5ac086d` (2026-06-02)_

> **Implementation note:** This feature and [Consistent RON enum casing](consistent_ron_enum_casing.md) both touch `PrefabDef` and require a `PREFAB_CATALOG_SCHEMA_VERSION` bump. Implement and commit them together in a single migration.

---

## What

`kind: "primitive"` prefabs currently abuse the `model:` field to carry the shape name (`model: "Cuboid"`, `model: "Capsule3d"`). This is confusing — `model` means an asset catalog key for `kind: "actor"` / `kind: "prop"`, and something completely different for `kind: "primitive"`.

This feature adds an explicit `shape: PrimitiveShapeKind` field to `PrefabDef` and `ChildPrimitiveDef`, and makes `model` invalid (must be empty string) for primitive prefabs. `ChildPrimitiveDef.shape` is promoted from `String` to the same typed enum.

---

## Why

Two reasons to fix this:
1. **Correctness at authoring time.** A designer reading a primitive prefab cannot tell from `model` alone whether the value is a shape name or a catalog key without reading both the `kind` field and the engine source. After this change, the intent is unambiguous.
2. **Validation coverage.** The current loader silently ignores unknown shape strings (falls back to a default mesh). A typed enum gives RON parse errors on typos before the engine runs.

---

## Current vs. target RON

```ron
// BEFORE
"my_cube": (
    kind: "primitive",
    model: "Cuboid",          // ← model repurposed as shape name; confusing
    primitive: ( size: (2.0, 1.0, 2.0) ),
)

// AFTER
"my_cube": (
    kind: Primitive,          // ← also changed; see consistent_ron_enum_casing.md
    shape: Cuboid,            // ← new explicit typed field
    primitive: ( size: (2.0, 1.0, 2.0) ),
)
```

```ron
// BEFORE — composite child
children: [
    ( shape: "Sphere", primitive: ( radius: 0.5 ) ),
]

// AFTER
children: [
    ( shape: Sphere, primitive: ( radius: 0.5 ) ),
]
```

---

## Schema changes (`schema/catalog.rs`)

### New `PrimitiveShapeKind` enum

```rust
/// Typed shape selector for `kind: Primitive` prefabs and `ChildPrimitiveDef`.
/// Replaces the bare `String` used in `model:` (top-level) and `shape:` (children).
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum PrimitiveShapeKind {
    Cuboid,
    Sphere,
    Cylinder,
    Capsule3d,
    Cone,
    Torus,
    ConicalFrustum,
    Plane,
}
```

### `PrefabDef` changes

```rust
pub struct PrefabDef {
    pub kind: String,   // unchanged here; see consistent_ron_enum_casing.md for the enum migration
    pub model: String,  // still present; must be empty string when kind == "primitive" (validated)

    /// Shape for kind: "primitive" prefabs. Required when kind == "primitive", None otherwise.
    /// Replaces the previous convention of writing the shape name into `model:`.
    #[serde(default)]
    pub shape: Option<PrimitiveShapeKind>,
    // ...
}
```

**Validation** added to `PrefabCatalog::validate()`:
- If `kind == "primitive"` and `shape.is_none()` → error: "primitive prefab requires `shape` field".
- If `kind == "primitive"` and `!model.is_empty()` → error: "`model` must be empty for primitive prefabs; use `shape` instead".
- If `kind != "primitive"` and `shape.is_some()` → warning (not an error; graceful).

### `ChildPrimitiveDef.shape` promoted from `String` to `Option<PrimitiveShapeKind>`

```rust
pub struct ChildPrimitiveDef {
    /// Typed shape for inline primitive children.
    /// Leave None (or omit) when `prefab` is set.
    #[serde(default)]
    pub shape: Option<PrimitiveShapeKind>,
    // ...
}
```

The loader already dispatches on `ChildPrimitiveDef.shape` as a string match. Replace the `match shape_str.as_str()` with `match shape_enum`.

---

## Runtime changes (`scene_loader.rs` / `entity_spawner.rs`)

The `build_primitive_mesh` function currently takes a `shape_name: &str` parameter and matches on it. Replace the parameter type with `&PrimitiveShapeKind`:

```rust
// Before:
fn build_primitive_mesh(shape: &str, params: &PrimitiveParams) -> Mesh { ... }

// After:
fn build_primitive_mesh(shape: &PrimitiveShapeKind, params: &PrimitiveParams) -> Mesh { ... }
```

Call sites: pass `prefab.shape.as_ref().unwrap()` for top-level primitives; `child.shape.as_ref().unwrap()` for children. The `unwrap()` is safe because `validate()` guarantees `shape` is `Some` for all primitive entries before the engine reaches the spawn path.

---

## Schema version bump

Bump `PREFAB_CATALOG_SCHEMA_VERSION` from `1` to `2`.

Update all `prefabs.ron` files across `assets/projects/` in the same commit:
- Replace `model: "Cuboid"` / `"Sphere"` / etc. with `shape: Cuboid` / `Sphere` / etc.
- Remove the old `model: ""` empty string remnants where they appear.
- Update child `shape: "Sphere"` → `shape: Sphere` etc. (no quotes).

---

## Migration guide (for `docs/20_data_formats.md`)

```
## Migrating PrefabCatalog from schema_version 1 to 2

For every `kind: "primitive"` prefab:
  1. Add `shape: <ShapeName>` using the value previously in `model:`.
  2. Delete the `model:` field (or set `model: ""` — the validator will warn but accept it).

For `ChildPrimitiveDef` entries:
  1. Change `shape: "Sphere"` → `shape: Sphere` (remove quotes from all shape names).

Bump `schema_version: 1` → `schema_version: 2` in each `prefabs.ron`.
```

---

## Tasks

- [ ] `PrimitiveShapeKind` enum in `schema/catalog.rs`
- [ ] `shape: Option<PrimitiveShapeKind>` on `PrefabDef`
- [ ] `shape: Option<PrimitiveShapeKind>` on `ChildPrimitiveDef` (replaces `String`)
- [ ] `PrefabCatalog::validate()` — require `shape` for primitives; error if `model` non-empty on primitives
- [ ] `build_primitive_mesh` updated to take `&PrimitiveShapeKind`
- [ ] All `assets/projects/*/prefabs/prefabs.ron` migrated
- [ ] `PREFAB_CATALOG_SCHEMA_VERSION` bumped to 2
- [ ] `cargo test -p ironhold_core` passes with no regression
- [ ] `docs/20_data_formats.md` — migration guide + updated primitive prefab examples
- [ ] `crates/ironhold_core/src/CLAUDE.md` — update primitive field docs

---

## Acceptance criteria

- Given a `kind: "primitive"` prefab with `shape: Cuboid`, the mesh spawns correctly.
- Given a `kind: "primitive"` prefab with `model: "Cuboid"` and no `shape`, `validate()` returns an error.
- Given a `kind: "actor"` prefab, `shape` is ignored.
- Given a `ChildPrimitiveDef` with `shape: Sphere` (bare enum, no quotes), the child mesh spawns correctly.
- Given an unknown shape variant in RON (e.g. `shape: Triangle`), RON fails to deserialise with a clear parse error.
- `cargo run -p ironhold_cli -- validate assets/projects/primitive_world` exits 0.
