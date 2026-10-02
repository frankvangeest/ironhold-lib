---
name: material-sharing-and-blend-facts
description: GLB/catalog StandardMaterials are shared across instances (never mutate in place); Bevy 0.18 Blend shadow/prepass/pipeline-variant facts; rapier rescales colliders on Transform.scale; one-shot Added<> fade-start trap
metadata:
  type: project
---

Facts verified during the fade_out_despawn plan-review (2026-10-02, at `34803b1`):

- **GLB materials are shared, never cloned per instance.** `SceneRoot(asset_server.load(..))` reuses the GLTF's material handles; `apply_material_overrides` inserts ONE catalog handle on every mesh. Corpses reuse the live monster's GLB (`zombie_corpse.model: "zombie"`). Any per-entity visual change (fade, tint, hit-flash) must clone per instance — mutating `Assets<StandardMaterial>` in place hits every sibling.
- **`apply_material_overrides` is deferred** — it waits for GLTF mesh children (`material_factory.rs` `if mesh_entities.is_empty() { continue; }`) and will later OVERWRITE any per-instance clone made earlier. Any "on X, rewrite this entity's materials" system must treat `PendingMaterialOverride` present / zero `Mesh3d` descendants as not-ready, not a one-shot `Added<>`.
- **Bevy 0.18 Blend:** casts shadows (`render/light.rs` ~1901, MAY_DISCARD), and the shadow prepass discards only below alpha 0.05 (`pbr_prepass_functions.wgsl` PREMULTIPLIED_ALPHA_CUTOFF), so the shadow stays full strength until the last few percent, then pops. Opaque→Blend therefore costs up to TWO new pipelines (Transparent3d main + MAY_DISCARD shadow); depth prepass just drops Blend (`prepass/mod.rs` ~913). `NotShadowCaster` removes the second.
- **bevy_rapier3d 0.33 `apply_scale`** rescales every Collider on `Changed<GlobalTransform>` — scaling a root Transform for a cosmetic effect changes physics geometry.

**Why:** These are the traps for any per-instance visual-effect feature (fade, dissolve, hit flash, highlight).
**How to apply:** In reviews, check that a shared handle is cloned before mutation, that application is retried instead of one-shot, and that the shadow/pipeline-variant cost and any collider side effects of scale changes are accounted for. Related: [[wasm-pitfalls]], [[deferred-despawn-double-queue]], [[schedule-update-vs-fixedupdate]].
