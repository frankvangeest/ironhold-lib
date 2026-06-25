---
name: project-nameplate-system
description: Nameplate capability (capabilities/nameplate.rs) WASM hot-path profile — 3 Update systems, change-detection guards, cleanup collect
metadata:
  type: project
---

`crates/ironhold_core/src/capabilities/nameplate.rs` — world-space nameplate widgets (Text2d name + Mesh2d pixel bars on an unparented WorldLabel anchor). Three Update systems registered in lib.rs ~L255-257.

**Per-frame cost profile (web build):**
- `nameplate_visibility_system` — runs every frame, iterates `Query<(Entity,&NameplateTag,&NameplateAnchor,&GlobalTransform)>` (~8 entities in 3rd_person_game_demo). `npc_q.contains(entity)` is **O(1)** (Bevy `Query::contains` is archetype/sparse-set membership, NOT O(log n)). Writes to Visibility are change-detection-guarded (`*vis != Hidden`). Cheap. Early-returns if `config.options` is None.
- `nameplate_cleanup_system` — runs every frame on `RemovedComponents<NameplateTag>`. **`removed.read().collect::<HashSet<Entity>>()` allocates every frame even when no removals** (then `is_empty()` returns early). On an empty iterator `HashSet::with_capacity(0)` does NOT allocate the backing table until first insert, so empty-frame cost is a zero-cap struct on the stack — negligible. Anchor query iteration only runs when set is non-empty.
- `world_pixel_bar_update_system` (stat_display.rs ~L218) drives the fill-bar scale per frame; properly guarded (`abs() > 0.5` on scale.x, `mat.color != new_color`). This is the real per-frame render-touch for nameplates, not nameplate.rs itself.

**Per-spawn cost (`Added<NameplateTag>`, one-shot):** `nameplate_setup_system` does `format!` Name strings + `meshes.add(Rectangle)` + `color_materials.add(ColorMaterial)` per stat bar. Each unique Mesh2d/ColorMaterial combo is a new WebGPU pipeline compile on first draw (~100-300ms WASM stall). Mesh2d/ColorMaterial use the Bevy sprite pipeline — fully WebGL2 AND WebGPU compatible, no compute/storage. `meshes.add()` only runs in the `Added<>` setup path, so it does NOT mark Assets<Mesh> changed every frame.

**Resources are `Option<ResMut<Assets<..>>>`** for headless-test compat (early return). WASM always has them.

**Binary size:** zero new deps. `std::collections::HashSet` is already in the WASM blob (used widely); importing it adds nothing.
