# Feature: Nested Prefabs — Mesh Support

_Status: Done_
_Planned at: `a173f8d` (2026-05-01)_

## What

Extends nested prefab references to work with all prefab kinds, not just composite
`kind: "primitive"` prefabs. After this feature, a `village` prefab can nest a `kind: "prop"`
GLB tree (e.g., a `house_glb` loaded from a `.glb` file) at a given offset just as easily as it
nests a hand-built primitive well.

## Why

The current implementation (`spawn_primitive_children`) only recurses into `nested_prefab.children`.
It never reads the nested prefab's top-level `kind` or `model` fields, so:

- `kind: "actor"` / `kind: "prop"` → the GLB is never loaded; only a bare anchor spawns.
- `kind: "primitive"` with a top-level `model` but no `children` (single-shape like `"Sphere"`)
  → the shape is silently dropped.

This creates a trap: a designer puts `(prefab: Some("oak_tree"))` in a composite prefab and sees
nothing in-game with no error. Completing this feature closes the trap and makes nested prefabs
work uniformly across all three kinds.

## Approach

### What needs to change in the spawner

`spawn_primitive_children` in `runtime/scene_manager/scene_loader.rs` currently does this when
it encounters a nested prefab reference:

```rust
let anchor = commands.spawn((Name::new(nested_key.clone()), child_tf, ...)).id();
commands.entity(parent).add_child(anchor);
spawn_primitive_children(commands, anchor, &nested_prefab.children, ...);
```

It must additionally dispatch on `nested_prefab.kind` after spawning the anchor:

- **`"primitive"` with `children`** — current behaviour; recurse. No change needed.
- **`"primitive"` with no `children`** — spawn a single mesh child using
  `build_primitive_mesh(&nested_prefab.model, &nested_prefab.primitive)` and the prefab's
  material, parented to the anchor. Mirrors the "single primitive mesh" branch in the outer
  entity loop.
- **`"actor"` / `"prop"`** — load the GLB via `model_spawner.spawn(...)` using the
  `AssetCatalog` path, apply any model fixes from `MergedModelFixes`, parent to the anchor.
  Mirrors the `spawn_prefab_instance` call in the outer entity loop.

### Context the helper needs

`spawn_primitive_children` currently takes a `ChildSpawnCtx` (meshes, materials, built catalog)
plus `&PrefabCatalog`. For GLB support it also needs:

- `&AssetCatalog` — to resolve `model` key → file path
- `&AssetServer` — to call `asset_server.load(path)` for the GLB
- `&ModelSpawner` — to call `model_spawner.spawn(...)`
- `&MergedModelFixes` — to pass to `model_spawner.spawn(...)`
- `&str` project_root — for `resolve_project_path`

These are all already available in `spawn_scene_v2` at both call sites. Extend `ChildSpawnCtx`
with references to these, or pass them as separate parameters.

### Error handling

If a nested `"actor"` / `"prop"` prefab's `model` key is missing from the asset catalog, push to
`load_errors` (same wording as the outer entity loop) and skip — do not panic.

### Validation

No schema changes needed; the validation rules added in the first phase already cover all cases.
Consider adding a soft warning (not a hard error) in `validate()` when a `"primitive"` prefab with
no `children` is referenced as a nested child while having an empty `model` — that's likely a
typo in the RON.

## Tasks

- [x] Extend `ChildSpawnCtx` with the additional refs needed for GLB dispatch
      (`AssetCatalog`, `AssetServer`, `ModelSpawner`, `MergedModelFixes`, `project_root`)
- [x] In `spawn_primitive_children`, after spawning the anchor, dispatch on `nested_prefab.kind`:
  - [x] `"primitive"` + no `children` → single-mesh branch
  - [x] `"actor"` / `"prop"` → GLB branch via `spawn_prefab_instance`
- [x] Update both call sites in `spawn_scene_v2` to pass the new context fields
- [x] Add a `kind: "prop"` prefab (`rock_deco`) to the `primitive_world` demo and
      nest it inside `"village"` to exercise the new path end-to-end
- [x] Update `docs/20_data_formats.md` — remove the "Current limitation" warning box; update
      the nested prefab section to document all three kinds
- [x] Update `crates/ironhold_core/src/CLAUDE.md` — note GLB dispatch in the spawner helper
- [x] Add a `ron_validation.rs` test: a catalog with a `kind: "actor"` prefab referenced as a
      nested child validates OK (schema allows it even before runtime wires it up)

## Acceptance criteria

- Given `(prefab: Some("oak_tree"))` where `"oak_tree"` is `kind: "prop"` pointing to a GLB,
  when the scene loads, then the GLB mesh appears at the correct world position.
- Given `(prefab: Some("beacon"))` where `"beacon"` is `kind: "primitive"` with `model: "Sphere"`
  and no `children`, when the scene loads, then a sphere mesh appears at the correct position.
- All existing composite-primitive nested prefabs (`village` → `well`, `house_cottage`, etc.)
  continue to work identically.
- A missing model key in the asset catalog produces a `load_errors` entry and skips the nested
  prefab, not a panic.
