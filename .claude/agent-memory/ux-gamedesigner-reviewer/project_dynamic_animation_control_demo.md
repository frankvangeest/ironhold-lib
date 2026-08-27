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
including a looping-alias mid-stride seek). All poses are driven from `logic/rules.ron` on
`scene.ready:{stem}` — no Rust, no player, flycam only.

**Reusable traps this project surfaced:**

1. **Flycam defaults are tuned for terrain-scale worlds, not dioramas.** `FlyCamDef` defaults are
   `speed: 100.0` / `fast_speed: 200.0` (docs/20 ~1896). Any demo whose whole set dressing fits
   in ~24 m needs an explicit `flycam: (speed: ~6.0, fast_speed: ~15.0)` or the designer flies out
   of the scene on the first W tap and can never inspect what the demo is teaching. `foliage_demo`
   sets `8.0/20.0`; `terrain_demo`/`custom_materials` correctly rely on the defaults because their
   worlds are huge. Check this on EVERY new small-scale flycam demo.

2. **`test_web.py` has no per-scene baseline exclusion mechanism.** `discover_scenes()` globs
   `scenes/*.scene.ron` unconditionally and every hit gets a committed baseline diffed at
   `BASELINE_DIFF_THRESHOLD = 0.04`. A RON comment claiming a scene is "excluded from screenshot
   baselines" is therefore always false. `wait_for_scene_ready` settles on a frame count
   (`SCREENSHOT_SETTLE_FRAMES = 120`) but animation advances on delta time, so any scene with
   mid-clip / looping animation at screenshot time is genuinely flaky. Either make every scene in
   a new project screenshot-deterministic, or add a real skip list.

3. **`clips:`-alias looping and un-freezing are asserted in scene labels but not in docs.** See
   [[animation-policy-doc-gaps]].

**Registration:** `test_web.py` `PROJECTS` and the `index.html` gallery card are the two steps
root CLAUDE.md lists (plus the baseline PNG). `docs/60_contributing.md` documents a FOURTH step
CLAUDE.md omits — "Add a new test here whenever a new project is added" for
`crates/ironhold_cli/tests/validate_projects.rs`. That file has drifted badly (missing
`foliage_demo`, `stats_demo`, `blank_project`, `camera_modes`, `dynamic_animation_control`), and
`README.md`'s "Example projects" table has drifted the same way (still only 5 of ~14 projects).
Reconcile the two lists rather than flagging each new project individually.
