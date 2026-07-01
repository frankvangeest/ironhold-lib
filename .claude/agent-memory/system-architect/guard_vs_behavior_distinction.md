---
name: guard-vs-behavior-distinction
description: Input-arbitration guards (panel_open) legitimately live in Rust, not RON; LogicState is a scalar mode-slot, not an orthogonal-flag set
metadata:
  type: project
---

Suppressing a capability's message *emission* (an input-arbitration guard) is upstream of the Message→Interpreter→Action→Executor pipeline and legitimately belongs in Rust — it is NOT a data-driven-philosophy violation. The pipeline's data-driven contract governs the *response* to a message (what RON decides), not whether the message fires. Example: `LoadedInventoryUi.panel_open` (runtime-only bool) read by `interactable_system` (Update) and `collectible_system` (FixedUpdate) to skip F-key / walk-over emission while a panel is open. Same category as "ignore camera drag while cursor is over a UI button."

**Why:** Frank asked (2026-06-28) whether to route panel-open suppression through FSM state or GameEvents instead of the hardcoded bool. The bool is the correct guard mechanism; the real gap was the lack of any RON-reachable open/close signal. Recommended **hybrid**: keep the bool (frame-accurate, runtime-only, do not promote to schema), AND emit `GameEvent::Trigger("ui.panel_opened")` / `"ui.panel_closed"` from the executor's six panel sites so designers can react (pause AI, duck music, hints) in RON. No new Action/message type, no schema change, no CLI impact.

**How to apply:**
- When asked whether something should be data-driven, first classify: is it a *guard on emission* (→ Rust is fine) or a *response to an event* (→ must be RON-reachable)? Don't flag emission guards as philosophy violations.
- Reject "route an orthogonal boolean condition through `LogicState`/`EnterState`": `LogicState` (runtime/scene_manager/mod.rs) is a SINGLE scalar string and there is NO `ExitState` (clear via `EnterState("")`). So `EnterState("panel_open")` clobbers the current gameplay state (`"playing"`, `"hp_low"`) with no way to restore it. It is a mutually-exclusive mode-slot, not an orthogonal-flag set. Conditions that overlap every gameplay state must NOT go through it.
- Reject "guard reads its own triggering GameEvent": executor emits in Update, interpreter reads next frame, and `collectible_system` is in FixedUpdate — no same-frame ordering guarantee, plus Message double-buffering. Events are for designers *reacting*, never for the frame-accurate guard itself.
- For multi-site state writes (the six panel_open sites), recommend a single shared helper that does both the bool write and the correct event emit, so they cannot drift (same discipline as the EffectDef/LayerDef sync rule). Watch `ToggleInventory` — it must emit opened-or-closed based on resulting state, not unconditionally.
- Defer a general interpreter condition-flag system unless multiple orthogonal guards (panel-open, in-dialogue, cutscene) actually accumulate. CLAUDE.md explicitly says don't add a general condition system while the EnterState pattern suffices. See [[npc_state_machine_design]] for the runtime-only-state precedent and [[event_pipeline_intent_layer]] for the executor-event family.
