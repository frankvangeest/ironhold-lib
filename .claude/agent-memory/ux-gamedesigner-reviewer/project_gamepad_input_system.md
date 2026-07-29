---
name: gamepad-input-system
description: Gamepad/controller InputMap fields, defaults, co-op support, camera-yaw parity gap, and canonical demo examples
metadata:
  type: project
---

Gamepad support lives on `InputMap` (per-player, in a prefab's `components.inputs`). Fields: `gamepad_index: Option<usize>` (None = keyboard-only; when set, that player reads gamepad *instead of* keyboard), `gamepad_jump` ("South"), `gamepad_run` ("East"), `gamepad_interact` ("West"), `gamepad_target_next` ("North"), `gamepad_deadzone` (0.15).

**Why:** shipped in feature/gamepad-controller-input (merged after per_player_camera_look_controls). Defaults match values every scene hardcoded before the fields existed.

**How to apply:**
- Button names use Bevy compass naming. Docs table maps them to physical: South=Xbox A/PS Cross, East=B/Circle, North=Y/Triangle, West=X/Square. GOTCHA: `LeftTrigger`=bumper (LB/L1), `LeftTrigger2`=analog trigger (LT/L2) — counterintuitive; the Xbox/PS columns in the docs button-names table are load-bearing, a designer guessing from the name alone will be wrong.
- gamepad interact + target_next WORK in local co-op (per-player checks), not single-player-only. Any doc/comment implying single-player-only is stale.
- **Permanent parity gap:** there is NO gamepad camera-yaw. Right-stick-X drives character turning; keyboard split-screen players can yaw camera via look_left/look_right, gamepad players can only pitch. Documented as deliberate, not an oversight.
- Right-stick-Y camera pitch reuses `CameraConfig.look_speed` (same dial as keyboard look_up/down) — a gamepad-only player's `camera:` block referencing look_speed is intentional, not a leftover. See [[world_stat_bar_style_landscape]]-adjacent CameraConfig docs.
- Canonical examples: entity_logic_demo/prefabs/prefabs.ron (commented single-player uncomment-and-go block in "player"); local_coop_demo/prefabs/prefabs.ron (player_p1/player_p2 commented gamepad_index: 0/1).
- Docs: docs/20_data_formats.md InputMap table (~"gamepad_jump" row), the "Valid gamepad button names" table, worked RON example, and two callout notes (look_speed reuse + camera-yaw parity gap).
- **The "Valid gamepad button names" table heading enumerates its consumers** (`gamepad_jump`/`gamepad_run`/`gamepad_interact`/`gamepad_target_next`). Any new feature that reuses `parse_gamepad_button` (e.g. `scene_gamepad_bindings`) must add itself to that heading or the table reads as InputMap-only. Recurring staleness trap.
- **Compass names are engine vocabulary, not player vocabulary.** On-screen prompt Labels must say "A"/"Cross", never "South" — the Xbox/PS columns exist precisely because no player recognises the compass name. Check any demo prompt text on gamepad features.
- **`gamepad_index: Some(n)` silently disables that player's whole keyboard `inputs:` block** (gamepad instead of, not in addition to, keyboard) AND removes camera yaw (parity gap above). So any HUD label describing a prefab's keyboard scheme ("P3: IJKL + U + O") becomes a lie the moment that same prefab is driven by a pad.
