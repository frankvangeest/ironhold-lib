# Feature: Real Pause (v1 — simulation freeze)

_Status: Queued_
_Planned at: `2a034d6` (2026-09-15)_

## What

Make "paused" stop the game. Today `LoadSceneOverlay("scenes/pause.scene.ron")` draws a
menu and nothing else — NPC AI, physics, player movement, interact/loot, particles, and
delayed-event timers all keep running behind it. Add three actions that a designer puts
in a state's `entry_actions`/`exit_actions`, next to the overlay action they already use.

## Why

Reported by Frank playtesting `3rd_person_game_demo` (2026-09-15): monsters kept
patrolling and attacking, chests could still be looted, and the character kept walking
around behind the pause overlay. Not project-specific — `primitive_world` has the same
defect in both its `paused` and (worse) its `game_over` states, and four docs pages
currently document pause as if it already works (`docs/10_architecture.md` lists a
`Paused` engine lifecycle state that does not exist; `docs/30_runtime_events_and_logic.md`
and `docs/STATUS.md` both ship the exact broken `paused` FSM block as a canonical
copy-paste pattern).

## Scope decision 1 — what "paused" means

**Freeze `Time<Virtual>` as the primary lever, plus an explicit `GamePaused` gate for
input-driven systems.** One lever is not enough on its own:

- `Time<Virtual>` covers everything delta-driven for free: Bevy's `AnimationPlayer`,
  NPC AI ticks, motion (rotate/bob), particle lifetimes, cooldowns, `DespawnTimer`,
  `EmitEventAfterDelay`, physics step. Correct by default, no per-system surgery.
- It does **not** cover edge-triggered input systems, which is exactly the reported
  bug (looting still worked). `interactable_system`, `collectible_system`,
  `tab_targeting_system`, `click_select_system`, `action_bar_input_system` and the
  player's own input read fire on key/click edges, not on delta. These need an
  explicit gate.

Must keep running while paused: Bevy UI interaction, `message_interpreter_system`,
`action_executor_system` (Resume/Quit/ToggleMute have to work), the scene loader,
and audio. `Time<Real>` and rendering are untouched.

Camera freezes too (it is delta-driven). This is desirable: a frozen tableau behind
the menu, and no mouse-look fighting the menu cursor.

Audio keeps playing — deliberately. Designers who want ducking already have
`SetVolume`/`ToggleMute`/`StopMusic` available as `entry_actions` on the paused state.
No new schema needed.

## Scope decision 2 — WASD menu navigation: NO, not now

Frank's request decomposes into two things, and only one is a real gap:

1. *"Stop WASD moving my character"* — that is just pause. Solved by v1.
2. *"Act on the menu without the mouse"* — **already possible today**, zero engine work:
   bind keys directly to the same triggers the buttons fire.

   ```ron
   // pause.scene.ron
   scene_key_bindings: { "KeyR": "resume", "KeyM": "toggle_mute" }
   ```

   ⚠️ **Must verify before documenting this recipe:** `scene_key_bindings` is
   documented as "cleared on each scene load", and an overlay load only spawns the UI
   section. If overlay loads do not merge the overlay scene's `scene_key_bindings`,
   this recipe silently does nothing and designers must fall back to
   `global_key_bindings` — which would then also fire during normal play. Either make
   overlay loads merge them, or document the `global_key_bindings` fallback explicitly.

Real focus-based menu navigation is deliberately deferred. It is a full feature, not a
fix: focus ring rendering, authored focus order, wrap behaviour, gamepad d-pad parity,
and per-player focus in 4-way split-screen. Critically, **every other panel in this
engine is mouse-only** (inventory, shop, container, dialogue). Making pause alone
keyboard-navigable teaches designers a rule that then breaks in four other places —
a worse UX than today. When it is built, it must be built engine-wide.

## Scope decision 3 — local co-op / split-screen

There is one `Time<Virtual>` for the app, so pause is global by construction. That is
also the correct behaviour for single-machine couch co-op — every couch co-op game
pauses for everyone. No per-player pause in v1. Two consequences to document:

- **Any player can pause everyone.** Accepted for v1.
- **A joined gamepad player cannot pause at all today.** `global_unclaimed_gamepad_
  bindings` only fires on pads not claimed by a player, so `"Start": "toggle_pause"`
  is unreachable for anyone actually playing. This is a real co-op gap. Either fix it
  in v1 (a player-scoped pause binding) or log it as its own item — do not ship the
  pause feature while silently leaving gamepad players with no menu.
- The pause overlay is one screen-space UI centred on the whole window, so in a 4-way
  grid it sits across the viewport seams. Acceptable for v1; note it in docs.

## Scope decision 4 — networking (Beta 0.6+)

A client-side simulation freeze desyncs other players, so pause must be a *policy the
run mode owns*, not a hardcoded global. Deliberately **not** adding a designer-facing
`pause_policy` field now — that pushes an engine concern onto designers and would need
docs/example coverage for a variant nothing can use yet. Instead: route `PauseGame`
through a single internal clock/run-mode owner so that when LAN co-op arrives,
`PauseGame` can degrade to "open the menu, don't stop the clock" with no content change.

