# Feature: Nested Prefabs

_Status: Done_
_Planned at: `a173f8d` (2026-04-30)_

## What

Allows a prefab's `children` list to reference another named prefab by key, in addition to inline
primitive shapes. A `village` prefab could place a `well` at `(5, 0, 0)` and a `house_cottage` at
`(12, 0, 3)` without duplicating their geometry definitions.

## Why

The composite primitive pattern (`children: [...]`) already exists and is in active use — the
`pond` basin has 5 children, the `well` has 3, and the player capsule has cosmetic children.
Referencing named prefabs by key is the natural next step: it eliminates copy-paste when the same
sub-structure appears in multiple contexts and makes scene RON easier to read at a glance.

## Transform composition: multiplicative vs. additive

This is the core design decision.

### Option A — Additive (flat inlining)

When a `Nested` child is resolved, its children's offsets are added to the placement offset and all
leaf meshes land directly under the top-level prefab anchor (no intermediate entity).

**Pros:**
- Simple for trivial cases — all offsets are in the same root-relative space.
- No intermediate entity → flat hierarchy in the inspector.
- No scale inheritance (parent scale does not propagate).

**Cons:**
- **Rotation is wrong in the general case.** Adding Euler angles does not compose. If a nested
  prefab has an internal rotation (e.g., a tilted fence section), its children's offsets must be
  rotated by that angle before being added to the parent offset — additive code that just sums
  offsets will place geometry in the wrong position.
- No anchor entity for the nested prefab → cannot attach physics, behavior, or motion to the
  nested prefab as a unit. Every capability would need to be duplicated per leaf.
- Inspector shows a flat list of meshes with no grouping — hard to debug.

### Option B — Multiplicative (standard Bevy hierarchy) ✓ Recommended

The nested prefab is spawned with its own anchor entity, parented under the calling prefab's anchor.
Bevy computes `GlobalTransform = parent_global * local` automatically at every level.
The child's placement `offset`, `rotation_euler_deg`, and `scale` become the anchor's
`LocalTransform` within the parent.

**Pros:**
- **Rotation composes correctly** — a well placed at 45° in a village has its torus and water disc
  at the right positions relative to the well's rotated axes, because they are Bevy children of
  the well anchor (which itself is a child of the village anchor).
- Intermediate anchor entity is available for physics colliders, `motion:`, `trigger_zone:`,
  `behavior:`, and `interactable:` — all existing capabilities attach to the prefab root entity.
- Clean inspector hierarchy: `village → well → Torus, Cylinder`.
- Implementation is minimal: the recursive spawner loop already does this for single-level
  composites; extending it to recursion is a small change.
- Zero risk to existing RON — `prefab: None` (default) leaves current behaviour unchanged.

**Cons:**
- Scale inheritance can cause **shearing** when a parent has non-uniform scale and the child has a
  rotation. Mitigation: document that non-uniform scale on prefab anchors is unsupported (scale
  should be uniform or left at 1.0).
- Extra entity per nesting level — negligible for the scales involved.

**Decision: Option B (multiplicative).** It is correct in the general case and consistent with how
every 3D scene hierarchy works. The scale-shearing limitation is a known, documentable constraint.

## Approach

### Schema (`schema/catalog.rs`)

Add an optional `prefab` field to `ChildPrimitiveDef`. When `prefab` is `Some`, the child is a
nested prefab reference; `shape` is ignored. When `prefab` is `None`, the child is an inline
primitive (current behaviour). This avoids changing the RON enum tag and keeps existing files valid.

```rust
pub struct ChildPrimitiveDef {
    /// Inline primitive shape. Empty string (default) when `prefab` is set.
    #[serde(default)]
    pub shape: String,
    #[serde(default)]
    pub primitive: PrimitiveParams,
    pub offset: (f32, f32, f32),
    #[serde(default)]
    pub rotation_euler_deg: (f32, f32, f32),
    #[serde(default = "one_vec3_child")]
    pub scale: (f32, f32, f32),
    #[serde(default)]
    pub material: Option<String>,

    /// NEW — reference a prefab by catalog key.
    /// Mutually exclusive with `shape`. When set, `shape`, `primitive`, and `material` are ignored.
    #[serde(default)]
    pub prefab: Option<String>,
}
```

Validate in `PrefabCatalog::validate()`:
- If `child.prefab.is_some()` → `child.shape` must be empty.
- If `child.prefab.is_none()` → `child.shape` must be a known primitive name.
- Referenced prefab key must exist in the catalog (forward-reference check after all defs loaded).

