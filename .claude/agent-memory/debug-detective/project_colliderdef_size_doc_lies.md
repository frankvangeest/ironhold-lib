---
name: colliderdef-size-doc-lies
description: ColliderDef.size's doc comment says "Half-extents override" but entity_spawner divides by 2 before Collider::cuboid — size is FULL extents; reading the doc alone doubles every collider in your analysis
metadata:
  type: project
---

`ColliderDef.size` in `crates/ironhold_core/src/schema/catalog.rs` is documented as
`/// Half-extents override for Cuboid: (width, height, depth) in world units.` That comment is
wrong. `crates/ironhold_core/src/runtime/scene_manager/entity_spawner.rs` (the
`ColliderShapeKind::Cuboid` arm) does `Collider::cuboid(x / 2.0, y / 2.0, z / 2.0)`, and Rapier's
`Collider::cuboid` takes half-extents — so the authored `size:` is **full extents**.

**Why:** Trusting the doc comment during a review of `feature/item_gated_interactable`
(2026-09-14) turned a 2.2 × 2.4 × 0.3 m door collider into a claimed 4.4 × 4.8 × 0.6 m invisible
wall — a fabricated High-severity finding that only the source read caught. `Sphere`/`Cylinder`
are unaffected (`radius` is a radius; `height` is documented as total height and is likewise
halved for the Rapier cylinder half-height, so that one matches its doc).

**How to apply:** Any time collider dimensions matter (overlap analysis, invisible-wall reports,
"does the collider match the mesh"), read the `entity_spawner.rs` Cuboid arm, never the schema
doc comment. Also worth remembering that `primitive.size` on `kind: Primitive` IS full size
(the `ground_plane` prefab's `size: (100.0, 0.5, 100.0)` is commented as a 100x100 plane), so the
two are consistent with each other and only the doc comment is the outlier.

To get real GLB mesh bounds for a collider-vs-mesh comparison, `ironhold inspect glb` does NOT
print them — parse the glTF JSON chunk instead and read `accessors[primitives.attributes.POSITION].min/max`.
