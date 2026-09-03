---
name: project_camera_shake_retrigger_ambiguity
description: RESOLVED — CameraShake re-trigger REPLACES (no merge/cap); the plan doc's "max intensity + sum duration" language was never shipped. Docs table's "restarts it with the new parameters" is the accurate one.
metadata:
  type: project
---

`Action::CameraShake(duration_secs, intensity)` (added ~2026-06-19, plan `38bb186`) had a re-trigger
semantics discrepancy across designer-facing artifacts: `docs/20_data_formats.md`'s Actions table
row said "restarts it with the new parameters", while `planning/features/camera_shake.md`
contradicted itself between "pick max intensity and sum duration (capped at 3.0s)" (Approach) and
"replaces it with the new parameters (no merge/cap)" (Acceptance criteria).

**RESOLVED — confirmed against the shipped executor.** `action_executor.rs`'s `CameraShake` arm
(~957-991) does a plain `commands.entity(camera_entity).insert(CameraShakeState { remaining:
duration_secs, duration: duration_secs, intensity })` per matching camera — `insert` unconditionally
overwrites any existing `CameraShakeState` component. There is no read-then-merge/cap logic anywhere
in the arm. **Re-triggering REPLACES**, matching the docs/20 table row and the plan's Acceptance
criteria; the plan's own "max intensity and sum duration" Approach text was never implemented and
should be treated as stale planning prose, not a spec.

**How to apply:** cite this as settled — a designer firing two kills in quick succession sees the
new shake's parameters take over immediately, not a merged/capped blend. Canonical copy-paste
examples live in 3rd_person_game_demo dead-state entry_actions (spider/snake/orc/alpaking; note
enemy_zombie was NOT given a shake — inconsistent coverage, still worth flagging on that specific
point).
