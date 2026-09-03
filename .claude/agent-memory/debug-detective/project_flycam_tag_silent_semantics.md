---
name: flycam-tag-silent-semantics
description: The "flycam" tag silently discards the prefab's own GLB model and silently keeps only the last flycam entity (and any flycam at all when the scene also has a player) — verified from pixels, not just code
metadata:
  type: project
---

`scene_loader.rs`'s entity loop early-`continue`s on `is_flycam` **before** the model-catalog
lookup, and `flycam_start` is a single `Option` consumed only in the `else if` arm after
`!player_configs.is_empty()`. Three silent outcomes, none of which emit any `warn!`:

1. A flycam-tagged prefab's own `model:` is never spawned — no mesh, no load error. Documented as
   by-design in `docs/20_data_formats.md` ("The `model` field is ignored"), but the engine says
   nothing at runtime.
2. Two flycam-tagged entities in one scene → **last one in `entities:` wins**; the earlier one
   contributes nothing at all (no camera, no body).
3. A scene with both a player-tagged and a flycam-tagged entity → the flycam is dropped entirely
   (the player branch wins the if/else).

**Verified empirically** (2026-08-17, throwaway project + real-GPU browser screenshot): GLB
`kind: Prop` **and** `kind: Actor` entities both render perfectly under a flycam camera — Prop vs
Actor has zero runtime divergence anywhere in `src/` (only `spawn_primitive_children`'s nested-
prefab arm mentions them, grouped together). So "flycam can't render Prop models" is only ever
outcome 1 above, never a Prop-specific rendering defect.

**Why:** every flycam prefab shipped in this repo is authored `kind: Prop, model: ""`, so "flycam"
and "Prop" look coupled in the RON even though they aren't in the code — an easy false lead.
Related: no flycam project in the repo (terrain_demo, custom_materials, foliage_demo, camera_modes)
contains a single GLB entity, so flycam+GLB has no screenshot-baseline coverage at all.

**How to apply:** when a flycam bug is reported, check these three silent paths before suspecting
rendering; and treat "the demo projects prove it works" as unavailable for flycam+GLB.
Related: [[project_camera_mode_switch_spawn_only_state]]. The camera_mode-vs-legacy-camera
dual-source bug this used to also link to (`project_camera_mode_dual_source.md`) is now FIXED
and that memory file was deleted.
