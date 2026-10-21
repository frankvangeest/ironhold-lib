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

Post-impl review of input_strafe_bindings (2026-10-16): strafe default None, example blocks cleaned, shipped strafe lines deleted (no leftover Q/E hints). Open: docs/15 strict table is 3-col but the 3 new rows have 4 cells (Fix text dropped by GFM); `movement_key_slot_collision` skips players with NO `inputs:` block (defaults never compared); global_key_bindings collision message names the scene file, not the .project.ron or the trigger; local_coop_demo P3 look_right "KeyP" = toggle_pause and docs/20 IJKL look table repeats KeyP while claiming "no key collisions"; docs/20 "Invalid key strings produce a warn!" is false for player inputs (no runtime warn); docs/15 invalid_key row says "uses the default key" but for inputs it unbinds.

Post-impl review of first_person_starter v1 (2026-10-17): FirstPerson `fov` is VERTICAL (straight into Bevy PerspectiveProjection) but the starter prefab comment calls it "horizontal-ish, 90 is the usual shooter default" (90 vertical ~ 120 horizontal at 16:9). Overriding only strafe_left/right under `controls: FirstPerson` leaves A/D DEAD (scheme unbinds left/right) - neither the prefab comment nor the docs/20 snippet (which overrides only strafe_left) says so. docs/20 `gamepad_run` row still says "toggle run" (now run_mode). Plan acceptance line still expects a warn for ThirdPerson+FirstPerson camera; code/docs deliberately don't. `query bindings` omits gamepad_index, so the "stick does nothing" footgun is invisible there. cursor.lock_lost only fires for a lock the engine requested and did not release itself (pause-release is safe). Playtest risk: Escape keydown leaking alongside the lock release in non-Chrome browsers would double-fire toggle_pause + lock_lost.

**How to apply:** at post-implementation review, verify each item above landed; see [[project_docs_lag_actions]], [[project_warn_vs_silent_fallback_principle]], [[project_quoted_string_vs_enum_house_style]].
