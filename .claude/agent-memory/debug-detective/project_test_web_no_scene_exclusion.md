---
name: test-web-no-scene-exclusion
description: test_web.py screenshots EVERY *.scene.ron a project has, with no opt-out — so any non-deterministic scene becomes a permanently failing baseline once its PNG is committed
metadata:
  type: project
---

`test_web.py`'s `discover_scenes(project)` globs `assets/projects/<project>/scenes/*.scene.ron`
and `test_screenshot_scene_baseline` is run for every one. There is **no skip list, no exclusion
flag, and no naming convention** that opts a scene out (grepped for
`SKIP`/`EXCLUDE`/`skip_scene`/`NON_DETERMIN` — nothing). `BASELINE_DIFF_THRESHOLD = 0.04` (4%).

Two consequences that bite when adding a project:

1. A plan that says "this scene is non-deterministic by construction, excluded from screenshot
   baselines" is **not implementable without a code change to test_web.py**. Once its PNG is
   auto-created, every later run diffs a freely-running animation/particle phase against a frozen
   frame and blows the 4% threshold — the browser suite goes permanently red, not flaky-red.
2. This compounds [[project_test_web_missing_baseline_skips_checks]]: the missing-baseline branch
   copies the screenshot and `return`s *before* `if errors: raise TestFailure(...)`, so the very
   first run of a new project never checks its console errors either.

**How to apply:** when registering a new project, either make every scene deterministic (freeze
animations, no particles) or add a real exclusion mechanism to `test_web.py` first. And always
generate + commit the baselines in the same change as the `PROJECTS` list entry and the
`index.html` card — the card's `<img src="screenshot_baselines/scenes/<project>_main.png">` is a
broken image on the live GitHub Pages gallery until that PNG exists.
