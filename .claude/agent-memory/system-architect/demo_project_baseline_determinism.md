---
name: demo-project-baseline-determinism
description: test_web.py auto-globs EVERY *.scene.ron for a screenshot baseline with no per-scene exclusion — any animated/moving demo scene is a guaranteed flake
metadata:
  type: project
---

`test_web.py` has **no mechanism to exclude a scene from screenshot baselines**. `discover_scenes()`
globs every `*.scene.ron` under `assets/projects/{name}/scenes/`, and the runner iterates all of
them against `BASELINE_DIFF_THRESHOLD = 0.04` (4% of pixels, `PIXEL_TOLERANCE = 15` per channel).
Adding a project to the `PROJECTS` list opts *every* one of its scenes in.

**Why:** repeatedly relevant when reviewing new demo/QA projects. A scene comment saying "excluded
from baselines" is aspirational, not enforced — this exact claim appeared in
`dynamic_animation_control/scenes/continue.scene.ron` (2026-08-26 review) for a scene containing a
permanently *looping* walk clip, which cannot pass a 4% threshold.

**How to apply:** when a new demo project lands, check every scene for continuously-moving content
(looping animation, particles with lifetime > frame, physics settling, anything time-driven). Push
back with one of: fold the non-deterministic examples into a deterministic scene; make the content
settle (non-looping clip that holds its final frame, screenshot taken after it holds); or add a real
per-scene exclusion list to `test_web.py` if "some scenes are inherently non-deterministic" turns out
to be a recurring need rather than a one-off.

Related: [[animation-seek-freeze-constraints]] (a *frozen* pose is baseline-safe; a playing one is
not).
