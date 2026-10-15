---
name: fps-controls-plan-review
description: 2026-10-16 plan-review of input_strafe_bindings + first_person_starter; tri-state Option gap, docs strafe example blocks, camera_modes Q vs flycam
metadata:
  type: project
---

Plan-review 2026-10-16 (plans at eb56cf3) of `input_strafe_bindings.md` (prereq) and `first_person_starter.md`.

Key findings to re-check at implementation review:
- Layering "authored > scheme > default" needs TRI-STATE per key (omitted vs authored `None` vs value); plain `Option<String>` + serde default cannot tell omitted from `None` once a scheme changes the default. Check how it was solved.
- docs/20 has ~8 example InputMap blocks (lines ~2374, 2488, 2749-3140) writing strafe_left/right, several with the SAME key as left/right (ArrowLeft, KeyJ, Numpad1). Once strafe is wired, copying them = turn+strafe on one key. Prereq plan only listed the table.
- camera_modes: hints never advertise Q/E; flycam default `down` is KeyQ and flycam_spectator_test spawns player_orbit_demo + flycam -> Q would strafe player AND descend flycam. Designer view: delete.
- `action_bar.no_target:{key}` already exists -> miss sound/muzzle flash on a no-target shot is pure RON; plan's open Q3 missed it.
- `gamepad_index` doc row says "Local co-op only" but a single-player FPS needs it for gamepad at all.
- first_person_starter still had stale `movement_style: Strafe` / `Classic` names from pre-consult design.
- Pointer lock: Escape consumed by browser to release lock; Escape-to-pause needs an explicit one-press decision.

**How to apply:** at post-implementation review, verify each item above landed; see [[project_docs_lag_actions]], [[project_warn_vs_silent_fallback_principle]], [[project_quoted_string_vs_enum_house_style]].
