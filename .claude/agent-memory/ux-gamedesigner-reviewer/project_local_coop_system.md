---
name: local-coop-system
description: Local co-op split-screen Stage 1 RON surface — first-entity-wins camera rule, gamepad_index, max_view_box, canonical example project
metadata:
  type: project
---

Local co-op (Stage 1, single-machine, NOT networked) adds four RON-authorable fields. Canonical example: `assets/projects/local_coop_demo/`.

- `PrefabDef.player_index: u32` (default 0) — top-level prefab field, sibling to kind/model. Player slot for scenes with 2+ `tags:["player"]` entities. Duplicate index = silent overwrite footgun.
- `InputMap.gamepad_index: Option<usize>` (in components.inputs) — binds player to Nth-connected gamepad (connection order, not USB port). Documented in docs/20 InputMap table (~line 1610).
- `CameraConfig.party: Option<PartyZoomDef>` (in components.camera) — authored on the FIRST scene-entity player ONLY; party on later players is ignored ("first entity wins"). PartyZoomDef { zoom_margin: f32 (required), allow_manual_zoom: bool (default false) }.
- `GameSceneV2.max_view_box: Option<(f32,f32,f32,f32)>` = (min_x, min_z, max_x, max_z) hard XZ clamp.

Fallback rule: 2+ players but no party block on first player -> engine logs warning + single orbit camera following player 1 only (never spawns competing cameras).

**Why:** These are the first RON fields whose behavior depends on entity ORDER in the scene list, not just field values — a new class of footgun for designers.

**How to apply:** When reviewing co-op changes, check that the "first entity wins" ordering rule is stated in BOTH the scene RON comments and prefab RON comments (a designer copying only prefabs.ron loses the context). Distinct from LAN Co-op networking (planning/features/networking_multiplayer.md Form 1, Beta 0.6) which is UNSHIPPED — watch for designer confusion between the two.

Doc alignment as of 2026-07: docs/20_data_formats.md covers all four fields well (party in CameraConfig table + PartyZoomDef section ~1675; player_index in PrefabDef table ~1516; gamepad_index in InputMap table ~1610; max_view_box in GameSceneV2 table ~182). This was the rare case where the doc-writer kept pace with schema.
