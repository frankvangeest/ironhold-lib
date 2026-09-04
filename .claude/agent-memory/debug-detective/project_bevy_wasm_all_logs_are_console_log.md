---
name: bevy-wasm-all-logs-are-console-log
description: tracing-wasm has ONLY console.log bindings, so every Bevy level incl. ERROR reaches DevTools as console.log — DevTools error badge stays 0, the "Errors" filter hides them, and test_web.py's msg.type=="error" gate was blind to all of them; fixed by a play.html shim
metadata:
  type: project
---

`tracing-wasm` 0.2.1 (installed unconditionally by `bevy_log`'s `#[cfg(target_arch = "wasm32")]`
branch as `WASMLayer::new(WASMLayerConfig::default())`) declares **only** `console.log` bindings —
`log1`/`log2`/`log3`/`log4`, every one `#[wasm_bindgen(js_namespace = console, js_name = log)]`.
There is no `console.error` or `console.warn` binding in the crate at all. Every Bevy level from
TRACE to ERROR arrives in DevTools as a `console.log`, distinguished only by a `%c` CSS colour
string (`%cERROR%c ...` + `color: red`).

**Why this matters:** three separate blind spots, all with the same cause.
1. DevTools' red error badge stays at **0** no matter how many `error!`s fire.
2. Filtering the console by **Errors** (or Warnings) — the first thing anyone does when hunting a
   failure — hides Bevy's own output entirely. An `error!` is real but invisible, buried among
   ~40 INFO lines per project load.
3. `test_web.py`'s console gate keyed on `level = msg.type; if level == "error"` — so the **whole
   browser test suite could never see any Bevy `error!`**, including every asset-load failure. Its
   substring fallback list (`panicked at`, `VALIDATION ERROR`, `wgpu error`, `No WebGPU`) has no
   `ERROR` entry, so a RON parse error in any project passed the suite green.

Root-caused 2026-09-04 on `feature/action-deny-unknown-fields`: a deliberately corrupted
`quick_scene/logic/rules.ron` was reported as producing "no console signal at all". It in fact
logged **two** correct errors (`bevy_asset` `server/mod.rs:570` "Failed to load asset ... RON parse
error", plus ironhold's own `project_loader.rs:172`) — both `msg.type == "log"`. Nothing was wrong
with the engine, the loader, `LoadState::Failed`, or the log bridge; the signal was purely
filtered out of view.

**Fix (applied):** a classic `<script>` shim at the top of `play.html`, before the deferred
`./pkg/ironhold_web.js` module import, that re-dispatches `%cERROR`-prefixed / `%cWARN`-prefixed
`console.log` calls to `console.error` / `console.warn`. Ten lines, no Rust change, no wasm
rebuild — RON/HTML edits are served live. This fixes blind spot 3 for free: `test_web.py` needs no
edit because its `msg.type == "error"` check now matches. Blast radius checked at the time:
`3rd_person_game_demo` and `quick_scene` (once repaired) emit zero `console.error`, so the shim
does not spuriously redden the suite. `LogPlugin`'s `fmt_layer` override cannot fix this — it is
only consulted in the non-wasm branch — and `custom_layer` is purely additive, so it would
double-log rather than replace `WASMLayer`.

**How to apply:** never conclude "no error appeared in the browser" from a filtered DevTools view
or from a playwright `msg.type` check on this project. Filter on **message text** (`%cERROR`,
`Failed to load asset`), or confirm the `play.html` shim is present in the checkout being served.
Related: [[project_browser_pixel_probe_recipe]] (which already noted the `%c`/`msg.type` hazard),
[[project_test_web_missing_baseline_skips_checks]] (an independent second way `test_web.py`
under-reports), [[project_serve_py_stale_checkout_trap]] (the other "symptom is a lie" trap to rule
out first).
