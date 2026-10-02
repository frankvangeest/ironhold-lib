---
name: lock-on-plan-review
description: lock_on_camera_mode.md plan-review (2026-10-02) — no per-player target-clear input exists (gamepad can't release lock); player-in-frame unguaranteed; offset_space "Target" collides with targeting vocab
metadata:
  type: project
---

Plan-reviewed `planning/features/lock_on_camera_mode.md` at `34803b1` (2026-10-02), verdict "Needs more design work", 2 blockers.

**Durable facts found (verify before reusing):**
- There is NO per-player "clear target" input. Release paths are only: click empty space (mouse), `ClearTarget` action (primary player's `CurrentTarget` only), target death/hide/despawn. Any feature that keys behaviour off `PlayerTarget` being `Some` strands gamepad / split non-primary players. Recommended `target_clear` + `gamepad_target_clear` (R3 "RightThumb" = genre lock-on convention).
- `camera_modes` project player prefabs author no `target_next` -> inherit "Tab" -> broken targeting in WASM. Any targeting demo added there must set `"KeyT"`.
- No field table exists for `FollowCameraDef` in docs/20 (only a Payload column); CameraConfig's `offset` described as "(right, up, back)" which is only true in character space.
- Framing features that lerp focus toward a target must guarantee the PLAYER stays in frame; acceptance criteria tend to only check the target.

**Why:** these recur on any targeting- or camera-framing plan.
**How to apply:** on lock-on v2 implementation review, check B1 release inputs and B2 player-in-frustum criterion landed; check renamed `offset_space` variant (recommended `Local`) and `distance_zoom`->`separation_zoom`. Related: [[per-player-targeting-gating]], [[camera-transition-and-default-sentinel]], [[ron-enum-double-paren]], [[warn-vs-silent-fallback-principle]].
