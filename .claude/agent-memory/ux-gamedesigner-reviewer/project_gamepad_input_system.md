---
name: gamepad-input-system
description: Gamepad/controller InputMap + *_gamepad_bindings fields, keyboard-additive truth, unclaimed-pad-only footgun, camera-yaw parity gap, canonical demo examples
metadata:
  type: project
---

Gamepad support lives on `InputMap` (per-player, in a prefab's `components.inputs`). Fields:
`gamepad_index: Option<usize>`, `gamepad_jump` ("South"), `gamepad_run` ("East"),
`gamepad_interact` ("West"), `gamepad_target_next` ("North"), `gamepad_deadzone` (0.15).
Scene/project-level triggers live in `global_gamepad_bindings` (ProjectConfig) /
`scene_gamepad_bindings` (GameSceneV2): `Map<button name, trigger name>`, per-key overlay merge,
mirroring `global_key_bindings`/`scene_key_bindings`. Shipped in feature/gamepad-hot-join.

**How to apply:**
- **CORRECTED 2026-07-29 (verified in source): keyboard and gamepad are ADDITIVE, never
  exclusive.** `input_translator_system` (runtime/input.rs) always reads the keyboard keys and then
  *adds* stick/button input; `camera_orbit_system` (capabilities/camera.rs) likewise adds
  right-stick pitch on top of keyboard look keys. A player with `gamepad_index` set can use their
  authored keyboard scheme AND the pad simultaneously. The earlier "gamepad instead of keyboard"
  belief in this memory was WRONG — do not repeat it.
  Status of the once-stale exclusivity claims, re-verified 2026-07-31: `docs/20_data_formats.md`'s
  `gamepad_index` row is FIXED (now "**This is additive, not a replacement**"); every
  `local_coop_demo/prefabs/prefabs.ron` commented `gamepad_index` hint is FIXED ("input is
  additive, not a replacement"); `docs/20`'s `Action::JoinPlayer` note is FIXED. **Still stale:**
  `crates/ironhold_core/src/CLAUDE.md`'s "Gamepad routing" paragraph (~L775) — "bind to a specific
  gamepad instead of the keyboard".
- **`*_gamepad_bindings` only fire on an UNCLAIMED pad** (no live player's `gamepad_index`, and no
  in-flight hot-join spawn holds it). Two consequences designers trip on: (a) `"South": "join"` is
  safe despite being `gamepad_jump`'s default — the keyboard "pick a key nobody uses" warning does
  NOT transfer; (b) it is unusable for ordinary in-game triggers — `{"Start": "toggle_pause"}`
  silently never fires for any actual player, because their pad is claimed. The field name reads
  parallel to `global_key_bindings` and hides this; consider `unclaimed_gamepad_bindings`-style
  naming/warning wording on any future change here.
- Button names use Bevy compass naming. GOTCHA: `LeftTrigger`=bumper (LB/L1), `LeftTrigger2`=analog
  trigger (LT/L2). The Xbox/PS columns in the docs table are load-bearing.
- **Compass names are engine vocabulary, not player vocabulary.** On-screen prompts must say
  "A"/"Cross", never "South". room8's `room_hint`/`join_prompt` do this correctly.
- gamepad interact + target_next work per-player in local co-op.
- **Permanent parity gap:** no gamepad camera *yaw* (right-stick-X turns the character; only pitch
  is camera). Keyboard `look_left`/`look_right` remain the only yaw.
- Right-stick-Y pitch reuses `CameraConfig.look_speed`.
- **The "Valid gamepad button names" table heading enumerates its consumers** — correctly lists
  `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings` (the maps WERE renamed with
  the `unclaimed_` prefix, as recommended) and `ActionSlotDef.gamepad_key` alongside the four
  `InputMap` fields. Any future consumer of `parse_gamepad_button` must add itself there; recurring
  staleness trap.
- **The four `InputMap` button fields all default to face buttons (South/East/West/North), which
  makes them a silent collision surface for every new per-player gamepad binding.** Nothing
  cross-checks a new binding against them. Concrete live case: `ActionSlotDef.gamepad_key: "South"`
  on a player whose `gamepad_jump` is the default "South" both jumps and fires the ability. When
  reviewing a new gamepad field, insist examples pick a NON-default button (`"RightTrigger"` = RB/R1
  is the genre-conventional free one) and that the overlap is called out.
- Canonical examples: `entity_logic_demo/prefabs/prefabs.ron` (single-player pad),
  `local_coop_demo/prefabs/prefabs.ron` (commented `gamepad_index: 0/1` on player_p1/p2 — note the
  `*_grid` variants have no such hint), `local_coop_demo/scenes/room8.scene.ron`
  (`scene_gamepad_bindings`).
- `ironhold validate` warns on unrecognised button names in both bindings maps (implemented).
