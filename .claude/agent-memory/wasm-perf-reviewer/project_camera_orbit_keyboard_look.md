---
name: camera-orbit-keyboard-look
description: camera_orbit_system per-player keyboard camera-look — O(1) key checks per camera/frame, keys pre-resolved at spawn, zero deps
metadata:
  type: project
---

`camera_orbit_system` (capabilities/camera.rs) is a per-frame Update system running once per active `OrbitCamera` (up to MAX_SPLIT_PLAYERS=4). The keyboard-look feature added a `keyboard_input: Res<ButtonInput<KeyCode>>` param and, per camera/frame, up to 4 `Option<KeyCode>.is_some_and(|k| pressed(k))` checks + a few float ops for yaw/pitch. All O(1), no allocations, no new queries.

Key strings (`InputMap.look_left/right/up/down`) are pre-resolved to `Option<KeyCode>` ONCE at spawn via `InputMap::parse_key`, mirroring how `orbit_lmb`/`orbit_rmb` are pre-resolved. Two spawn sites: `spawn_orbit_camera_for_player` (entity_spawner.rs) and `spawn_scene_v2` (scene_loader.rs). Confirmed spawn-time-only, never re-parsed per frame.

**Why:** validates the "pre-resolve RON strings at spawn, store enum on the component, hot path only compares" idiom as the correct pattern for input-string bindings in this engine.
**How to apply:** if future input-binding features re-parse strings inside a per-frame system, flag it — the established fix is spawn-time pre-resolution into the component.

Binary size: zero new deps; `Res<ButtonInput<KeyCode>>` already used elsewhere, new `parse_key` match arms (Comma/Period/etc.) and struct fields are negligible. No shader/pipeline touched.