### Spawner (`runtime/scene_manager/scene_loader.rs`)

Extract the current `for child_def in &prefab.children` loop into a helper:

```rust
fn spawn_prefab_children(
    commands: &mut Commands,
    parent: Entity,
    children: &[ChildPrimitiveDef],
    prefab_catalog: &PrefabCatalog,
    /* mesh/mat builders, LevelEntity, etc. */
    depth: u8,
    visiting: &mut HashSet<String>,
)
```

When `child_def.prefab` is `Some(key)`:
1. Guard: if `depth >= 8` → log error and skip.
2. Guard: if `visiting.contains(key)` → log error (cycle) and skip.
3. Insert `key` into `visiting`.
4. Spawn an anchor entity for the nested prefab with the child's `offset`/`rotation_euler_deg`/`scale`
   as `LocalTransform`, parented to `parent`.
5. Recurse: call `spawn_prefab_children` with the nested prefab's own `children`, the new anchor
   as `parent`, `depth + 1`, and `visiting`.
6. Remove `key` from `visiting`.

### RON authoring example

```ron
// prefabs.ron
"village": (
    kind: "primitive",
    model: "",
    components: (),
    children: [
        // Inline primitive — existing syntax, unchanged
        (shape: "Cuboid", primitive: (size: (20.0, 0.1, 20.0)), material: "mat_grass"),
        // Nested prefab reference — new syntax
        (prefab: "well",         offset: ( 5.0, 0.0,  3.0), rotation_euler_deg: (0.0, 45.0, 0.0)),
        (prefab: "house_cottage", offset: (12.0, 0.0, -2.0)),
    ],
),
```

## Tasks

- [x] Schema: add `prefab: Option<String>` to `ChildPrimitiveDef` with `#[serde(default)]`
- [x] Validation: extend `PrefabCatalog::validate()` to check mutual exclusion and referenced keys
- [x] Spawner: extract child-spawning into a recursive helper with `depth` and `visiting` guards
- [x] Spawner: handle `child_def.prefab` branch — spawn anchor, recurse
- [x] RON: add a `village` prefab to `primitive_world/prefabs/prefabs.ron` as a real-world test
- [x] RON: update `main.scene.ron` to use the nested prefab (village at (-22, 0, -10))
- [x] Tests: 5 `ron_validation.rs` tests — parse OK, mutual exclusion, neither set, unknown key, cycle detection
- [x] Docs: update `docs/20_data_formats.md` — `prefab:` field, nested prefab section, scale-shearing caveat
- [x] Docs: update `crates/ironhold_core/src/CLAUDE.md` — `spawn_primitive_children` helper, cycle detection, call sites

## Known limitations

- **Composite `kind: "primitive"` only.** `spawn_primitive_children` recurses into
  `nested_prefab.children` but never touches the nested prefab's top-level `model` field or `kind`.
  Consequence:
  - A nested prefab with `kind: "actor"` or `kind: "prop"` (GLB model) spawns only a bare
    anchor entity — the mesh is silently dropped.
  - A nested prefab with `kind: "primitive"` and a top-level `model` but no `children` (single-shape
    prefab like `"Sphere"`) also spawns only a bare anchor.
  - Only nested prefabs that themselves use a `children` list produce visible geometry.
  - See `planning/features/nested_prefabs_mesh_support.md` for the planned extension.

## Open questions

- Should the nested prefab anchor entity carry a `Name` (e.g., `"well@(5,0,3)"`) or just the
  prefab key as-is? The key alone is clearest in the inspector.
- Do we want to allow `motion:` on a `ChildPrimitiveDef` that references a prefab? That would let
  a nested prefab spin independently. Probably yes — it falls out naturally if the anchor entity
  gets a `Motion` component.
- Should `trigger_zone:` / `interactable:` / `behavior:` also be promotable to nested-prefab
  children, or are those concerns always on the top-level entity?

## Acceptance criteria

- Given a prefab `"village"` with a nested child `(prefab: "well", offset: (5,0,0))`, when
  `primitive_world` loads, then a `well` subtree appears as a child of the `village_01` entity at
  the expected world position.
- Given a cyclic catalog (`a` → `b` → `a`), when loaded, then the engine logs a clear error and
  spawns what it can without panicking.
- Given a prefab with `(prefab: "well", rotation_euler_deg: (0,45,0))`, when viewed in the
  inspector, then the well's Torus and Cylinder children are oriented 45° around Y relative to
  the world, matching expectations for multiplicative composition.
- All existing RON files (no `prefab:` field) load identically to before.
