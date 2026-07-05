---
name: split-screen-viewport
description: Local co-op Stage 3 vertical split-screen — split_screen_viewport_system, ActiveSplitScreen gating, multi-camera cost, pipeline warmup coverage
metadata:
  type: project
---

Local Co-op Split-Screen Stage 3 (added ~2026-07, files: capabilities/camera.rs `split_screen_viewport_system` + `SplitViewportSlot`, schema/player.rs `SplitScreenDef`/`SplitOrientation`, scene_manager/mod.rs `ActiveSplitScreen` resource, entity_spawner.rs `spawn_players_and_camera`). First feature in the codebase to touch `Camera.viewport`/multi-camera rendering.

**split_screen_viewport_system (Update, unconditional registration):** first line is `let Some(orientation) = active_split.0 else { return };`. `ActiveSplitScreen` defaults to `None`, is set to `None` on every full LoadScene (action_executor) AND in every non-split spawn branch of `spawn_players_and_camera` (0-1 players, party, fallback). So on ALL existing single-player and Stage-1/2 co-op scenes it early-returns after ONE resource read (Res access is a pointer deref, no archetype scan). Free. Not a per-frame regression. Do not flag.

**Empty-archetype `Query<(&mut Camera, &SplitViewportSlot)>`:** even if the early-return weren't there, scenes with 0 SplitViewportSlot entities iterate nothing (Bevy 0.18 empty-archetype = free), same as party_camera_follow confirmed in Stage 1. `&mut Camera` in the query signature does NOT cause change detection to fire on unrelated Camera entities — change detection only triggers on entities the query actually MATCHES and you actually deref-mut. No SplitViewportSlot = no match = no Camera touched. Safe.

**Camera.viewport written unconditionally every frame when split IS active (2 cameras):** `camera.viewport = Some(Viewport{..})` every frame regardless of whether window resized. This DerefMut marks Camera as Changed every frame. In Bevy 0.18 the render app re-extracts Camera every frame ANYWAY (ExtractComponent/camera extraction is not change-gated in the main render path), so the Changed flag does not add extract cost. BUT: viewport changes can invalidate view-dependent GPU resources (viewport-sized textures) if the value actually differs; here the value is identical frame-to-frame so no GPU realloc. Net: writing unchanged viewport every frame is cheap. Guarding with `if camera.viewport != Some(new) {...}` matches CLAUDE.md change-detection discipline and is a cheap correctness-preserving nicety, but for a 2-camera opt-in demo it is NOT a measurable win. Recommend as optional follow-up only. SCOPED TO SPLIT SCENES ONLY (local_coop_demo room3), never touches single-player.

**Two Camera3d full-scene passes:** two active cameras each render the full 3D scene to half the window = roughly 2x draw calls / shader invocations / view-uniform sets vs one camera. This is INHERENT to split-screen, not a bug. WebGL2/WebGPU both support multiple cameras rendering to viewports of one surface (no MRT/compute needed). Expected cost, opt-in only.

**pipeline_warmup_system + multi-camera (question 3 concern is UNFOUNDED):** warmup (lib.rs) inserts `NoFrustumCulling` on EVERY `Mesh3d` entity for 4 frames, then removes it. `NoFrustumCulling` is a per-ENTITY component that disables culling in ALL views/cameras, not per-camera. So both split cameras' frustums are irrelevant during warmup — every mesh is force-visible to every camera, so every material pipeline compiles regardless of which camera sees it. No split-screen-specific warmup gap. The "entity only visible in one camera's frustum fails to warm for the other" scenario cannot occur.

Zero new deps, zero new shaders, zero new asset-loading paths (confirmed via git diff — only ECS/schema/RON changes). No binary-size impact.
