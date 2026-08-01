---
name: camera-config-party-split-nesting
description: party:/split: are nested INSIDE components.camera (the CameraConfig struct), so any future camera_mode: enum forces them to relocate — plus the agreed per-camera action targeting recommendation
metadata:
  type: project
---

`components.camera` (`CameraConfig`, ~14 flat fields) is the biggest single RON component block a
designer hand-writes, and the two local-co-op switches `party:` (`PartyZoomDef`) and `split:`
(`SplitScreenDef`, itself nesting `dynamic:` = `DynamicSplitDef`) live **inside** it, read from the
**first** `"player"`-tagged scene entity only.

**Why this matters for any camera refactor:** `planning/features/camera_modes.md` proposes replacing
`camera: (...)` with `camera_mode: Orbit(...)` while explicitly keeping split-screen a separate,
mode-agnostic layer. Those two goals collide at the RON level — `split:` currently has no home
outside `camera:`, so the enum forces a relocation decision (sibling `split:` next to
`camera_mode:`? inside every variant?) that the plan does not yet make. Same for `party:`, which is
simultaneously slated to become a `Party` *variant*. Also note the new schema makes the old
"party + split both set" error state look syntactically natural (`camera_mode: Party(...)` +
`split: (...)`), and `split.dynamic` internally swaps between a per-player Orbit camera and a shared
party camera at runtime — i.e. the engine itself mutates the active mode, which can fight a
designer's `SetCameraMode`.

**Agreed targeting recommendation (plan-review 2026-08-01):** per-camera actions in a multi-camera
scene should take an optional `owner_player: u32` (the established per-player field name — see
[[player-index-owner-player-wiring]]), **not** a viewport/slot index, since designers already author
`player_index` on prefabs and viewport slots are engine-assigned. Omitted = applies to *all* active
cameras (matches the plan's own CameraShake acceptance bar); `owner_player` on a party-mode scene or
an unjoined hot-join slot should warn, not silently no-op (see
[[warn-vs-silent-fallback-principle]]).

**How to apply:** when reviewing camera schema changes, check that (a) `party`/`split`'s authoring
location is stated explicitly, (b) the "first player entity wins" rule survives the move, (c)
docs/20_data_formats.md's ~330-line camera block (CameraConfig table through DynamicSplitDef,
~lines 2023-2350) is on the update list, and (d) hot-join (`join_prefab_keys`, see
[[hot-join-input-prefab-coupling]]) is covered — a player joining mid-session needs a defined answer
for which camera mode they get.
