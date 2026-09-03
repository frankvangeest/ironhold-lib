---
name: project_camera_shake_retrigger_ambiguity
description: CameraShake re-trigger behavior is documented three contradictory ways across plan/docs; verify against shipped impl
metadata:
  type: project
---

`Action::CameraShake(duration_secs, intensity)` (added ~2026-06-19, plan `38bb186`) has a re-trigger
semantics discrepancy across designer-facing artifacts:

- `docs/20_data_formats.md` Actions table row: "Re-triggering while a shake is active restarts it with the new parameters."
- `planning/features/camera_shake.md` Approach > Executor: "pick max intensity and sum duration (capped at 3.0 s)".
- Same plan, Acceptance criteria: "replaces it with the new parameters (no merge/cap)".

**Why:** Whichever the shipped executor actually does, the doc must match it — a designer firing two
kills in quick succession will see different camera behavior than the doc promises if the impl merges/caps.

**How to apply:** When reviewing CameraShake docs, confirm the doc row's re-trigger sentence matches the
shipped executor arm in `runtime/scene_manager/action_executor.rs`. Do not assume the doc is correct.
Canonical copy-paste examples live in 3rd_person_game_demo dead-state entry_actions (spider/snake/orc/alpaking;
note enemy_zombie was NOT given a shake — inconsistent coverage).
