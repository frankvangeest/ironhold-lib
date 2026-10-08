# Feature: Live volume percent readout (`audio_volume_percent`)

_Status: Done (2026-10-07) — shipped on `feature/audio_volume_readout`; playtest confirmed by Frank_
_Planned at: `e847b93` (2026-10-06)_

## What

The engine publishes the current volume as a `GameVariables` entry, `audio_volume_percent` (e.g.
`"75"`), kept in sync automatically. A designer binds a `Label` to it
(`bind: "audio_volume_percent", format: "Volume: {}%"`) and the options screen shows which volume
preset is active — from the very first frame, with no `state_machine.ron` rules and no initial
value to seed. The `3rd_person_game_demo` options screen gets the readout.

## Why

The options menu has four volume preset buttons (`SetVolume(25/50/75/100)`) but nothing says which is
active (raised by Frank during the flex `Group` playtest, 2026-10-06). RON alone cannot do it:

- a bound `Label` renders **nothing** until its variable has been set (`update_dynamic_labels_system`
  uses `unwrap_or("")`), and RON has no hook to seed a variable at project start;
- seeding at start-menu entry (`SetVariable` in the `menu` state's `entry_actions`) would **reset**
  the display to 100 every time the player returns from Options after choosing 50;
- an event-based mirror (`audio.volume:{pct}` + one `global_on` rule per value) was the first idea
  (backlog entry, 2026-10-06) but needs a rule per possible value (up to 101 for a `u8`), a new event
  family, and CLI event-check changes — and still cannot show the initial value without a
  `SyncAudioState` call.

Engine-written `GameVariables` already have precedent for exactly this shape: the targeting capability
writes `target_display` / `target_name` / `target_id` on every change and `docs/20_data_formats.md`
documents them under "GameVariables auto-written by capabilities" ("bind a `Label` to these — no rule
wiring needed"). `audio_volume_percent` joins that table. (`score` is NOT a precedent: designers write
it with `IncrementVariable`; the engine only reads it for `DebugState`. The docs table lists it as
"auto-written", a pre-existing inaccuracy to correct in the same docs pass — R8.)

## Resolutions from the plan reviews

| # | Resolution | Source |
|---|---|---|
| R1 | **Ordering corrected.** The FSM interpreter never reads `GameVariables` (readers: `update_dynamic_labels_system`, `icon_button_sync_system`, dialogue conditions, `DebugState.score`), so the planned `.before(fsm_interpreter_system)` rationale ("a rule reading it the same frame sees it") was false and would add a frame of lag: `SetVolume` mutates `AudioState` inside `action_executor_system`, which runs *after* the interpreter. Register the mirror `.after(action_executor_system).before(update_dynamic_labels_system)` so the label updates the same frame and a one-`app.update()` test is deterministic. `audio_state_system` itself stays where it is (it must precede `PlayMusicLoop` for `mute_on_start`). | architect (blocking) |
| R2 | Helper `volume_percent_string(f32)`: `if !f.is_finite() { 0 }` explicit, then `(f * 100.0).round().clamp(0.0, 100.0) as u32`. **Round, not truncate**: `29/100` in f32 is `0.28999999`, `*100` truncates to 28. | architect |
| R3 | Skip the `vars.0.insert` when the stored value already equals the new one (so `ToggleMute`, which changes only `muted`, does not touch `GameVariables`' change ticks). Cosmetic but free. | architect |
| R4 | Key name `audio_volume_percent` confirmed by both reviewers; per-channel siblings (queued "Audio channels" item) can later be `audio_volume_percent.music` etc. Reserved-key behaviour is **intermittent**, not "always overwritten": the engine writes only when `AudioState` changes, so a project's own `SetVariable("audio_volume_percent", ..)` survives until the next `SetVolume`/`ToggleMute`/project load. Docs say: "Reserved — the engine overwrites it whenever the audio state changes; don't `SetVariable` it." Policy: docs-only now; a `validate` warning for any `SetVariable`/`IncrementVariable` on an engine-written key (`target_*`, `audio_volume_percent`) is ONE suggestion in `claude_suggestions.md`. | both |
| R5 | Mute stays on its existing RON `global_on` bridge (`audio.muted`/`audio.unmuted` -> `audio_state`/`audio_muted`); an engine-written mute key is out of scope, but logged in `claude_suggestions.md` so the two-conventions state does not become permanent (and so a future key name avoids colliding with the demo's own `audio_muted`). The docs say explicitly that mute is NOT auto-written. | both |
| R6 | "Volume: 50% - Muted" means **two labels** (a `Label` binds one key with one `{}`): the bound heading plus the existing `options_audio_state` label. Use an ASCII hyphen in any such string (the engine font has no em-dash glyph). A `mute_on_start: true` project correctly reads "Volume: 100%" next to "Audio: Muted". | UX |
| R7 | Docs reach beyond the auto-written table: (a) `AudioConfig` section of `docs/20` (~4660-4687) gets a "GameVariables" line under its "Pipeline events" table, since that is where a designer looking for audio settings lands; (b) the `SyncAudioState` row says it is not needed for `audio_volume_percent`; (c) the `Label` `bind` row documents "shows nothing until the key has been set; `ironhold validate` does not check bind keys"; (d) `docs/30` next to the audio events (~123), `docs/STATUS.md:98`, and the existing `SetVolume(u32)` vs `u8` inconsistency at `docs/30:366`. | UX |
| R8 | `docs/20` auto-written table: add the `audio_volume_percent` row, and correct the `score` row (designer-written, engine-read). The options scene gets an "audio_volume_percent is written by the engine - no rule needed" comment above the bound heading (mirrors the `main.scene.ron:330` "audio_muted is not written automatically" comment). | UX + architect |
| R9 | Follow-ups to log (not built here): bound `Label` falling back to its `text:` until its key exists (removes the blank-until-set footgun for every bound label); a highlighted/"selected" preset button (Button has no selected binding); the reserved-key `validate` warning; the engine-written mute key (R5). | UX |

## Approach

No schema change, no new `Action`, no new event, no CLI change.

**One new system**, in `runtime/scene_manager/mod.rs` next to `audio_state_system`, registered in
`lib.rs` per R1:

```rust
/// Mirrors `AudioState.active_fraction` into `GameVariables["audio_volume_percent"]` ...
pub fn audio_volume_var_system(audio_state: Res<AudioState>, mut vars: ResMut<GameVariables>) {
    if !audio_state.is_changed() { return; }
    let value = volume_percent_string(audio_state.active_fraction);
    if vars.0.get(AUDIO_VOLUME_PERCENT_KEY) != Some(&value) {
        vars.0.insert(AUDIO_VOLUME_PERCENT_KEY.to_string(), value);
    }
}
```

- Runs whenever `AudioState` changes. A resource insert counts as changed in every path (bevy_ecs 0.18
  `ResourceData::insert` sets `changed_ticks` unconditionally, on first insert and on replacement):
  `AudioState` is `init_resource`d at `lib.rs:171` (variable present from frame 1) and re-inserted at
  project load (`project_loader.rs:141`/`:335`); `SetVolume` mutates `active_fraction`; nothing clears
  `GameVariables` on scene change (verified: no `game_vars.0.clear()`; `LoadScene` only calls
  `clear_target_vars`), so scene changes cannot lose it. `SyncAudioState` only reads and does not mark
  the resource changed — not needed.
- **Value:** the *chosen preset*, `round(active_fraction * 100)` clamped to `0..=100`, as an integer
  string (`"100"`, `"25"`). It is NOT the effective volume: `SetVolume(100)` equals the project's
  `max_volume` ceiling, so the player-facing number is the percent they picked. Muting does not change it.
- `GameVariables` is `// det: lookup-only` and the insert names no hash type, so `determinism_lint` is
  unaffected; the system only does work on change (negligible WASM cost).

**RON / designer surface:** the options scene's `audio_heading` label becomes
`Label((id: "audio_heading", text: "", bind: "audio_volume_percent", format: "Volume: {}%", ...))`
(12 characters, ~160px at 26px in the existing 596px box; `clip` stays `false`, section height
unchanged). The preset buttons are unchanged. `state_machine.ron` is **unchanged**.

## Tasks
- [x] `AUDIO_VOLUME_PERCENT_KEY`, `volume_percent_string`, `audio_volume_var_system`
      (`runtime/scene_manager/mod.rs`), re-exported and registered in `lib.rs` per R1
- [x] Tests in `tests/audio_tests.rs` (use `scene_lifecycle_tests.rs:16-28`'s mocked-`ProjectConfig`
      pattern for the project-load case): variable present after load with `"100"`; `mute_on_start: true`
      still `"100"` with `muted == true`; one `SetVolume(25)` + one `app.update()` -> `"25"` (R1 makes this
      deterministic); `SetVolume(200)` clamps to `"100"`; `ToggleMute` leaves it unchanged; survives a
      `LoadScene`; helper tests: round-trip `volume_percent_string(p as f32 / 100.0) == p.to_string()` for
      every `p in 0..=100` (pins round-vs-truncate), NaN/inf/negative/over-1 clamp
- [x] `3rd_person_game_demo/scenes/options.scene.ron`: bound heading + the R8 comment; refresh only
      `3rd_person_game_demo_options.png` (`--real-gpu`, existing procedure)
- [x] Docs per R7/R8 (`docs/20`, `docs/30`, `docs/STATUS.md`)
- [x] `planning/claude_suggestions.md`: R5/R9 follow-ups; `planning/backlog.md`: replace the event-based
      "Show the live volume percent" item with this feature's Active/Done entry
- [ ] Code-change workflow steps 4-10 (reviews, full test loop, WASM dev build, playtest, merge)

## Acceptance criteria
- Given a project with the default audio config, when the options scene shows a `Label` bound to
  `audio_volume_percent`, then it reads `Volume: 100%` on first view (no button pressed, no rule authored).
- Given the player presses the `50%` preset, when the options scene is open, then the label reads
  `Volume: 50%` within the same frame's render; after `Back` and returning to Options it still reads `Volume: 50%`.
- Given the game is muted, when the volume preset is 75, then `audio_volume_percent` is still `"75"`.
- Given `SetVolume(200)`, then the variable reads `"100"`, matching the clamped `active_fraction`.
- Given a project with no label bound to it, then nothing about its behaviour changes.

## Notes moved from `crates/ironhold_core/src/CLAUDE.md` (2026-10-08, core CLAUDE.md split)

These paragraphs were in the crate `CLAUDE.md` and are kept here verbatim; that file now carries only the condensed current-state rule. Wording such as "above"/"below" refers to the old file.

### b:1060: `audio_volume_var_system` mirrors `AudioState` to a `GameVariable`
<!-- moved-from-claude-md: b:1060 -->

### Live volume variable (`audio_volume_var_system`)
`audio_volume_var_system` mirrors `AudioState.active_fraction` into `GameVariables[AUDIO_VOLUME_PERCENT_KEY]` (`"audio_volume_percent"`, the chosen preset as an integer string, not the effective volume) whenever `AudioState` changes, so a bound `Label` shows the live volume with no RON rules — same shape as the targeting `target_*` variables. It is scheduled `.after(action_executor_system).before(update_dynamic_labels_system)` (the FSM interpreter never reads `GameVariables`, so `.before(fsm_interpreter_system)` would only add a frame of lag). Mute is deliberately NOT mirrored here: it still goes through the `audio.muted`/`audio.unmuted` RON `global_on` bridge. Tests that stand in for the project loader's `AudioState` re-insert must insert a non-default `active_fraction`, or they pass vacuously (the test harness never completes a project load).
