---
name: fps-control-scheme-cursor-capture
description: InputMapDef serde(from) layering (controls/run_mode), FirstPersonCameraDef look/capture/hide fields, cursor.lock_lost runner contract; review traps found 2026-10-17
metadata:
  type: project
---

first_person_starter v1 (feature/first_person_starter, reviewed 2026-10-17):
- `InputMap` = resolved type via `#[serde(from = "InputMapDef")]`; layered key fields use a `written` deserialize_with (three-state Option<Option<String>>). Scheme defaults live in `scheme_key()` + an inline run_mode match in schema/player.rs: that is the scheme *definition*, not hardcoding. `sources` map is CLI-only.
- Right-stick owner = live `FirstPersonCameraMode` camera (translator skips stick-X Turn). Keyboard `look_*` keys now also drive FP look (was Orbit-only) -- check any project with look_* + FirstPerson camera.
- Cursor capture: capture click hardcoded to Left mouse; `cursor.lock_lost` is web-only (fires only when the runner reports); no rule bound = game keeps running with mouse look gated off.
- Traps found: (1) starter binds Escape to `toggle_pause` AND lock_lost -> LoadSceneOverlay+PauseGame; if the browser delivers Escape keydown too, toggle can cancel the pause (suggest an idempotent `pause` trigger). (2) hide_own_body drops nameplate/stat widgets at spawn (assemble_player_config) but SetCameraMode un-hide only restores the body, not the widgets. (3) controls_camera_mismatch warns one direction only, diverging from the plan's acceptance criterion. (4) validate source_file hardcodes "prefabs/prefabs.ron" (see [[validate-cross-file-blind-spots]]).

**How to apply:** when extending FP (v2 aim/mouse_button) re-check these; the Escape wiring is copied by every designer who starts from the starter.
