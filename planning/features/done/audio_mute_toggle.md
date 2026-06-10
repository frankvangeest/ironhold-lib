# Feature: Mute Audio Toggle + Master Volume

_Status: Ready_
_Planned at: `c24c256` (2026-06-10)_

## What

Adds a project-level master volume ceiling (`max_volume`) and a mute toggle to `ProjectConfig`.
Designers can cap overall game volume without touching individual audio source files, and wire a
mute button from RON without writing any code.

## Why

Games often need their audio pre-balanced relative to each other (via per-asset `volume` in
`assets.ron`) but still need a global ceiling — e.g. a project tuned for a quiet ambient
experience should cap at 70 %, not blast at full. Without `max_volume`, the only option is to
re-export every audio file or adjust every `assets.ron` entry. The mute toggle is the minimum
viable player control every game needs.

## Approach

### New schema — `AudioConfig` on `ProjectConfig`

```ron
// ProjectConfig
audio: (
    max_volume: 0.8,      // project ceiling; 0.0–1.0; default 1.0
    mute_on_start: false, // default false
)
```

`AudioConfig` is a new `#[derive(Serialize, Deserialize)]` struct with `#[serde(default)]` on
both fields so existing projects that omit the block get sensible defaults.

### New resource — `AudioState`

```rust
pub struct AudioState {
    pub max_volume: f32,       // from ProjectConfig at load time
    pub active_fraction: f32,  // set by SetVolume; default 1.0
    pub muted: bool,
}
```

Actual `GlobalVolume` = `muted ? 0.0 : active_fraction * max_volume`.

### New actions

| Action | Behaviour |
|---|---|
| `ToggleMute` | Flips `AudioState.muted`; updates `GlobalVolume`; emits `audio.muted` or `audio.unmuted` |
| `SetVolume(f32)` | Sets `AudioState.active_fraction` (clamped 0–1); updates `GlobalVolume`; emits `audio.volume_changed` |

`SetVolume` scales against `max_volume`, so `SetVolume(1.0)` always means "loudest the designer
allows" regardless of the project ceiling.

### New system — `audio_state_system`

Runs in `Update`. Watches for `AudioState` changes (via `ResMut` changed detection) and writes
the computed value to `GlobalVolume`. Also applies `mute_on_start` once on `scene.ready`.

### Demo

Wire a mute toggle `Button` in `3rd_person_game_demo` that fires `ToggleMute`.

## Tasks

- [ ] `AudioConfig` struct in `schema/project_config.rs`; add `audio: AudioConfig` field to `ProjectConfig`
- [ ] `AudioState` resource in `runtime/audio_state.rs`; initialized from `AudioConfig` on project load
- [ ] `Action::ToggleMute` and `Action::SetVolume(f32)` variants
- [ ] `audio_state_system` — change-detect `AudioState`, write to `GlobalVolume`; apply `mute_on_start` on ready
- [ ] Hook both actions into `action_executor_system`
- [ ] Mute toggle button in `3rd_person_game_demo` scene RON
- [ ] Emit `audio.muted` / `audio.unmuted` from `ToggleMute`; emit `audio.volume_changed` from `SetVolume`
- [ ] Integration test: `ToggleMute` flips muted state; `SetVolume` respects `max_volume` ceiling; pipeline events fire correctly
- [ ] Docs: `docs/20_data_formats.md` (`AudioConfig` fields); `docs/30_runtime_events_and_logic.md` (new actions)

## Pipeline events

| Event | When |
|---|---|
| `audio.muted` | Emitted by `ToggleMute` when transitioning to muted |
| `audio.unmuted` | Emitted by `ToggleMute` when transitioning to unmuted |
| `audio.volume_changed` | Emitted by `SetVolume` after the fraction is updated |

Designers can hook these to swap a mute button icon or show/hide a volume indicator entirely from RON rules, with no code changes.

## Open questions

_(none)_

## Acceptance criteria

- Given `max_volume: 0.8` in project config, when the game starts unmuted, `GlobalVolume` = 0.8.
- Given `mute_on_start: true`, when the scene is ready, `GlobalVolume` = 0.0.
- Given `ToggleMute` fired while unmuted, `GlobalVolume` becomes 0.0.
- Given `ToggleMute` fired while muted, `GlobalVolume` restores to `active_fraction * max_volume`.
- Given `SetVolume(0.5)` with `max_volume: 0.8`, `GlobalVolume` = 0.4.
- Given a project with no `audio:` block, behavior is identical to `max_volume: 1.0, mute_on_start: false`.
- Given `ToggleMute` fired while unmuted, the `audio.muted` pipeline event is emitted.
- Given `ToggleMute` fired while muted, the `audio.unmuted` pipeline event is emitted.
- Given `SetVolume(0.5)` fired, the `audio.volume_changed` pipeline event is emitted.
