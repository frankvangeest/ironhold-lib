---
name: flycam-spectator-priority
description: Flycam-vs-player camera priority (spectator mode) — shared-WASD footgun, docs location, and the stale-baseline gap when a scene is added to an existing project
metadata:
  type: project
---

A scene containing both a `tags: ["player"]` entity and a standalone `tags: ["flycam"]` entity is
supported ("spectator mode"): the flycam wins camera priority, the player still plays but gets no
camera. Canonical example: `assets/projects/camera_modes/scenes/flycam_spectator_test.scene.ron`,
reachable from that project's hub via the teal "Spectator" pad. Docs live in
`docs/20_data_formats.md` under `### Special tag: "flycam"` → `#### Spectator mode`.

The non-obvious designer footgun: **both entities read raw input independently, so default WASD
drives the player AND the flycam at the same time.** There is no engine-side arbitration. The
remedy is authoring-side only — re-bind the flycam (`flycam: (forward: "ArrowUp", ...)`) or the
player (`inputs:`). Check this is stated in docs, not just in a scene comment, whenever this
section is touched.

**Why:** implemented 2026-08-17 on `feature/flycam-scene-conflicts`; the shared-key behaviour is
inherent to two independent input consumers and will keep surprising designers.

**How to apply:** when reviewing any change to flycam/camera-priority behaviour, verify the docs
subsection still carries the shared-key note plus the two documented known limitations (runtime
`Action::Spawn` players still get their own camera; `SetCameraMode` without `owner_player` can
convert the flycam itself).

Related review lesson: CLAUDE.md's baseline-screenshot registration step only covers *new
projects*. Adding a new `scenes/*.scene.ron` to an **existing** project silently skips it — check
`screenshot_baselines/scenes/{project}_{scene}.png` exists for the new scene, and that the hub
scene's own baseline is regenerated when a portal pad is added to it. See also
[[camera-config-party-split-nesting]], [[local-coop-demo-room-conventions]] (hint-label character
ceilings).
