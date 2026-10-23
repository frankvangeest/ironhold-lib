---
name: debug-state-dom-sync
description: sync_debug_state_to_dom (lib.rs, PostUpdate, wasm32-only, ships in release) — per-frame JSON build + DOM write; cannot be gated out or dirty-skipped
metadata:
  type: project
---

`sync_debug_state_to_dom` writes `debug_state_json(&DebugState)` into `#debug-state` every frame, release builds included.
Reviewed 2026-10-23 (feature/debug_state_json): json_escape per string field = 6 small Rust allocs/frame (was 4), char loop over
~100-300 B, sub-microsecond; switch set_inner_html -> set_text_content removes an HTML fragment parse (net win on the DOM side).

**Why it can't be removed/throttled:** play.html's loading overlay + `body[data-ready]` readiness signal is the FIRST write
to #debug-state (MutationObserver), so compiling it out of release builds breaks the live demo's loading screen. test_web.py
uses `frame` as a progress counter (`>= entry + settle`, `> frame`), so "skip when unchanged" is impossible (frame always
differs); every-N throttle would be tolerated by the `>=`/`>` predicates but only adds latency for ~µs saved.

**How to apply:** don't re-flag this as a per-frame blocker. `last_action` is `{:?}` of one Action; actions.rs has no nested
Vec<Action>/Box<Action> payloads, so it is bounded (~tens to a few hundred bytes). If future work wants to shave it, the only
sane lever is caching the escaped last_action/scene in update_debug_state (they change per-action / per-scene-load).
