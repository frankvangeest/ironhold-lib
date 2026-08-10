---
name: camera-transition-and-default-sentinel
description: camera_modes v2 — transition: is read from the TARGET mode (so SetCameraMode("default") round-trips are asymmetric), and fov has three different defaults across payload structs
metadata:
  type: project
---

Shipped in `camera_modes.md` v2 (`feature/camera_modes_v2`, 2026-08-09). Two designer-facing
semantics that are easy to get wrong and are under-documented.

**1. `transition:` is read from the mode being switched TO, never from the action.**
`action_executor.rs` does `let transition = resolved_mode.transition().cloned();`. Consequences a
designer will hit:
- `SetCameraMode(mode: "default")` resolves to the camera's `AuthoredCameraMode` — i.e. the
  **prefab's own `components.camera_mode:`**. If that block has no `transition:` (or the prefab has
  no camera block at all and got the tag-driven `default_camera_config()`), the restore is an
  **instant cut** even though the outbound switch was a smooth blend.
- Both shipped v2 demos have this asymmetry: `entity_logic_demo` main (C = 0.4s EaseInOut out,
  V = snap back) and `local_coop_demo` room11 (Digit1 = 0.5s EaseInOut out, Digit2 = snap back).
  `entity_logic_demo` **cannot** be fixed without adding a `camera_mode:`/`camera:` block to a
  player prefab that deliberately has none.
- `transition:` authored on a *prefab's* singular `camera_mode:` does **nothing at spawn** — it is
  only ever consumed by a later `SetCameraMode(mode: "default")`. Same silent-field class as
  `ActionSlotDef.label` and `depth_scale`.

**2. `fov` has three different defaults depending on which payload struct you are in.**
`CameraConfig.fov` (the `Orbit` payload) = **45.0** (`schema/player.rs`, deliberately Bevy's
`PerspectiveProjection::default()` so omitting it reproduces pre-v1 framing).
`FollowCameraDef`/`FixedCameraDef` = **60.0** (`schema/camera.rs`'s own separate `default_fov()`).
`FirstPersonCameraDef` = **90.0**. So a `Fixed` preset that omits `fov:` silently widens 45°→60° on
every switch. The 45.0-for-parity reasoning was never carried into the new structs.
`docs/20_data_formats.md`'s `CameraConfig` table has stated `60.0` for `fov` since v1 — **wrong**;
verify before quoting it.

**Docs surface for the v2 section:** `docs/20_data_formats.md` `### camera_modes: registry and
SetCameraMode (v2)`. The `CameraModeDef` variant table's "Payload" column is a bare comma-separated
field list — there is **no** field table (types/defaults/required) anywhere for `FollowCameraDef`,
`FirstPersonCameraDef`, `FixedCameraDef`, `PartyCameraDef`, or `CameraTransition`.
`CameraTransition.duration_secs` is **required** (parse error if omitted); `ease` defaults to
`Linear`.

**How to apply:** on any camera-transition or preset review, check (a) whether the round-trip
back to `"default"` is as smooth as the outbound switch and whether the demo/doc says so, (b) which
`fov` default applies to the variant being authored, (c) that new payload structs get real field
tables, not just a Payload column. Related: [[camera-mode-reachability-matrix]],
[[quoted-string-vs-enum-house-style]], [[ron-enum-double-paren]].
