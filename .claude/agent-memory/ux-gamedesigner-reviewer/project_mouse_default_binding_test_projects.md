---
name: mouse-default-binding-test-projects
description: Which demo projects exercise DEFAULT mouse bindings (orbit_button "Either", strafe_mouse_button Left) vs override them — matters for any UI-click-vs-camera playtest
metadata:
  type: project
---

Any playtest checking "does a left-click on UI also orbit/strafe?" must run in a project that uses
the DEFAULT mouse bindings, or it passes falsely.

- `3rd_person_game_demo` player prefabs set `orbit_button: "Right"` (left-click never orbits there),
  but leave `strafe_mouse_button` at its default `Some("Left")` — so LMB-held + A/D still strafes.
- `local_coop_demo` prefabs set `orbit_button` to `"Right"`/`"None"` and `strafe_mouse_button: None`
  almost everywhere — useless for default-binding checks.
- `primitive_world` and `stats_demo` author NO `camera:` block on `player_capsule` → full defaults
  (`orbit_button: "Either"`, strafe Left). Both also have an `ActionBar`. Use these.

Observed 2026-09-30 while plan-reviewing `action_bar_mouse_click.md` (its "Required" orbit check
said "default orbit binding" but named only 3rd_person_game_demo / local_coop_demo room3).

**How to apply:** when a feature adds clickable UI (slots, buttons over the 3D view), check the
playtest names primitive_world/stats_demo for the camera-conflict step. Related: [[action-bar-single-player-assumptions]], [[ui-trigger-wiring]].
