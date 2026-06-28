---
name: audio-mute-state-machine
description: How mute/volume audio state flows through the pipeline; single source of truth is AudioState.muted; indicator label can never legitimately desync from real state
metadata:
  type: project
---

Audio mute/volume is data-driven through the standard pipeline with ONE source of truth.

**Source of truth:** `AudioState` resource (in `runtime/scene_manager/mod.rs`): `{ max_volume, active_fraction, muted }`. `effective_volume() = muted ? 0.0 : active_fraction*max_volume`. Initialized from `ProjectConfig.audio` (mute_on_start, max_volume) at project load.

**Executor (`action_executor.rs`):**
- `ToggleMute` flips `audio_state.muted`, writes `GlobalVolume`, updates live `bg_music_query` sinks, then emits GameEvent `audio.muted` / `audio.unmuted`.
- `SetVolume(pct)` sets active_fraction, same sink update, emits `audio.volume_changed`.
- `SyncAudioState` re-emits muted/unmuted WITHOUT changing state — use in entry_actions so bound labels are correct on first load / scene nav.

**Indicator label:** A `Label` with `bind: "audio_state"`. The `audio_state` GameVariable is ONLY written by `global_on` rules reacting to `audio.muted`/`audio.unmuted`. So the label is a pure READ projection of real state — it CANNOT legitimately get ahead of the action. `update_dynamic_labels_system` (lib.rs) reads GameVariables every frame.

**Trigger string mapping:** scene Button `action: "ui.toggle_mute"` → scene_loader strips `"ui."` prefix → `UiAction::Trigger("toggle_mute")` → interpreter formats `ui.button_pressed:toggle_mute`. So `action: "ui.foo"` matches FSM rule `ui.button_pressed:foo`. The `ui.` prefix is mandatory in scene RON and stripped; do NOT author `action: "toggle_mute"` (would become event `...:toggle_mute` only if no prefix — actually strip_prefix unwrap_or keeps full string, so missing `ui.` → trigger `toggle_mute`... no: unwrap_or returns original, giving trigger="toggle_mute" anyway only if it had no ui. — but THEN event is button_pressed:toggle_mute too. The hazard is a DOUBLE prefix like action:"ui.ui.x").

**Architectural rule for any reactive indicator:** the visual must be SET by the executor-emitted result event (audio.muted), never by the button press itself. Button press → pipeline → executor → result event → SetVariable → label. Never let the button click directly SetVariable the indicator (that decouples visual from truth).
