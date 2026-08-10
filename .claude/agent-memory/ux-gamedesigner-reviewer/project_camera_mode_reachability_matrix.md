---
name: camera-mode-reachability-matrix
description: HISTORICAL — camera_mode: used to be ignored on the split/party spawn paths; fixed before v1 shipped. Kept for the review lesson about checking WHICH spawn path an example exercises.
metadata:
  type: project
---

> **STATUS: FIXED — do not cite the table below as current behaviour.** The v1 post-implementation
> review (2026-08-07, 5 agents converging) caught this before ship; the split/party/dynamic
> dispatch now resolves through a `resolve_orbit_config_for_multiplayer` helper (Orbit payload
> wins, non-Orbit warns and falls back), pinned by
> `test_split_screen_honors_camera_mode_orbit_not_just_legacy_camera_field`. `local_coop_demo`
> room11 (v2) authors `camera_mode: Orbit((...))` + sibling `split:` on a 2-player scene and works.
> The **review lesson** below still stands and is the reason to keep this memory.

`components.camera_mode` (`CameraModeDef`, shipped in `camera_modes.md` v1) was **not** honoured on
every camera spawn path. Verified 2026-08-07 on `feature/camera-modes-v1`, fixed before ship:

| Authoring site | Does `camera_mode:` take effect? |
|---|---|
| `tags:["player"]`, scene has **1** player | **Yes** — `spawn_active_camera_for_player` → `resolve_camera_mode`; all 6 variants dispatch |
| `tags:["player"]`, scene has **2+** players (split / party / dynamic) | **No** — `spawn_orbit_camera_for_player` / `spawn_party_orbit_camera` read `&player_config.camera` (the legacy field) directly. Orbit-only, silently |
| `tags:["flycam"]` | Yes for `Flycam(...)`; any other variant **warns** and falls back to `FlyCamDef::default()` |
| Prefab with **neither** tag | Ignored entirely — no camera, no warning |

`PlayerConfig` is built in exactly one place (`assemble_player_config`, `entity_spawner.rs`) and
sets `camera: prefab.components.camera.clone().unwrap_or_else(default_camera_config)` — it never
derives `camera` from `camera_mode`. So a co-op prefab that migrates to `camera_mode: Orbit((...))`
and **deletes** its `camera:` block silently gets `default_camera_config()` on screen: offset
(0,5,10), look_at_offset (0,2,0), zoom_speed 10.0, orbit_speed 0.5, radius 2..20,
orbit_button `"Either"`. In a split-screen scene that last one re-enables shared-mouse orbit that
`orbit_button: "None"` was authored to suppress.

By contrast `split:`/`party:` **do** resolve correctly from either home — `components.split` first,
falling back to `components.camera.split`. But `split:`/`party:` written *inside* the
`Orbit((...))` payload parse cleanly (they are real `CameraConfig` fields, no `deny_unknown_fields`)
and are silently dropped — the single most likely migration mistake, since the docs describe
`Orbit` as "reuses the exact struct `camera:` already uses".

**Why:** the v1 plan scoped mode-generic dispatch to the single-player branch only, but the docs
and the shipped `local_coop_demo` room4 migration present the co-op shape as fully supported.

**How to apply:** any review of a `camera_mode:` change must check *which spawn path* the example
exercises, not just that the RON parses or that `ironhold_cli validate` passes — neither catches
this. Ask for an assertion that the authored offset/orbit_button actually reaches the spawned
camera in a 2-player scene. Related: [[camera-config-party-split-nesting]],
[[split-switch-prefab-duplication]], [[warn-vs-silent-fallback-principle]].
