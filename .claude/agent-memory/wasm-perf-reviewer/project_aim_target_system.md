---
name: aim-target-system
description: first_person_starter v2 aim_target_system (targeting.rs) per-frame cost profile, LOS raycast bound, warmup gap in the starter project
metadata:
  type: project
---

`aim_target_system` (capabilities/targeting.rs, Update, Targeting chain, before first_person_camera_system) reviewed 2026-10-22.

- Runs every sim frame in EVERY scene; with no FirstPersonCameraMode camera it only does `ReadRapierContext::single()` + `player_targets.iter().count()` (cheap, could early-return).
- Per aiming camera: O(N Targetable) scan; range check before projection (good); `project_to_viewport` recomputes `view.affine().inverse()` PER TARGET (hoistable nit, glam scalar on wasm32).
- Candidates Vec borrows `&str` SpawnIds (no String clones); `Vec::new()` only allocates when something is in the 3-deg catch circle.
- LOS = one Rapier cast_ray per candidate nearest-first until clear; unbounded K in theory but catch circle keeps it tiny.
- PlayerTarget/GameVariables/events written only on change (Deref compare, no Changed trigger) - correct.
- `is_aim_driven` in tab_targeting runs per controller per frame BEFORE the key-press gate (<=16 iters, alloc-free nit).
- action_bar_input_system mouse_button check short-circuits on `cursor_lock.locked`; fast path unchanged.
- first_person_starter project uses blend-only sprite particles (hit_spark/death_poof) with NO scene.ready warmup -> first hit stalls 0.3-1 s on WASM. Check whether it got fixed.

Related: [[first-person-cursor-capture]], [[per-player-targeting]], [[targeting-capability]].
