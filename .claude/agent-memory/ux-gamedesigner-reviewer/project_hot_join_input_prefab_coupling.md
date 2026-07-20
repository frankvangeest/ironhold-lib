---
name: hot-join-input-prefab-coupling
description: Local co-op hot-join — joiner input map comes from its spawn prefab, so a single fixed join prefab collides keyboard bindings; per-slot prefabs needed for keyboard join
metadata:
  type: project
---

For the local-coop hot-join feature (planning/features/local_coop_hot_join_leave.md, v1),
a newly-joined player gets its `InputMap` from whatever prefab it spawns from.

**The coupling designers/plan-authors miss:** whether a single fixed `join_prefab_key` works
depends entirely on the INPUT SOURCE:
- **Gamepad join** — single fixed prefab is fine; the physical device + auto-assigned
  `gamepad_index` provides the distinction between seats.
- **Keyboard join** — single fixed prefab is BROKEN by construction: press-join twice spawns two
  players with the *same* keyboard scheme (they move together / collide). Local-coop keyboard seats
  MUST have distinct `inputs:` blocks — see local_coop_demo prefabs.ron player_p1..p4_grid
  (WASD / Arrows / IJKL / Numpad). One prefab cannot yield two distinct keyboard maps.

Because the WASM test harness (`test_web.py`) and Frank's browser playtest drive the KEYBOARD (not
gamepads — WebGPU headless gamepad injection isn't available), the keyboard path is the one that
gets exercised, so a single-prefab v1 would ship a demo that visibly fails at the 4th joiner.

**Recommendation stance:** `join_prefab_keys: [..]` per-slot list (indexed by target slot), NOT
`join_prefab_key: String`. This composes with scene-wide auto-assign-lowest-free-slot (the right
answer for the join-trigger question). Also unresolved in that plan: joiner spawn position (no
authored transform since no `entities:` entry) and `player_index` assignment vs the prefab's baked
`player_index` (owner_player action bars route by it — see [[player-index-owner-player-wiring]]).

**How to apply:** on any hot-join / dynamic-player-count review, check that keyboard seats get
distinct input and that the demo target room does not already start at MAX_SPLIT_PLAYERS (room6
starts at 4 = the cap; wiring join there as-is only exercises the no-op warn path).
See [[local-coop-system]].

**SHIPPED v1 state (reviewed at feature/local-coop-hot-join-leave, uncommitted):** the design
landed as `join_prefab_keys: Vec<Option<String>>` (per-slot, indexed by 0-based absolute slot) +
payload-less `Action::JoinPlayer`, bound via `scene_key_bindings: {"KeyG":"join"}` + a
`ui.button_pressed:join -> JoinPlayer` rule. Canonical example: `local_coop_demo/scenes/room8.scene.ron`
(starts at 2, grows to 4 — correctly NOT room6). docs/20_data_formats.md "Local co-op hot join"
section is exemplary (covers all 4 v1 limits incl. KeyJ-collision reasoning). Two recurring traps
this feature surfaced, worth re-checking on any spawn-point / hot-join review:
- **Joiner spawn-point lookup is 0-based** (`format!("player_{}_start", next_slot)` in
  action_executor.rs, next_slot 0-based) but the demo rooms author 1-based names
  (`player_1_start`..`player_4_start`, copied from room6 where spawn_points are DEAD/unread because
  players are transform-placed). Result in the shipped room8: 3rd joiner (slot 2) looks up
  `player_2_start` which is P2's own spot -> spawns on top of P2; `player_4_start` is never read.
  Fix = rename demo spawn_points to `player_0_start`..`player_3_start`. docs step 4's "same
  convention room6 uses" claim is misleading (room6 never consumes them).
- **`join_prefab_keys` prefab refs are NOT validated by `ironhold validate`** (validate.rs cross-
  checks `entities[].prefab` at line ~254 but has no branch for join_prefab_keys) — a typo'd key
  there is runtime-warn-only, inconsistent with every other scene->prefab reference.
