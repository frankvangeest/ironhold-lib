---
name: render-layers-reserved-scheme
description: RenderLayers reserved-layer convention (per-viewport target ring visibility) — camera-must-match-ring invariant, hardcoded [0,1,2,3,4] union coupled to MAX_SPLIT_PLAYERS, and the unvalidated player_index modulo collision
metadata:
  type: project
---

Introduced by `SplitScreenDef.own_viewport_only` (per_viewport_target_ring_visibility, reviewed
2026-07-31, ALIGNED w/ warnings). First designer-facing use of Bevy `RenderLayers` in this crate
(prior use: `inspector.rs` debug camera, layer 31, `#[cfg(feature = "inspector")]`).

**Reserved-layer map:** 0 = ordinary scene geometry (implicit — every componentless entity/camera).
1..=4 = per-split-player target rings, `1 + player_index % MAX_SPLIT_PLAYERS` (same modulo scheme
`PLAYER_LABEL_COLORS` uses). 31 = inspector.

**Invariant to check on ANY new `RenderLayers` consumer:** a ring carries ONLY its own non-zero
layer (no layer 0), so *every camera that should see it must carry that layer explicitly*. Missing
this is invisible in tests that only assert the restricted case — it manifests as a camera that
renders zero rings. Three camera spawn sites currently handle it:
`spawn_split_camera_for_player` (static Grid + hot-join), the inline `dynamic`-split loop in
`spawn_players_and_camera`, and `spawn_party_orbit_camera` (union). A 4th,
`spawn_player_entity`'s plain OrbitCamera (non-hot-join `Action::Spawn` of a `tags:["player"]`
prefab), does NOT — that camera sees no rings in `OwnViewportOnly` mode.

**Two known-unvalidated collision paths (warn candidates, not blockers):**
- `player_index >= MAX_SPLIT_PLAYERS` (e.g. 0 and 4) → same layer → a "private" ring leaks into
  exactly one other player's viewport. Only existing validation is "2+ players with player_index 0".
- duplicate non-zero `player_index` (authored, or hot-join forcing `player_index = next_slot` into
  a scene whose starting players authored non-contiguous indices) → same layer, feature silently
  defeated.

**Maintenance coupling:** `spawn_party_orbit_camera` hardcodes `RenderLayers::from_layers(&[0, 1,
2, 3, 4])`. Raising `MAX_SPLIT_PLAYERS` (which the Grid over-cap `warn!` text literally invites)
silently drops the new players' rings from the merged/party view. Derive from the const instead.

Related: [[local_coop_pattern]], [[targeting_capability_pattern]].
