# Feature: Mouse-Click Activation for Action Bar Skill Slots

_Status: Ready (open questions resolved 2026-09-30; plan-review still to run)_
_Planned at: `70cb631` (2026-09-30)_

## What
Clicking a skill slot on an action bar with the left mouse button activates it, exactly as if its
bound keyboard key (or `gamepad_key`) had been pressed. No new RON fields: every existing
`ActionBarDef`/`ActionSlotDef` becomes clickable, and cooldown, `cost`, `{target}` gating,
`owner_player` resolution, the `intent.slot.*` rule-override hook and every `action_bar.*` event
behave identically regardless of which device fired the slot. Motivating example:
`3rd_person_game_demo`.

## Why
Today the docs state it outright ("No mouse-click binding — a designer-clicked slot button does
nothing", `docs/20_data_formats.md`). Clickable skill bars are the default expectation for
mouse-driven action games, and the slots are already spawned as `Button` entities
(`scene_loader.rs`, `Name::new("Slot:{key}")`), so Bevy already tracks an `Interaction` on every
slot — nothing consumes it. It also gives touch/tap activation on web for free, since Bevy's
`Interaction` is driven by touch as well as mouse.

## Findings from reading the code (so the approach below isn't guesswork)
- **Slots are already `Button` + `Interaction`.** `button_system` (`lib.rs`) requires a `UiAction`
  component, which slots do not have, so it ignores them — no double handling and nothing to
  disable.
- **World click-through is already blocked.** `click_select_system` (`capabilities/targeting.rs`)
  returns early when any `Interaction` is `Pressed`, so clicking a slot will not also
  select/clear a world target. Verify with a test rather than assume (see Tasks).
- **Whole-slot hit area comes for free.** The icon, cooldown overlay and key-hint text are child
  `Node`s without their own `Interaction`; the `Button` root receives the hit. The backlog's open
  question ("icon only, or whole slot?") resolves to: the whole slot square, not the whole bar.
- **The fire path is one function.** `action_bar_input_system` computes `keyboard_fired` /
  `gamepad_fired` and everything after (player resolution, cooldown, cost, `{target}`,
  intent event, pending actions) is device-agnostic. A click is a third input source feeding
  the same booleans — not a separate code path.

## Approach
1. **New tiny system `action_slot_click_system`** in `capabilities/action_bar.rs`:
   `Query<&ActionSlotUi, Changed<Interaction>>` filtered to `Interaction::Pressed`, writing the
   clicked slots' keys into a new `ClickedSlots(HashSet<String>)` resource (cleared and refilled
   each run; same shape as `HandledIntentSlots`). It fires on the **press edge**
   (`Changed<Interaction>` == `Pressed`), not on release and not while held, matching
   `just_pressed` keyboard semantics. It is added to `ActionBarPlugin`'s existing `.chain()`
   immediately before `action_bar_input_system` (still `.before(fsm_interpreter_system)`), so the
   `TargetingPlugin`-ordering guarantee documented in `crates/ironhold_core/src/CLAUDE.md` is
   untouched.
   - Not `Query<&Interaction>` polled directly inside `action_bar_input_system`: that would need a
     `Local` to edge-detect and would widen a system whose query is already wide.
2. **`action_bar_input_system` change (small):**
   - add `clicked: Res<ClickedSlots>`;
   - `let click_fired = clicked.0.contains(slot.slot_key.as_str());`
   - treat `click_fired` like `keyboard_fired` — both are "device-independent" fires. The
     fast-path skip becomes `!keyboard_fired && !click_fired && slot.resolved_gamepad_button.is_none()`;
     the pre-player-resolution cooldown gate fires for `keyboard_fired || click_fired`; the
     post-resolution gate for gamepad-only stays as is; `if !keyboard_fired && !click_fired && !gamepad_fired { continue }`.
   - Because all three sources collapse into one "fired this frame" decision per slot, a click and
     a key press on the same frame produce **one** activation and one `action_bar.pressed:{key}`,
     not two.
