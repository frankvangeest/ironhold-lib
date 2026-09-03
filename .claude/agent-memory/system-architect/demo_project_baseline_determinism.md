---
name: demo-project-baseline-determinism
description: test_web.py auto-globs EVERY *.scene.ron for a screenshot baseline; NON_DETERMINISTIC_SCENES now provides a per-scene exclusion mechanism — use it as a last resort, prefer settling content first
metadata:
  type: project
---

**FIXED.** `test_web.py` now has an explicit exclusion mechanism: `NON_DETERMINISTIC_SCENES` (a
`set`, keyed the same way as `PROJECTS`: `"project/relative/scene/path.scene.ron"`) is checked in
`discover_scenes()` (`if f"{project}/{rel}" not in NON_DETERMINISTIC_SCENES`), which is exactly the
"real per-scene exclusion list" this note used to recommend as a fallback. As of this update it
excludes exactly the scene that motivated this note —
`dynamic_animation_control/scenes/continue.scene.ron` (the permanently-looping walk clip) — with a
comment explaining why: "they never converge to a fixed pixel state ... so any committed baseline
would flake against `BASELINE_DIFF_THRESHOLD` regardless of run count."

**How to apply:** when a new demo project lands, still check every scene for continuously-moving
content (looping animation, particles with lifetime > frame, physics settling, anything
time-driven) — the preference order is unchanged: fold non-deterministic examples into a
deterministic scene, or make the content settle (non-looping clip holding its final frame,
screenshot taken after it holds), before reaching for `NON_DETERMINISTIC_SCENES`. The list exists
now, so there's no more "no mechanism" excuse for skipping the exclusion when settling truly isn't
an option — but it should stay a last resort, not a default landing spot for every new animated
scene.

Related: [[animation-seek-freeze-constraints]] (a *frozen* pose is baseline-safe; a playing one is
not).
