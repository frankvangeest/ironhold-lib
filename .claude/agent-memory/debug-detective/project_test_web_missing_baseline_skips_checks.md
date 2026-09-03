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
`--skip-build` is used against a stale `pkg/`.

**Note on the dangling `project_webgpu_headless_black_screen` link this file used to carry:**
that memory file doesn't exist. The closest match, [[project_browser_pixel_probe_recipe]],
documents a real but **distinct** headless-rendering pitfall — headless Chromium on this machine
has no WebGPU adapter at all, so a `--features webgpu` build screenshotted headless renders a
blank/black canvas — but it does NOT explain this file's core mechanism. `test_web.py`'s own
`wasm-pack build` call (no `--features webgpu` flag) always builds the WebGL2-fallback backend,
which doesn't hit that no-adapter problem, so a baseline `test_web.py` auto-creates is not
generally a black screen for that reason. If a genuinely black/broken baseline is ever suspected
here, verify the *actual* rendered pixels with `project_browser_pixel_probe_recipe`'s
`--real-gpu`/non-headless recipe before assuming either cause — do not assume the WebGPU-adapter
issue explains it without checking, since the build backends differ.
