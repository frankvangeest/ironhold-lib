---
name: real-pause-pattern
description: Plan-review facts for real_pause.md (PauseGame/ResumeGame/TogglePause, Time<Virtual> freeze) — what auto-freezes vs needs a gate, sibling-state pause re-runs entry_actions, overlay key bindings never merge
metadata:
  type: project
---

Reviewed `planning/features/real_pause.md` (plan dated 2026-09-15) at `d2191ab` on 2026-10-13. Verdict: Needs work.

**Verified facts (re-check before relying on them):**
- `Time<Virtual>` pause stops the whole `FixedUpdate` chain (`lib.rs` ~341: gamepad_bind, input_translator,
  player_movement, collectible, trigger_zone, npc_behavior, motion) plus every `Res<Time>` Update system
  (tick_delayed_events, stat regen/modifiers, despawn timers, particles, camera orbit/party, shake/blend).
  So `collectible_system` needs no gate (plan listed it wrongly).
- NOT dt-scaled, so they keep acting while paused: `first_person_camera_system` (mouse yaw rotates the
  **player Transform** — a gameplay mutation), flycam mouse look (cosmetic). Edge-triggered Update systems
  needing a gate: interactable, click_select, tab_targeting, action_bar_input (keyboard+gamepad+click).
- `panels_open` gates only collectible/interactable/targeting; action_bar is NOT gated by it, and
  `3rd_person_game_demo` slot "i" fires ToggleInventory — folding panels_open into a shared condition
  that also covers action_bar would make the inventory unclosable by key.
- Overlay loads never touch `LoadedKeyBindings` (`scene_loader.rs` rebuild is in the Replace branch only),
  so an overlay's `scene_key_bindings` are dead data; no validate warning exists.
- FSM sibling-state pause (`playing`<->`paused`) re-runs `playing.entry_actions` on every unpause: both
  shipped projects reset HP (3rd_person: `SetStat player_health 100`) / score (primitive_world) and restart
  music on resume. Real pause does not fix this; "pause as `on:` bindings, not a state" avoids it.
- No engine code special-cases a "paused"/"game_over" state name. Executor-emitted state events have
  precedent (`audio.muted`/`unmuted`, `nameplate.own_shown`).

Related: [[intent-event-layer-pattern]], [[gamepad-hot-join-pattern]], [[rules-vs-state-machine-coexistence]],
[[fixed-timestep-det-math-pattern]].