**This owner is shared with `planning/features/static_scene_mode.md`**, which also plans
to call `time.pause()`. Whichever lands first must establish the owner; otherwise
opening the pause menu in a `?static=1` session and resuming un-freezes static mode.
`pause_on_focus_loss` (backlog Queued) must use the same owner, not a third mechanism.

## v1 scope

- `GamePaused(bool)` resource with exactly one owning system.
- Three actions, mirroring the shipped `OpenInventory`/`CloseInventory`/`ToggleInventory`
  triple (tuple variants, no fields): **`PauseGame`**, **`ResumeGame`**, **`TogglePause`**.
  Designer-guessable from the existing `ui.button_pressed:toggle_pause` trigger name.
- Pause = `time.pause()` + `GamePaused(true)`; gate the edge-triggered input systems
  listed above behind a shared `not(game_paused)` run condition. **Collapse the three
  existing `panels_open > 0` early-returns into that same condition** rather than adding
  a fourth independent check.
- **Auto-resume on a Replace-mode `LoadScene`.** This is the highest-risk v1 footgun:
  `primitive_world`'s `game_over` → `retry` fires `LoadScene(main)` with no
  `ResumeGame`, which would boot the new scene permanently paused — an unrecoverable
  soft-lock with no diagnostic.
- `ironhold_cli validate` check `unrecoverable_pause`: a state whose `entry_actions`
  contain `PauseGame` with no reachable `ResumeGame`/`TogglePause` anywhere.
- Example updates (all three are broken today):
  - `3rd_person_game_demo` `paused` → add `PauseGame`/`ResumeGame`.
  - `primitive_world` `paused` → same.
  - `primitive_world` `game_over` → add `PauseGame` (and confirm the retry path resumes).
- Docs — six surfaces, per the standard new-Action checklist:
  - `docs/20_data_formats.md` action table
  - `docs/30_runtime_events_and_logic.md` action list **and** both pause FSM examples
    (~229, ~360)
  - `docs/STATUS.md` action list **and** its pause example (~158)
  - `assets/projects/CLAUDE.md` tuple-variant list
  - `docs/10_architecture.md:110` — make `Paused` real or remove it
  - `docs/30_runtime_events_and_logic.md:599-606` **and** the RON comment at
    `3rd_person_game_demo/logic/state_machine.ron:12-30` — the "timers tick while
    paused" rationale becomes false; the `global_on` rule stays, its reason changes.
- Tests: pausing mid-scene-load; a `LoadScene` issued while paused; delayed events
  armed before pause firing after resume (not during); interact/loot suppressed while
  paused; UI buttons still clickable while paused.
- Regenerate `pause_nav` baselines (`python test_web.py --update-baseline pause_nav`) —
  they should become *more* stable, since the scene no longer drifts behind the menu.

## Deferred (v2+)

- **Engine-wide keyboard/gamepad focus navigation** across pause, inventory, shop,
  container, and dialogue: `focus_order` on button defs, a visible focus ring, wrap
  behaviour, gamepad d-pad parity, per-player focus in split-screen. Its own feature.
- **Player-scoped pause for joined gamepad players** (if not folded into v1).
- **`SetTimeScale(f32)`** for slow-motion — natural sibling on the same lever, out of scope.
- **Per-player / networked pause semantics** at Beta 0.6, once a run mode exists.
- Re-evaluate (do **not** blindly revert) the local workaround that moved the gated-door
  rules from `global_on:` into `"playing"`'s `on:` list (`item_gated_interactable.md`).
  Note that the monster-respawn rules must stay in `global_on:` regardless — their
  reason is unrelated to pause.

## Open questions

- Does `LoadSceneOverlay`/`ToggleOverlay` merge the overlay scene's own
  `scene_key_bindings` into the running scene, or does an overlay only ever contribute
  its UI section? This decides whether "act on the menu without a mouse" (scope
  decision 2) is free today or needs its own small fix.
- Should the joined-gamepad-player pause gap (scope decision 3) be folded into v1, or
  filed as its own backlog item? Decide before implementation starts, not during the
  co-op playtest.
- Who owns the shared clock/run-mode resource this feature needs to coordinate with
  `static_scene_mode.md` and `pause_on_focus_loss` — is it built here, or does this
  feature block on a small shared-owner primitive landing first?

## Acceptance criteria

- Given `3rd_person_game_demo` in the `paused` state, monsters do not move or attack,
  the player character does not move on WASD, `F` does not interact or loot, and the
  scene behind the menu is visually frozen.
- Given the pause menu is open, Resume, Toggle Mute, and Quit are all still clickable
  and work.
- Given `primitive_world`'s game-over screen, Retry loads a fresh, **unpaused** scene.
- Given a monster death timer armed just before pausing, the respawn fires after
  resume, not during the pause.
- Given no pause action is authored, every existing project behaves exactly as before.
