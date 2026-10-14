# Feature: Real Pause (v1 — simulation freeze)

_Status: Ready (plan-review 2026-10-14: system-architect, ux-gamedesigner-reviewer, alignment-reviewer; Frank's decisions recorded below)_
_Planned at: `d2191ab` (2026-10-14). First written at `2a034d6` (2026-09-15); rewritten after plan-review against current code._

## What

Make "paused" stop the game. Today `LoadSceneOverlay("scenes/pause.scene.ron")` draws a menu and nothing else — NPC AI,
physics, player movement, interact/loot, particles and delayed-event timers all keep running behind it. Add three unit-variant
actions a designer puts in a state's `entry_actions`/`exit_actions`, next to the overlay action they already use, plus the
engine pieces that make that safe (a single clock owner, input gates, auto-resume, events, validate checks) and a way for a
**joined gamepad player** to pause at all.

## Why

Reported by Frank playtesting `3rd_person_game_demo` (2026-09-15) and a stakeholder-list item in three reviewers' top-5
(Alignment #2, Game-World #3, UX #1): monsters kept patrolling and attacking, chests could still be looted, and the character
kept walking behind the pause overlay. It also produces a logged soft-lock (backlog Bugs: **Dying while paused never reaches
game over**) and the docs teach the broken pattern as canonical. Not project-specific — `primitive_world` has the same defect in
`paused` and (worse) `game_over`.

## Decisions (Frank, 2026-10-14)

1. **Authoring pattern (a): paired actions in the paused state.** `PauseGame` in the state's `entry_actions`, `ResumeGame` in
   the **same state's** `exit_actions`, beside `LoadSceneOverlay`/`UnloadOverlay`. `TogglePause` is for flat (`global_on`-only)
   projects and is documented as second-class. Rejected: pause-as-bindings (two toggles that can drift = highest soft-lock
   risk; state-scoped rules keep firing because `LogicState` never changes) and a `pauses_game` state flag (deferred as
   possible sugar, see Deferred).
2. **Fold the free-heal fix into this feature.** Today `paused → playing` re-runs `playing.entry_actions`, so every unpause resets
   health/mana/score and restarts the music in both demos. Run-start actions move to the `scene.ready:main` handlers of the states
   that start a run (see Demo changes). Real pause does not fix this by itself.
3. **Joined-gamepad pause is part of v1** (`global_claimed_gamepad_bindings`, see below).
4. **`PauseGame` is a request, not a guarantee** (multiplayer, see "Networking fit"): the engine emits `game.paused` /
   `game.resumed` only when the clock actually stops/starts, and docs teach reacting to those events.

## Phases (one branch, one commit per phase)

| Phase | Content | Status | Completed |
|---|---|---|---|
| 1 | `SimClock` + actions + gates + auto-resume + events + core tests | Queued | — |
| 2 | `global_claimed_gamepad_bindings` (+ loader, CLI, docs) | Queued | — |
| 3 | `validate` checks + overlay-binding `warn!`/validate + demo RON changes + docs | Queued | — |

Backlog: **Real pause** (Queued ▸ Engine/Runtime, `backlog.md:71`) closes with phase 3; also close the Bug **Dying while paused
never reaches game over** (`:21`) and the interim docs item "Add a 'the game keeps running' warning…" (Designer Experience).

## Approach

### 1. `SimClock` — the single clock owner (`runtime/sim_clock.rs`)

- `#[derive(Resource, Default)] pub struct SimClock { holds: BTreeSet<PauseHold> }`, `enum PauseHold { Menu, Static, FocusLoss,
  Session }` (`Ord`; **never a `HashSet`/counter** — determinism lint, and `panels_open` shows how counters leak). `Static` and
  `FocusLoss` are reserved for `static_scene_mode.md` and the focus-loss backlog item; `Session` is reserved for the network layer
  (Beta 0.6). Methods: `hold`, `release`, `toggle`, `is_running`.
- `apply_sim_clock_system` (`run_if(resource_changed::<SimClock>)`, ordered `.after(action_executor_system)`) is the **only**
  caller of `Time<Virtual>::pause()`/`unpause()` and the **only** emitter of `GameEvent` `game.paused` / `game.resumed` (emitted on an
  actual running↔held change, including auto-resume). Nothing else in the crate may call `Time<Virtual>::pause`/`set_relative_speed`
  or read `Time<Virtual>::is_paused()` / Bevy's `common_conditions::paused`; all gameplay reads `sim_running`.
- `pub fn sim_running(clock: Res<SimClock>) -> bool`, stable home `runtime/sim_clock.rs`.
- Why `Time<Virtual>` is the right lever (verified in `bevy_time-0.18.0`): `run_fixed_main_schedule` (`fixed.rs:243-258`) adds
  `Time<Virtual>::delta()` to the accumulator, so a paused clock runs **zero** fixed ticks and the whole `FixedUpdate` chain
  (`input_translator_system`, `player_movement_system`, `collectible_system`, `trigger_zone_system`, `npc_behavior_system`,
  `motion_system`, `gamepad_bind_system`, Rapier's `TimestepMode::Fixed` step) freezes with no per-system code. Unpausing is clean
  (`virt.rs:231-256` drops paused real time; the first frame after unpause advances one real frame, no catch-up burst), so
  `max_frame_delta_secs` needs nothing. `Update` systems on `Res<Time>` freeze too (delayed events, cooldowns, `DespawnTimer`,
  stat modifiers, particles, popups, fading lights/decals, camera shake/blend, orbit/party cameras, Bevy animation). Nothing in
  core reads `Time<Real>`. The pause takes effect the frame **after** `PauseGame` executes (the clock reads the hold in `First`);
  say so in docs and tests.
- `bevy_framepace` (native) paces on real time: rendering continues while paused. Lowering the rate while paused belongs to the
  focus-loss item.

### 2. Actions

`PauseGame`, `ResumeGame`, `TogglePause` — **unit variants** (not tuple variants), mirroring `OpenInventory`/`CloseInventory`/
`ToggleInventory`.

- Semantics: they touch **only the `Menu` hold**. `PauseGame`/`ResumeGame` are **no-ops if already in that state** (no nesting
  counter: pause → options-from-pause must not need two resumes). `TogglePause` flips the `Menu` hold.
- Executor arms reach `SimClock` through `SceneStateParams` (`mod.rs:670`): `action_executor_system` is at the 16-param limit.
- `Action::is_presentation_only` (`schema/actions.rs:449`, exhaustive): all three return **`false`** (they change how many
  simulation ticks run). Extend the test that pins the classification.
- Compiler-forced arms: `rewrite_self` / `rewrite_target` (`action_substitution.rs`) identity group; CLI `query.rs::action_kind`.
  Wildcard sites (`action_needs_target` `_ => false` at `action_bar.rs:442`, dialogue `substitute_self_in_action` `other => other`
  at `dialogue.rs:363`) work unchanged but get **explicit pass-through arms** (the `ui_set_visible` lesson). Do not take on the
  structural `{target}` fix (backlog `:43`).

### 3. Input gates (Update systems only)

Two **separate named conditions**, stacked with `.run_if(a).run_if(b)` (Bevy ANDs them): `sim_running` and `no_panels_open`.
**The three existing `panels_open > 0` early-returns are NOT merged into the pause condition** (plan change: merging would stop
the action bar from closing the inventory — slot "i" fires `ToggleInventory` directly — and `panels_open` is a counter with a
history of sticking above 0; pause must not inherit that).

| System | Where | Gates |
|---|---|---|
| `interactable_system` | `capabilities/interactable.rs:43` (panel early-return :52 stays) | `sim_running` + `no_panels_open` |
| `tab_targeting_system` | `targeting.rs:274` (early-return :286 stays) | `sim_running` + `no_panels_open` |
| `collectible_system` | `collectible.rs:33` | `FixedUpdate`: already frozen by the clock, **no new gate**; its `panels_open` check stays |
| `click_select_system` | `targeting.rs:190` | `sim_running` only |
| `action_bar_input_system` | `action_bar.rs:173` (registered :103) | `sim_running` only — **check inside the function** (or document) so its `Option<Ref<Interaction>>::is_changed()` baseline keeps advancing: a slot whose `Interaction` is still `Pressed` on the first frame after resume must not fire (test) |
| `first_person_camera_system` | `camera.rs:577` | `sim_running` — mouse yaw is not dt-scaled and rotates the **player's Transform** (blocking) |
| `fly_camera_system` | `flycam.rs:56` | `sim_running` — mouse look ignores frame time ("frozen tableau") |
| `dialogue_tick_system` | `dialogue.rs:85` | **choice-click branch only**, gated inside the function body (a system-level `run_if` would stop its panel upkeep) |

For any gated system that reads a `MessageReader<MouseMotion>`/similar, **early-return after draining the reader** (a `run_if` skip
leaves the cursor behind and the first frame after resume consumes stale input).

**Must NOT be gated:** `global_input_system` (`input.rs:42`; pause/resume keys), `unclaimed_gamepad_trigger_system` (`input.rs:94`),
`button_system`/`icon_button_click_system`, `target_auto_clear_system`, `action_bar_visual_system`, the interpreters
(`fsm_interpreter_system` → `entity_fsm_interpreter_system` → `flush_pending_intent_system` → `action_executor_system`), the scene
loader, audio, `ui_scale_system`, nameplates/labels, split-screen viewport systems and `pipeline_warmup_system` (counts frames).

Registration: `.run_if(sim_running)` per system at the sites in `lib.rs`, `TargetingPlugin` (the two systems, not the chained trio)
and `ActionBarPlugin`. None of these add ordering edges, so D3 is unaffected; **D3 gets a task** (see Docs): a separate
`PlayerInputSet` with no ordering edges and `configure_sets(Update, PlayerInputSet.run_if(sim_running))`, not a condition on
`GameStreamSet::Targeting`/`::Interact` (that would also gate `target_auto_clear_system`).

### 4. Auto-resume on a Replace `LoadScene`

Policy: a Replace-mode scene load releases the `Menu` hold (and **only** `Menu`: never `Static`/`FocusLoss`/`Session`).

- **Where:** `spawn_scene_v2`'s Replace branch (`scene_loader.rs:125`, beside `spawn_registry.entities.clear()` and the
  `LevelEntity` sweep at :173) — not the executor's `Action::LoadScene` arm (`action_executor.rs:42-70`): no gameplay system checks
  `AppState`, so the old world keeps simulating for the whole `LoadingScene` window and resuming at the arm would un-freeze a dying
  `game_over` world. Add `ResMut<SimClock>` to `SceneV2Params` (`mod.rs:645`; `spawn_scene_v2` is at the 16-param limit).
- **Overlay loads never resume** (`LoadSceneOverlay`/`ToggleOverlay`/`UnloadOverlay` do not touch `SimClock`; the overlay branch at
  `scene_loader.rs:121-124` stays clock-free). Not driven from `SceneEvent::Loaded` (written for overlay and Replace alike, `:87`).
- Designer-visible: `info!` log, `game.resumed` event, documented in the `LoadScene` doc comment, docs/20 and docs/30. FIFO
  consequence: `[LoadScene(x), PauseGame]` deliberately starts the new scene paused. The FSM stays in `paused` until a
  `scene.ready` transition fires while the clock is already running — documented.

### 5. `global_claimed_gamepad_bindings` (phase 2)

Today unclaimed-pad bindings never fire for a claimed pad, so no gamepad player can pause; the slot-based workaround
(`gamepad_key: "Start"`, `do_actions: [TogglePause]`) can pause but never un-pause because the action bar is gated.

- New `ProjectConfig.global_claimed_gamepad_bindings: BTreeMap<String, String>` (button name → trigger; `BTreeMap` for the
  determinism lint), named after the existing `*_unclaimed_*` maps. Fires `UiEvent::ButtonPressed` (`ui.button_pressed:<trigger>`),
  so it is covered by `unreachable_trigger`.
- Evaluated inside `unclaimed_gamepad_trigger_system` against the **same `claimed` set** built at `input.rs:119-145`: a pad is claimed
  or unclaimed each frame, never both, so "Start = join" on a free pad and "Start = toggle_pause" on a joined pad never conflict.
- Writes only `UiEvent`, **never `PendingJoinGamepad`**; stays ungated while paused.
- **Must-have: each trigger is emitted at most once per frame across all pads** (pads in sorted order, buttons in map order);
  otherwise two players pressing Start in one frame produce two `toggle_pause` events and the FSM toggles straight back.
- Touch points: both binding-load sites in `project_loader.rs` (~:113/:127 and ~:252/:265), a new `Loaded…` resource, the CLI's
  button-name validation, `docs/20_data_formats.md`, `docs/dev/gamepad-routing.md`. A per-scene override is out of v1.
- A validate warning when a claimed-pad binding collides with a per-player button. A gamepad-only player still cannot click Quit/Mute
  (focus navigation is deferred) — say so.

### 6. Overlay `scene_key_bindings` (open question resolved: NOT merged)

Effective key/gamepad bindings are rebuilt only in the Replace branch (`scene_loader.rs:121-164`); an overlay contributes UI only, so
`scene_key_bindings` in an overlay is dead data and `unreachable_trigger` still reports it clean.

- v1: no merging (it needs a layered binding stack that `UnloadOverlay`/`ToggleOverlay` can undo). The **plan's old
  `scene_key_bindings: { "KeyR": "resume" }` recipe is dropped.** Document the `global_key_bindings` route (`"Escape": "toggle_pause"`,
  `"KeyR": "resume"`): safe because `ui.button_pressed:resume` is handled only from `paused`; note that a global `"KeyM": "toggle_mute"`
  would also fire during play. Suggest a second pause key (`"KeyP"`): browsers can eat Escape in fullscreen/pointer lock (verify).
- A one-line `warn!` in the overlay branch when the overlay scene has non-empty `scene_key_bindings`/`scene_unclaimed_gamepad_bindings`,
  plus a `validate` warning for scenes loaded only as overlays (docs/15 row), plus docs/20 (~L164): "ignored when the scene is loaded as
  an overlay". Backlog item for the merge/restore design.

### 7. `validate` checks (phase 3; all warnings unless noted)

- **`unrecoverable_pause`** (per state): `PauseGame` in a state's `entry_actions` with no `ResumeGame` in the **same state's**
  `exit_actions`. A Replace-mode `LoadScene` counts as a resume. (Plan change: the old "no reachable `ResumeGame` anywhere" rule was
  undefined and silenced by one `ResumeGame` in any other state.) Warning, not error: the auto-resume is the runtime safety net.
- `TogglePause` in any `entry_actions`/`exit_actions` (entering while already paused would unpause).
- `PauseGame` in a `.behavior.ron` (entity despawn → nothing resumes).
- `PauseGame` in an `on:`/`global_on` binding with no `ResumeGame`/`TogglePause` anywhere in the project.
- `EmitEventAfterDelay` in a pausing state's own `on:`/`entry_actions` (never fires while paused: a permanent freeze).
- Claimed-pad binding collision (above); overlay-only `scene_key_bindings` (above).
- Scan state machine, behaviors and dialogue choices; **known blind spot:** ActionBar slot `do_actions` (backlog `:262`).
- Rows in `docs/15_authoring_tools.md` "Logic wiring" table for each code.

### 8. Demo changes (phase 3)

Only two projects define pause/game-over states (`blank_project` mentions `paused` in a comment only).

- **`3rd_person_game_demo`** (`logic/state_machine.ron:224-256`): `paused` gets `PauseGame` in `entry_actions` and `ResumeGame` in
  `exit_actions`, plus `CloseInventory`/`CloseShop`/`CloseContainer` (and optionally `EndDialogue`) in the pause entry recipe (gameplay
  panels stay clickable under a pause because `button_system` is ungated).
- **`primitive_world`** (`:103-132`): `paused` same; `game_over` gets `PauseGame` in `entry_actions` and `ResumeGame` in `exit_actions`
  (Retry's `LoadScene(main)` also auto-resumes; the exit `ResumeGame` is then a harmless no-op). Its `playing.entry_actions` reset
  (`SetVariable` health/score/status/buff, `SetStat(player_health)`, `PlayMusicLoop`, preloads, particle warmup) must **not** re-run on
  unpause.
- **Free-heal fix:** run-start actions move out of `playing.entry_actions` into the `scene.ready:main` handlers of the states that
  start a run (`primitive_world`: `menu` and `game_over`; `3rd_person_game_demo`: `spawning_male`/`spawning_female`, plus the
  direct-to-main `menu` fallback), keeping only what must run on every (re)entry in `playing.entry_actions`. Verify the ordering the
  existing comment relies on ("`scene.ready:main` fires AFTER the `on:` Spawn rule … before playing's entry_actions") and with a
  browser check that a pause/resume no longer heals, resets the score or restarts the music, while a new run and Retry still reset.
- **Dying-while-paused fix** (closes backlog `:21`): real pause shrinks but does not remove the race (a `ModifyStat` executed in the
  same frame as the pause transition makes `stat_threshold_system` emit `depleted` the next frame, when the state is already
  `paused`, and `from: "playing"` drops it; threshold events never re-fire). `primitive_world`: `( on: "stat.player_health.depleted",
  to: "game_over" )` (drop `from:`); `3rd_person_game_demo`: move the death-animation binding (`:220`) to `global_on`.
- Rewrite the stale comments at `3rd_person_game_demo/logic/state_machine.ron` ~12-30, ~177-179 and ~195-197.

### 9. Networking fit (Beta 0.6+, architect answer 2026-10-14)

- `Time<Virtual>::pause()` is local-only. Lockstep: a peer that stops its clock stalls everyone, so a pause must be a tick-stamped
  session command. Host-authoritative (0.6 fallback, 0.8): only the host's clock matters; a client's `PauseGame` is a request to the
  host (policy host-only / anyone / vote) or only its local menu. Dedicated server (0.9): clients can never stop the sim; `PauseGame`
  only opens the menu.
- The `SimClock` hold set copes: a reserved `Session` hold written only by the network layer; in a networked run mode `Menu` stops
  touching the clock and `PauseGame` becomes "send a request if the policy allows". No designer-visible split is needed (the overlay
  is the menu; the action is the hold). Future `pause_policy: HostOnly | AnyPlayer | MenuOnly` is a project-level field decided per
  session mode, never per state. **Designer-facing RON is identical for single-player, couch co-op and online.**
- **Docs line (v1):** "In online play, `PauseGame` may only open your menu; build your pause UI around `game.paused`, not around the
  action itself."
- Replay: single-player pause is not a replay input (paused frames produce zero fixed ticks); actions executed in `Update` while paused
  (UI-driven `SetVariable`/`AddItem`, `JoinPlayer`) apply before the next tick. The online `Session` hold is an input and is recorded with
  its tick. Hold state lives in `SimClock`, never in game state.
- **Forward invariant** (also written into `gameplay_fixed_tick_pipeline.md` §6): the clock lever only works while the interpreters and
  executor run in `Update`; when they move into `FixedUpdate` the "paused control tick" must key off `SimClock::is_running`, not
  `Time<Virtual>::is_paused()` — control systems must run while the sim is held.

## Local co-op / split-screen

One `Time<Virtual>` ⇒ pause is global (correct for couch co-op). Any player can pause everyone; the pause overlay is one screen-space UI
across the whole window (it sits across the viewport seams in a 4-way grid) — document both. Hot join while paused is allowed (the
player spawns frozen and the split grid reflows; a pad connected while paused does not bind a pending player until resume because
`gamepad_bind_system` is fixed-tick) — document. A joined gamepad player can pause via `global_claimed_gamepad_bindings` (phase 2).

## Audio (documented, not changed)

Audio keeps playing while paused (deliberate). Docs must warn: `ToggleMute` in pause entry/exit **un-mutes** a player who had muted;
`StopMusic` → `PlayMusicLoop` restarts the track from the beginning; `SetVolume` overwrites the options preset shown via
`audio_volume_percent`; looping ambient SFX keep playing over frozen visuals. Real ducking (`PauseMusic`/`ResumeMusic` or a
music-volume action) is a follow-up. Designers can react to `game.paused`/`game.resumed`.

## Docs (phase 3; line numbers as of `d2191ab`, re-check)

- `docs/20_data_formats.md`: action table (three actions; no-op-if-already semantics; auto-resume on Replace load; FIFO consequence),
  `LoadScene` doc comment, ~L164 `scene_key_bindings` overlay note, ~L3254-3261 (rewrite: it uses `"Start": "toggle_pause"` as its example
  of what does not work) + `global_claimed_gamepad_bindings`.
- `docs/30_runtime_events_and_logic.md`: action list (~47, ~352), both pause FSM examples (~223-257, ~409-450; teach pattern (a) and the
  close-panels recipe), the "timers tick while paused" rationale (~675-678; the `global_on` rule stays, its reason changes), and a new
  section "What freezes and what keeps running while paused" (incl. the one-frame-delay, UI buttons still clickable, gameplay buttons
  belong in `playing`'s `on:`, audio footguns, hot join, any-player pause, online line).
- `docs/STATUS.md` (action list ~86, pause example ~157-165); `README.md` (~94, ~101, ~127: `LoadSceneOverlay` = "pause menus" rows and the
  3rd_person row); `docs/15_authoring_tools.md` (validate rows); `assets/projects/CLAUDE.md` (unit-variant list);
  `docs/dev/10_architecture.md:112` (`Paused` lifecycle state: make real or remove); `docs/dev/gamepad-routing.md`.
- `crates/ironhold_core/src/CLAUDE.md` rule b:575 ("`tick_delayed_events_system` ticks on raw `Time` with no pause gate") and the
  `lib.rs:365` `load-bearing:` comment: both become wrong (the timer freezes during a real pause but still fires in any non-`playing`
  state; the `global_on` rule stays). `capabilities/CLAUDE.md`: the new gate rule + the first-person/flycam camera note.
- `planning/features/gameplay_fixed_tick_pipeline.md` §6 (forward invariant above), `planning/features/gameplay_pipeline_system_sets.md`
  (new task: `PlayerInputSet`), `planning/features/static_scene_mode.md` and the focus-loss backlog item (use `SimClock` holds).
- Backlog: close/update `:21` (Bug) and the interim docs item; log: overlay key-binding merge/restore, real music ducking.
- Regenerate `pause_nav` baselines (`python test_web.py --update-baseline pause_nav`, `--real-gpu`, own commit).

## Tests

- Unit/integration (`ironhold_core/tests`): `SimClock` hold set semantics (idempotent, `Menu` only); paused ⇒ `Time<Virtual>::delta() == 0`
  and a fixed-tick counter stays 0; resume frame has no catch-up burst; `PauseGame`/`ResumeGame` no-op when already in state;
  `TogglePause`; `game.paused`/`game.resumed` emitted only on real change (incl. auto-resume); interact/loot/tab-target/click-select/action-bar
  suppressed while paused; **first-person mouse-look leaves the player's rotation unchanged while paused**; action-bar slot still `Pressed`
  on the first frame after resume does not fire; UI buttons still work while paused; pausing mid-scene-load; **`LoadScene` from `paused`
  while the FSM is still `paused`** (auto-resume, new scene unpaused); `LoadSceneOverlay` / `ToggleOverlay` close while paused stay paused;
  `[LoadScene, PauseGame]` starts paused; delayed events armed before pause fire after resume, not during; `EmitEventAfterDelay` armed from
  the pause menu fires only after resume; **pause in the same frame as lethal damage still reaches `game_over`**; Escape spam /
  Resume + Escape in the same frame; `Static` hold is not released by `ResumeGame`/auto-resume; an overlay `scene_key_bindings` entry is
  inert (pins today's behaviour); claimed-pad binding fires only for a claimed pad and **at most once per trigger per frame with two pads**,
  never writes `PendingJoinGamepad`, still fires while paused.
- RON: parse tests for the three variants (`ron_validation`), `ron_lint` after any demo RON change.
- CLI: `cargo check -p ironhold_cli`; validate tests for each new code (true and false positives incl. `LoadScene`-as-resume);
  `query actions` shows the new variants.
- Browser (`test_web.py`, after the cargo loop and the wasm build, never concurrently): pause/resume in both demos freezes monsters/player,
  Resume/Mute/Quit still work, no heal/score reset/music restart on unpause, Retry loads an unpaused fresh scene.

## Deferred (v2+)

- Engine-wide keyboard/gamepad focus navigation across pause, inventory, shop, container and dialogue (its own feature; every other panel
  is mouse-only, so pause-only navigation would teach a rule that breaks in four places).
- `pauses_game: true` state flag (possible later sugar expanding to the same request; `StateMachineAsset` is shared with `.behavior.ron`, which
  must reject it), per-scene claimed-pad bindings, overlay key-binding merge/restore.
- `SetTimeScale(f32)` (natural sibling on the same lever), real music ducking actions.
- Networked pause: `pause_policy`, `Session` hold wiring, votes, tick-stamping, server-admin holds (Beta 0.6-0.9).
- Re-evaluate (do not blindly revert) the earlier workaround that moved the gated-door rules from `global_on:` into `"playing"`'s `on:`
  (`item_gated_interactable.md`); the monster-respawn rules stay in `global_on:` regardless.

## Acceptance criteria

- Given `3rd_person_game_demo` in `paused`, monsters do not move or attack, WASD does not move the character, `F` does not interact or
  loot, the scene behind the menu is visually frozen (including first-person/flycam mouse look), and Resume, Toggle Mute and Quit still work.
- Given an unpause, health, mana, score and music are **not** reset; given a new run or Retry, they are.
- Given `primitive_world`'s game-over screen, Retry loads a fresh, **unpaused** scene; given lethal damage in the same frame as the pause,
  the game still reaches `game_over`.
- Given a monster death timer armed just before pausing, the respawn fires after resume, not during the pause.
- Given a joined gamepad player pressing the claimed-pad pause button, the game pauses and the same button resumes it, with no double toggle
  when two players press it in one frame.
- Given a `paused` state with `PauseGame` and no `ResumeGame` in its `exit_actions`, `ironhold validate` warns (`unrecoverable_pause`).
- Given no pause action is authored, every existing project behaves exactly as before.
