# Feature: Live volume percent readout (`audio_volume_percent`)

_Status: Draft_
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
  (backlog entry, 2026-10-06) but needs a rule per possible value, a new event family, and CLI
  event-check changes — and still cannot show the initial value without a `SyncAudioState` call.

Engine-written `GameVariables` already have precedent for exactly this shape: the targeting capability
writes `target_display` / `target_name` / `target_id` on every change and `docs/20_data_formats.md`
documents them under "GameVariables auto-written by capabilities" ("bind a `Label` to these — no rule
wiring needed"). `audio_volume_percent` joins that table.

## Approach

No schema change, no new `Action`, no new event, no CLI change.

**One new system**, next to `audio_state_system` (`runtime/scene_manager/mod.rs`, registered in
`lib.rs` right beside it):

```rust
/// Mirrors `AudioState.active_fraction` into `GameVariables["audio_volume_percent"]` ...
pub fn audio_volume_var_system(audio_state: Res<AudioState>, mut vars: ResMut<GameVariables>) {
    if !audio_state.is_changed() { return; }
    vars.0.insert("audio_volume_percent".into(), volume_percent_string(audio_state.active_fraction));
}
```

- Runs whenever `AudioState` changes. `AudioState` is inserted at project load
  (`project_loader.rs`, two sites), and an inserted resource counts as changed, so the variable exists
  from the first frame; `SetVolume` mutates `active_fraction` (change detected); nothing else clears
  `GameVariables`, so scene changes cannot lose it (verified: no `game_vars.0.clear()` anywhere).
- **Value:** the *chosen preset*, `round(active_fraction * 100)` clamped to `0..=100`, as an integer
  string (`"100"`, `"25"`). It is NOT the effective volume: `SetVolume(100)` equals the project's
  `max_volume` ceiling (`docs/20`: "scales against `max_volume`"), so the player-facing number is the
  slider-style percent they picked. Muting does not change it — the existing `audio_state` mirror
  (`"Muted"`/`"Sound On"`) already covers mute, and showing both is the useful UI ("Volume: 50% — Muted").
- Ordered `.before(fsm_interpreter_system)` alongside `audio_state_system` so a rule reading it the same
  frame sees the new value.
- A small pure helper `volume_percent_string(f32) -> String` (unit-testable: 0.0 -> "0", 0.25 -> "25",
  1.0 -> "100", 0.555 -> "56", NaN/negative/over-1 clamped) keeps the formatting out of the system body.

**RON / designer surface:** the options scene's `audio_heading` label becomes
`Label((id: "audio_heading", text: "", bind: "audio_volume_percent", format: "Volume: {}%", ...))`.
The preset buttons are unchanged. The `3rd_person_game_demo` `state_machine.ron` is **unchanged**.

**Backwards compatibility:** a new variable key; a project that never binds it is unaffected. A project
that already used the key `audio_volume_percent` for its own purposes would be overwritten when
`AudioState` changes — the same reserved-key caveat as `target_*`/`score`; documented in the table.

## Tasks
- [ ] `volume_percent_string` + `audio_volume_var_system` (`runtime/scene_manager/mod.rs`), exported and
      registered in `lib.rs` next to `audio_state_system`
- [ ] Tests (`tests/audio_tests.rs`): variable present after project load with the default 100; present
      and equal to `mute_on_start` projects' fraction (muted does not change it); `SetVolume(25)` ->
      `"25"`; `SetVolume(200)` clamps to `"100"`; `ToggleMute` leaves it unchanged; value survives a
      `LoadScene`; pure-helper table test (rounding, clamping, NaN)
- [ ] `3rd_person_game_demo/scenes/options.scene.ron`: bind the Volume heading to the variable; keep
      the section fitting 720px; refresh only `3rd_person_game_demo_options.png` (`--real-gpu`)
- [ ] Docs: add the `audio_volume_percent` row to "GameVariables auto-written by capabilities" in
      `docs/20_data_formats.md` and a one-line mention next to `SetVolume`/`SyncAudioState` in the
      action tables (`docs/20`, `docs/30`); note that `ironhold validate` does not check `bind:` keys
- [ ] Backlog: replace the event-based "Show the live volume percent" item with this plan's Done entry
- [ ] Code-change workflow steps 4-10 (reviews, full test loop, WASM dev build, playtest, merge)

## Open questions
- Key name: `audio_volume_percent` (chosen: matches `audio_state`/`audio_muted` prefix and says what the
  number is). Alternative `volume_percent`/`audio_volume` — any objection before it becomes a reserved
  key?
- Should the same system also mirror `audio_muted`/`audio_state` so the existing `global_on` bridge in
  `state_machine.ron` becomes unnecessary? **Out of scope here** (it would change an existing, working
  convention and the strings `"Muted"`/`"Sound On"` are designer-authored); log as a follow-up if wanted.
- Reserved-key policy: is a docs-table entry enough, or should `ironhold validate` warn when a
  `SetVariable` targets an engine-written key? (Not done for `target_*`/`score` today.)

## Acceptance criteria
- Given a project with the default audio config, when the options scene shows a `Label` bound to
  `audio_volume_percent`, then it reads `Volume: 100%` on first view (no button pressed, no rule authored).
- Given the player presses the `50%` preset, when the options scene is open, then the label reads
  `Volume: 50%`; after `Back` and returning to Options it still reads `Volume: 50%`.
- Given the game is muted, when the volume preset is 75, then `audio_volume_percent` is still `"75"`.
- Given `SetVolume(200)`, then the variable reads `"100"`, matching the clamped `active_fraction`.
- Given a project with no label bound to it, then nothing about its behaviour changes.
