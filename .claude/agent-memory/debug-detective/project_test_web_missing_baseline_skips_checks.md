---
name: test-web-missing-baseline-skips-checks
description: test_web.py auto-creates a missing scene baseline and returns before the browser-console-error check, so a newly added scene silently passes without ever being verified
metadata:
  type: project
---

`test_web.py`'s `discover_scenes()` globs `scenes/*.scene.ron`, so a newly authored scene is
auto-discovered with no registration step. But `test_screenshot_scene_baseline()` does
`if update or not baseline_path.exists(): copy; return` — it `return`s **before** the
`if errors: raise TestFailure("Browser errors: ...")` check at the end of the function.

**Why:** this means "`python test_web.py` passed" is not evidence a new scene works. On its first
run the new scene gets its baseline created from whatever rendered (including a black screen or a
scene that logged console errors) and is reported as OK. Several `local_coop_demo` rooms
(room6, room8, and room9 as of 2026-07-31) have no committed baseline, so they are all currently in
this never-actually-verified state.

**How to apply:** when a feature's acceptance criteria are browser-observable and the feature adds
a new scene, do not accept a green `test_web.py` as verification. Either generate the baseline
first (`python test_web.py --update-baseline <scene>` / `--update-baselines`) and re-run so the
diff+console-error path actually executes, or require a manual playtest. Also relevant when
`--skip-build` is used against a stale `pkg/`. Related:
[[project_webgpu_headless_black_screen]] (the WebGPU build can't screenshot at all in this sandbox,
which is how a black baseline gets committed in the first place).