3. **Ownership: the mouse is shared hardware, like the keyboard.** A click activates the slot for
   the slot's own `owner_player` (resolved by the existing `owns_slot`), regardless of which
   split-screen viewport the cursor is in. This mirrors the documented keyboard rule ("`key` fires
   from the one global `ButtonInput<KeyCode>` regardless of `owner_player`") and needs no new
   per-player mouse-routing machinery. Answers the backlog's `owns_slot` question: yes, same
   scoping as keyboard, not the stricter per-device scoping gamepad uses (a gamepad is per-player
   hardware; a mouse is not).
4. **No schema change, no new action/event.** Deliberately no `clickable: bool` field in v1 (see
   Open questions). Consequences: `ironhold_cli` needs no update, `docs/20_data_formats.md` needs a
   behavior-description update only, and existing projects gain click support automatically.
5. **Visual feedback** is out of scope for v1 beyond what already exists (the cooldown overlay and
   `action_bar.activated` events already convey the result). A hover/pressed highlight is a small
   follow-up that fits with **"Drop shadow support for UI text"** / the slot-restyle backlog item
   and should be its own item.
6. **WASM / perf:** negligible. `Changed<Interaction>` over a handful of slots; the new system can
   `run_if(any_action_slots)` like its siblings. No allocations on the no-click path (the resource
   is only written when a slot was actually pressed — guard `clear()`/`insert` so change detection
   doesn't fire every frame, per the change-detection-discipline rule in `crates/ironhold_core/src/CLAUDE.md`).
7. **Determinism / replay (Beta 0.5):** the click source is `Interaction` (set by Bevy's
   `ui_focus_system` in `PreUpdate` from real mouse/touch input), so a future replay recorder must
   capture it as an input like key presses. Note it in `deterministic_fixed_timestep.md`/the
   replay plan when that lands; nothing to build now.

## Tasks
- [ ] Add `ClickedSlots` resource and `action_slot_click_system`; register both in `ActionBarPlugin`,
      chained before `action_bar_input_system`
- [ ] Extend `action_bar_input_system` per Approach step 2 (single collapsed "fired" decision)
- [ ] Update `ActionSlotDef.key` doc comment in `schema/scene_v2.rs` (currently says mouse buttons
      "not supported" — still true for the `key` *string*; add that clicking the slot itself works)
- [ ] Tests in `crates/ironhold_core/tests/action_tests.rs` (drive by inserting/mutating
      `Interaction::Pressed` on the slot entity — see the existing key-press tests for harness setup;
      consult `integration-test-author`):
  - [ ] click fires slot: `do_actions` queued, `action_bar.pressed` + `action_bar.activated` emitted, cooldown started
  - [ ] click during cooldown → `action_bar.on_cooldown:{key}`, nothing queued
  - [ ] click with insufficient `cost` → `action_bar.insufficient_resource:{key}`
  - [ ] click on a `{target}` slot with no target → `action_bar.no_target:{key}`
  - [ ] `owner_player: Some(1)` slot clicked → acts for player 1 (target/cost from player 1's own `PlayerTarget`/`StatMap`)
  - [ ] a `state_machine.ron` rule matching `intent.slot.{key}:{player}` suppresses the built-in `do_actions` on click, same as on key press
  - [ ] click and key press on the same frame → exactly one activation
  - [ ] holding the button (Pressed persists across frames) → exactly one activation, not one per frame
  - [ ] clicking a slot does not change `CurrentTarget` / `PlayerTarget` (guards the `click_select_system` early-return assumption)
- [ ] `cargo test -p ironhold_core --test '*'` (one-file-at-a-time loop per root `CLAUDE.md` — disk!)
- [ ] `cargo check -p ironhold_cli` (mandatory gate even though no schema change is expected)
- [ ] Docs: `docs/20_data_formats.md` (replace the "No mouse-click binding" sentence in the ActionBar
      section; add a short "Activating slots" note listing keyboard, `gamepad_key` and mouse
      click as the three sources, with the shared-hardware ownership rule); `crates/ironhold_core/src/CLAUDE.md`
      (add a paragraph next to "Gamepad-routed action-bar slots")
- [ ] Demo: no RON change needed — `3rd_person_game_demo`'s existing bar becomes clickable. If a
      dedicated split-screen check is wanted, use `local_coop_demo/scenes/room3.scene.ron`
      (two `owner_player`-scoped bars)
- [ ] WASM dev build + playtest checklist (below)

## Playtest checklist (draft)
- `3rd_person_game_demo`: click each skill slot with the mouse → same effect as its key; cooldown
  overlay appears; clicking again during cooldown does nothing visible and emits no duplicate activation.
- Click a slot while an enemy is targeted → target stays selected (no deselect from the click).
- Click empty world next to the bar → target still deselects as before (click-through only blocked over the slot).
- Press the slot's key and click it in the same moment → one activation.
- `local_coop_demo` room3: click each bar → the correct player acts; keyboard/gamepad still work.
- **Required:** left-click (and hold, moving the mouse slightly) on a slot with the default orbit
  binding → the camera must not orbit/rotate the character; record the result in the playtest notes.
- Web build: repeat on Chrome; if touch hardware is available, tap a slot.

## Decisions (Frank, 2026-09-30)
- **Fire on press**, not release — parity with keyboard `just_pressed`, lower latency.
- **No opt-out field.** Every action bar is clickable; add `mouse_click: bool` on `ActionBarDef`
  later only if a real need appears.
- **One physical mouse may operate any player's bar** in split-screen (like the keyboard); no
  viewport→player resolution.
- **Camera orbit while clicking is a playtest requirement, not an open question:** `camera_orbit_system`
  has no UI-hover guard (no `Interaction` use in `capabilities/camera.rs`), so a left-click on a slot
  with `orbit_button` including `Left` may start an orbit drag if the mouse moves while held. The
  playtest must explicitly verify this; if it bites, fix it in this feature or log a separate
  backlog item before merging (decide at playtest).

## Acceptance criteria
- Given a slot bound to `key: "1"`, when the player left-clicks that slot, then its `do_actions`
  run exactly as if `1` had been pressed, including cooldown start and the `action_bar.*` events.
- Given a slot on cooldown, or lacking the stat `cost`, or needing a `{target}` that isn't set,
  when it is clicked, then it emits the same `on_cooldown` / `insufficient_resource` / `no_target`
  event as the keyboard path and does nothing else.
- Given a `state_machine.ron` rule that handles `intent.slot.{key}:{player}`, when the slot is
  clicked, then the rule runs and the slot's built-in `do_actions` are suppressed.
- Given the mouse button is held on a slot, then the slot activates once, not every frame.
- Given a click on a slot, then the world target selection is unchanged.
- Given a click and a key press on the same frame, then exactly one activation occurs.
- Given an existing project with no RON changes, then it validates cleanly and its bars are now clickable.
