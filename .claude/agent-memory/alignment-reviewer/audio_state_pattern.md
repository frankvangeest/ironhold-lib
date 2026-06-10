---
name: audio-state-pattern
description: How project-level audio (AudioConfig + AudioState + SetVolume/ToggleMute) is wired data-driven across schema, project_loader, executor, and a change-detection system; the dual-write redundancy footgun
metadata:
  type: project
---

The 2026-06-10 mute/master-volume review established the reference shape for project-level runtime tuning state that is seeded from `ProjectConfig` and mutated by Actions. Mirrors [[particle_quality_budget_pattern]] Pattern A but with a config-seeded resource (not a `Default`-only one).

## Touchpoints (all must line up)

1. **`schema/project.rs`** — `AudioConfig { max_volume, mute_on_start }` with `#[serde(deny_unknown_fields)]`, `#[serde(default)]` per field via `default_*` fns, AND a hand-written `Default` impl whose values match the `default_*` fns. Added to `ProjectConfig` as `#[serde(default) audio: AudioConfig` (non-Option — the struct's own Default covers omission). `*.project.ron` is schema_version 3; no version bump needed for an additive `#[serde(default)]` field.
2. **`schema/actions.rs`** — tuple `SetVolume(u8)` (0–100 percent) + unit `ToggleMute`. Doc-comments state the scaling-against-`max_volume` semantics and which pipeline event each emits.
3. **`runtime/scene_manager/mod.rs`** — `#[derive(Resource)] AudioState { max_volume, active_fraction, muted }` with `effective_volume()` = `if muted {0} else {(active_fraction*max_volume).clamp(0,1)}`. `audio_state` is a field on the executor's `SceneStateMut` SystemParam bundle (ResMut).
4. **`runtime/scene_manager/project_loader.rs`** — `AudioState` inserted from `config.audio` at BOTH project-load phases (there are two `insert_resource(AudioState{...})` sites — phase 1 ~line 118 and phase 2 ~line 280). Forgetting the second site would leave `mute_on_start`/`max_volume` unapplied on one load path.
5. **`runtime/scene_manager/action_executor.rs`** — `SetVolume`/`ToggleMute` arms mutate `scene_state.audio_state`, then ALSO write `GlobalVolume` directly, then emit `GameEvent::Trigger("audio.volume_changed"|"audio.muted"|"audio.unmuted")`. **`SyncAudioState`** (unit Action) only READS `audio_state.muted` and re-emits `audio.muted`/`audio.unmuted` without mutating anything — used to seed bound labels on first state entry before any toggle has fired.
6. **`lib.rs`** — `init_resource::<AudioState>()` (replaced at load by project_loader) + `audio_state_system.before(message_interpreter_system)`. Ordering matters: `mute_on_start` must apply before `PlayMusicLoop` fires in the same frame.

## Label text lives in RON, NOT Rust (2026-06-10 fix)

`audio_state_system` USED to write `"Muted"`/`"Sound On"` strings to `GameVariables["audio_state"]` directly — a philosophy violation (presentation text hardcoded in Rust). As of 2026-06-10 it does NOT: it is purely the `GlobalVolume` writer. The label mapping now lives in `state_machine.ron`'s `global_on` bridge: `audio.muted`→`SetVariable("audio_state","Muted")`, `audio.unmuted`→`SetVariable("audio_state","Sound On")`. Designers change label text in one RON line, zero recompile. The split is the correct pattern: **Rust owns the semantic fact (muted?), RON owns the presentation (what word)**. The event strings `audio.muted`/`audio.unmuted`/`audio.volume_changed` are semantic identifiers (the designer's binding contract) and correctly stay in Rust — do NOT flag those as hardcoded. 3rd_person_game_demo puts `SyncAudioState` in `entry_actions` of menu/options/playing so the label is seeded on first entry (the `global_on` bridge alone only fires on a transition, never on initial load).

## Designer reachability — fully data-driven, confirmed

- `max_volume` / `mute_on_start` set in `*.project.ron` `audio:` block.
- `SetVolume(0..100)` and `ToggleMute` reachable from rules.ron, state_machine.ron, behavior files, UI buttons (`ui.button_pressed:toggle_mute`). 3rd_person_game_demo wires both via the options scene + state_machine.
- `audio.muted` / `audio.unmuted` / `audio.volume_changed` events let designers chain follow-on actions (e.g. swap a mute-button label) with zero Rust.
- All three layers documented in docs/20_data_formats.md and docs/30_runtime_events_and_logic.md.

## Footgun: dual write to GlobalVolume

The executor writes `GlobalVolume` directly AND mutating `scene_state.audio_state` (a ResMut) trips `is_changed()`, so `audio_state_system` writes `GlobalVolume` AGAIN next frame. Benign today (same value, idempotent) but it is redundant and a latent change-detection-churn smell. If anyone adds a third writer or makes `GlobalVolume` writes expensive, prefer ONE source of truth: mutate only `AudioState` in the executor and let `audio_state_system` be the sole `GlobalVolume` writer. Flag this if a future change touches either site. Not a blocker — the system is `is_changed()`-guarded so it does not churn every frame, only the frame after a volume action.
