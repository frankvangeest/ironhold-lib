---
name: dynamic-animation-control-demo
description: canonical demo project for PlayAnimationOn start_at_fraction/freeze; holds the flycam-default-speed-100 trap and the fact test_web.py baselines EVERY scene with no exclusion hook
metadata:
  type: project
---

Added 2026-08-26 on `feature/dynamic-animation-control`. `assets/projects/dynamic_animation_control/`
is the canonical designer-facing demo for `Action::PlayAnimationOn`'s `start_at_fraction`
(`Option<f32>`, 0.0–1.0 fraction of clip duration — deliberately NOT named `start_at`, to avoid a
seconds/fraction ambiguity) and `freeze` (`bool`, default `false`). Two scenes:
`main.scene.ron` (four frozen poses at 0/50/75/100%) and `continue.scene.ron` (freeze:false,
including a looping-alias mid-stride seek). All poses are driven from `logic/state_machine.ron` (`global_on` bindings) on
`scene.ready:{stem}` — no Rust, no player, flycam only. (Originally authored against `rules.ron`;
migrated to `state_machine.ron` along with every other shipped project by
`rules_to_state_machine_consolidation`.)

**Reusable traps this project surfaced:**

1. **Flycam defaults are tuned for terrain-scale worlds, not dioramas.** `FlyCamDef` defaults are
   `speed: 100.0` / `fast_speed: 200.0` (docs/20 ~1896). Any demo whose whole set dressing fits
   in ~24 m needs an explicit `flycam: (speed: ~6.0, fast_speed: ~15.0)` or the designer flies out
   of the scene on the first W tap and can never inspect what the demo is teaching. `foliage_demo`
   sets `8.0/20.0`; `terrain_demo`/`custom_materials` correctly rely on the defaults because their
   worlds are huge. Check this on EVERY new small-scale flycam demo.

2. **Baseline exclusion now EXISTS (verified 2026-10-02):** `test_web.py` has
   `NON_DETERMINISTIC_SCENES` (keyed "project/scenes/x.scene.ron", ~line 60) — excluded from the
   baseline diff. Settling is still frame-count based (`SCREENSHOT_SETTLE_FRAMES = 120`) while
   animation/sim advance on time/ticks, so any always-moving scene (animation, ocean, physics)
   must be listed there or made frozen. Flag new projects that don't.

3. **`clips:`-alias looping and un-freezing are asserted in scene labels but not in docs.** See
   [[animation-policy-doc-gaps]].

**Registration:** `test_web.py` `PROJECTS` and the `index.html` gallery card are the two steps
root CLAUDE.md lists (plus the baseline PNG). `docs/60_contributing.md` documents a FOURTH step
CLAUDE.md omits — "Add a new test here whenever a new project is added" for
`crates/ironhold_cli/tests/validate_projects.rs`. That file has drifted badly (missing
`foliage_demo`, `stats_demo`, `blank_project`, `camera_modes`, `dynamic_animation_control`), and
`README.md`'s "Example projects" table has drifted the same way (still only 5 of ~14 projects).
Reconcile the two lists rather than flagging each new project individually.
