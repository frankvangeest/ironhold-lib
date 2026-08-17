---
name: browser-pixel-probe-recipe
description: How to actually get rendered pixels for a visual bug on this machine — headless Chromium has no WebGPU adapter, so a probe must run non-headless; RON edits need no wasm rebuild
metadata:
  type: project
---

For any "X doesn't render" investigation, the committed `pkg/` on `integration` plus `serve.py` is
enough to get real pixels in minutes — **no `cargo`/`wasm-pack` build**, because the WASM binary
fetches `assets/**` over HTTP at runtime. A throwaway project under `assets/projects/<name>/`
(project + assets + prefabs + scenes + logic/rules.ron) is visible immediately at
`play.html?project=<name>&testing=1&scene=scenes/<file>.scene.ron`.

**The one blocker:** headless Chromium on this machine has **no WebGPU adapter** — both
`test_web.py`'s `CHROMIUM_ARGS_GL` and its SwiftShader `CHROMIUM_ARGS_WEBGPU` set produce
`No available adapters` → Bevy panics `Unable to find a GPU!` and you get a blank canvas that looks
like the bug you're chasing. Launch playwright with `headless=False` and
`["--enable-unsafe-webgpu", "--no-sandbox", "--disable-setuid-sandbox"]`
(`test_web.py`'s `--real-gpu` args) — then it renders correctly.

Other practicalities: `playwright install chromium` was not yet done on this machine (had to run
it once); `tools/bin/ironhold` (the cached CLI) does **not** exist here, so `validate` is
unavailable without a full `cargo` build — the engine's own scene-load `load_errors` block in the
browser console is the substitute, and it names every skipped entity and unresolved prefab/model
key. Console `[log]` lines carry the `INFO`/`WARN` prefix inside a `%c`-formatted string, so filter
on the message text, not `msg.type`.

**How to apply:** reach for this before writing an ECS-level integration test whenever the question
is genuinely "what appears on screen"; use a test only for state the pixels can't show.
