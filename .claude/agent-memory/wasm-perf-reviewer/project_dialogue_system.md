---
name: project-dialogue-system
description: dialogue_tick_system (Update) per-frame cost, ActiveDialogue resource, choice-button UI spawn on node transition, DialogueDef asset loading
metadata:
  type: project
---

Dialogue system: `crates/ironhold_core/src/capabilities/dialogue.rs` + `schema/dialogue.rs`.

`dialogue_tick_system` runs every Update, gated `.after(button_system).after(interactable_system).before(message_interpreter_system)`.

**Why:** new conversation capability; UI-only (no mesh/material/GPU resources).

**How to apply (hot-path facts):**
- Idle path (`!active.is_active()`, ~99% of frames): two `MessageReader::read().filter_map().collect::<Vec>()` calls. Empty iterators -> `Vec` does NOT allocate (no heap traffic when no events). Then iterates panel_q (1 entity) with change-detection guard on Visibility, and a nested loop over interacted_ids(0) x entity_dialogue_q. Effectively free. PASS.
- Active path: node render is gated by `last_rendered_node != Some(idx)` — runs ONLY on node transition, NOT per-frame. Per-frame while active = just auto_advance timer decrement + cheap guarded text/visibility checks. PASS.
- Node-transition render does String allocs (.replace {self}, format! for trigger/Name) and UI entity spawn/despawn via Commands. Bounded by visible choice count (~2-5). Per-transition, not per-frame. Fine.

**ActiveDialogue resource**: same resource exposed two ways — `init_resource::<ActiveDialogue>()` in lib.rs:129, AND borrowed as `ResMut<'w, ActiveDialogue>` inside the scene_manager SystemParam bundle (`mod.rs:428` field `active_dialogue`). Executor writes via `scene_state.active_dialogue.*`; tick system reads via `ResMut<ActiveDialogue>`. Same store — verified, NOT a split-state bug.

**Asset loading**: `Action::StartDialogue` (executor) calls `asset_server.load::<DialogueDef>()` on demand — NO preload mechanism (unlike GLB/audio). DialogueDef RON is 1-10 KB so first-open HTTP fetch on WASM is a brief decode, not a multi-second GLB-class stall. Tick system handles the not-yet-loaded frame gracefully (`dialogue_assets.get(&handle)` returns None -> early return). Minor first-open latency only; acceptable. If designers report first-line lag, add an Action::PreloadDialogue mirroring PreloadGlb.

**Binary size**: zero new deps. DialogueDef reuses existing bevy_common_assets ImplicitRonPlugin + serde Deserialize. One new asset loader registration. Negligible (<<100 KB) wasm delta.

UI spawn note: choice buttons are UI nodes only (Button/Node/Text/BackgroundColor) — no new mesh+material pipeline variant, so no WebGPU createRenderPipeline stall class (unlike Action::Spawn / SpawnEffect).
