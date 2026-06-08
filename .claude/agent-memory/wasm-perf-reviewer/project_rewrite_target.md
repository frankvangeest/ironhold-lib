---
name: project-rewrite-target
description: rewrite_target() string-substitutes {target} per action pushed; only allocates when actions fire, not per-frame
metadata:
  type: project
---

`rewrite_target(action, target_id)` in `crates/ironhold_core/src/runtime/scene_manager/message_interpreter.rs` (~line 243). Canonical copy moved here; `action_bar.rs` now imports it (its old private `substitute_target` was deleted — dedup, good).

**Why:** Implements `{target}` substitution (mirror of `rewrite_self`). Called by all interpreter systems (message/fsm/entity_fsm) and `action_bar_input_system` before `action_queue.push`.

**How to apply:**
- Cost is `.replace("{target}", ...)` allocations PER ACTION PUSHED — i.e. only when a rule/FSM/action-bar slot actually fires and emits actions. NOT per-frame. The interpreters already `.clone()` the action before push, so this adds one extra String alloc per substitutable string field on an already-cloned action. Negligible vs existing `rewrite_self` which does the same thing.
- `target_id` is fetched once per system run via `current_target.0.as_deref().unwrap_or("")` — cheap.
- `message_interpreter_system` now takes `Res<CurrentTarget>` — added param, no new per-frame work beyond the one deref.
- WASM-safe; no deps, no blocking, no GPU.

Link: [[project-targeting-capability]] (sets CurrentTarget).
