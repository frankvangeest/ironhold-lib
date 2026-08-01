---
name: hot-join-input-prefab-coupling
description: Local co-op hot join (keyboard v1 + gamepad v2) — per-slot join prefabs, pad-binds-only-at-join-time limitation, and which earlier gaps are now closed
metadata:
  type: project
---

Hot join lets a player enter an already-`Grid`-split scene at runtime. A joiner gets its
`InputMap` from `join_prefab_keys[slot]` (per-slot `Vec<Option<String>>`, 0-based absolute slot),
so **keyboard seats must be distinct prefabs** (WASD/Arrows/IJKL/Numpad) — one prefab cannot yield
two usable keyboard maps. Gamepad joins don't have that problem: the pressing pad is bound at join
time. Canonical example: `assets/projects/local_coop_demo/scenes/room8.scene.ron` (starts at 2,
grows to 4; correctly NOT room6, which already starts at the 4-player cap).

**Shipped surface:**
- keyboard: `scene_key_bindings: {"KeyG": "join"}` + rule `ui.button_pressed:join -> JoinPlayer`
  (KeyG not KeyJ — KeyJ is P3's own strafe key, and `global_input_system` reads keys unconditionally).
- gamepad (v2, feature/gamepad-hot-join): `scene_unclaimed_gamepad_bindings: {"South": "join"}`
  (verified field name 2026-08-01 — NOT `scene_gamepad_bindings`), same rule.
  `Action::JoinPlayer` overrides the joiner's `InputMap.gamepad_index` to the pressing pad (and the
  spawn-time `OrbitCamera` picks that override up, so right-stick pitch works for the joiner).
  See [[gamepad-input-system]] for the unclaimed-pad-only semantics.

**Designer-visible limitation to keep flagging until documented:** a pad binds **only at join
time**. A player who joined via keyboard can never be handed a controller afterwards, and
scene-authored players (P1/P2 in room8) only get a pad if their prefab authors `gamepad_index`.
So demo hints of the form "P3: IJKL..., or a gamepad" overpromise — the pad option only exists on
the branch where that player joined *by* pad.

**Previously-flagged gaps now CLOSED (do not re-report):**
- Joiner spawn-point lookup is `player_{next_slot + 1}_start` — 1-based, matching the demo rooms'
  authored names. The old 0-based mismatch is fixed.
- `ironhold validate` now cross-checks `join_prefab_keys` prefab refs (exists, has `tags:["player"]`,
  not primitive-shaped) and unrecognised `*_gamepad_bindings` button names.

**Still true / accepted:** only `Grid` split supports hot join (party/dynamic/Vertical/Horizontal
no-op with a warn); `PlayerIndex` is overridden to the slot, ignoring the prefab's baked
`player_index`; `coop.lobby_full` fires on the join that reaches the cap but `SetEntityVisible`
cannot hide a scene-authored UI `Label`, so join prompts are always-visible; at most one
gamepad-triggered join is serviced per frame (a same-frame second press is dropped, not queued).
See [[local-coop-system]], [[player-index-owner-player-wiring]].
