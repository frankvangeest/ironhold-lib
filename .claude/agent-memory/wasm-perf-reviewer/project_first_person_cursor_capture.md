---
name: project-first-person-cursor-capture
description: first_person_starter v1 (2026-10-17 review) - cursor_capture_system per-frame write guards, web pointer-lock DOM listeners, FP look per-frame parse_key, hide_own_body
metadata:
  type: project
---

Reviewed 2026-10-17 (feature/first_person_starter v1). Verdict OK with minor concerns, nothing blocking.

- `cursor_capture_system` (capabilities/cursor_capture.rs, Update, runs while paused, ALL projects on web): writes
  `CursorOptions` only on a just_pressed click or in the release branch whose condition
  (`requested || grab_mode != None || !visible`) self-clears after one write. Reads through Mut don't mark Changed.
  So no per-frame `request_pointer_lock()`. Idle cost: atomic load + single_mut + empty query. Re-check this
  condition if anything ELSE ever writes CursorOptions (would cause a per-frame write fight).
- Web runner: 2 leaked DOM closures (pointerlockchange/error), one-time; web-sys Document/Element/EventTarget/Event
  features almost certainly already unified-in via winit/wgpu -> ~0 size.
- `first_person_camera_system` calls `InputMap::parse_key` per frame for 4 look keys (Orbit pre-resolves at spawn).
  Defaults are None for FirstPerson controls -> zero cost; single lowercase letter = 1 tiny String alloc/key/frame (nit).
- input_translator: FP-camera `.any(contains)` over <=4 cams per gamepad player per tick (trivial); Hold run_mode emits
  one Run(held) message/tick/player (Messages Vec reused; nobody consumes Changed<CharacterController>).
- hide_own_body = Visibility::Hidden on player root: descendants culled early in check_visibility AND excluded from
  shadow views (no self-shadow - UX not perf). No animation_policy on the FP body -> no graph eval.

Related: [[project-animation-hot-path]], [[project-camera-modes-v2]], [[project-wasm-size]]
