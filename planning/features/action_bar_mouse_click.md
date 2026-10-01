# Feature: Mouse-Click Activation for Action Bar Skill Slots

_Status: Ready (plan-review passed 2026-09-30; architect B1 and ux B1/B2 folded in)_
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
1. **No new system or resource — detect the click per-entity inside `action_bar_input_system`.**
   Widen its slot query to `Query<(&ActionSlotUi, Option<Ref<Interaction>>)>` and compute
   `click_fired = interaction.is_some_and(|i| i.is_changed() && *i == Interaction::Pressed)`.
   `Ref::is_changed()` compares against the system's own last run, so it is the press-edge detector
   (fires once on the transition to `Pressed`; not while held; not on release) — no `Local`, no
   extra resource. The `Option` is required: existing action-bar tests spawn a bare `ActionSlotUi`
   with no `Button`/`Interaction` (`tests/entity_logic_tests.rs`, `local_coop_tests.rs`), and a
   required `Ref<Interaction>` would silently drop them from the query. Everything stays inside
   `ActionBarPlugin`'s existing `.chain().before(fsm_interpreter_system)`, so the documented
   TargetingPlugin ordering guarantee in `crates/ironhold_core/src/CLAUDE.md` is untouched.
   - *Rejected (plan-review, system-architect B1):* a separate `action_slot_click_system` writing a
     `ClickedSlots(HashSet<String>)` resource. Its clear/refill lifecycle was ambiguous (a stale key
     would re-fire every frame), it went stale across `LoadScene` (both systems are
     `run_if(any_action_slots)`), and it was keyed by slot-key string, so two bars sharing a key
     would both fire from one click. Per-entity `Ref` fires only the slot actually clicked.
2. **`action_bar_input_system` change (small):**
   - `click_fired` as above, gated off when `InspectorEnabled` is true (see step 8);
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
6. **WASM / perf:** negligible — a per-slot change-tick compare; no extra system, resource or
   allocation, and nothing written on the no-click path.
7. **Determinism / replay (Beta 0.5):** do **not** record `Interaction` for replay/netcode — it is
   derived from layout and cursor position, not a stable input. The natural capture point is the
   existing device-agnostic intent layer (`intent.slot.{key}:{player}`). Nothing to build now.
8. **Inspector gating:** with the `inspector` feature on and `InspectorEnabled` true, egui windows
   over the HUD don't block `ui_focus_system`, so clicking an inspector pane over the bar would
   fire a skill. Gate **clicks only** on `InspectorEnabled` (as `button_system` and the
   camera/input systems already do); keyboard slot presses stay ungated as today.
9. **Deliberate parity notes (record in docs):** like keyboard presses, clicks have no
   `panels_open`/pause gate; they are naturally blocked under a `FocusPolicy::Block` panel root or
   overlay backdrop. Clicks in split-screen route by the bar's `owner_player`, whereas a *world*
   click (`click_select_system`) routes by the viewport under the cursor — document both rules
   side by side so a designer doesn't assume clicking in P2's half always means P2.
10. **Bar padding/gaps (playtest item):** the bar root is `FocusPolicy::Pass` with no `Interaction`,
    so a click on padding between slots falls through to `click_select_system` and clears the
    world target. Acceptable for v1; if playtest finds it annoying, give the bar root
    `FocusPolicy::Block` + `Interaction::default()` (same pattern as panel roots/overlay backdrop).

## Tasks
- [ ] Extend `action_bar_input_system` per Approach steps 1-2 (`Option<Ref<Interaction>>`, single
      collapsed "fired" decision, inspector gate) — no new system/resource
- [ ] Update `ActionSlotDef.key` doc comment in `schema/scene_v2.rs` (currently says mouse buttons
      "not supported" — still true for the `key` *string*; add that clicking the slot itself works)
- [ ] Tests (harness patterns live mostly in `entity_logic_tests.rs`/`local_coop_tests.rs`, plus
      `action_tests.rs`; consult `integration-test-author`). Drive by inserting/mutating
      `Interaction::Pressed` on the slot entity (`ui_focus_system` isn't present under
      `MinimalPlugins`); for the "held" test do **not** re-insert each frame — that counts as a change:
  - [ ] click fires slot: `do_actions` queued, `action_bar.pressed` + `action_bar.activated` emitted, cooldown started
  - [ ] click during cooldown → `action_bar.on_cooldown:{key}`, nothing queued
  - [ ] click with insufficient `cost` → `action_bar.insufficient_resource:{key}`
  - [ ] click on a `{target}` slot with no target → `action_bar.no_target:{key}`
  - [ ] `owner_player: Some(1)` slot clicked → acts for player 1 (target/cost from player 1's own `PlayerTarget`/`StatMap`)
  - [ ] a `state_machine.ron` rule matching `intent.slot.{key}:{player}` suppresses the built-in `do_actions` on click, same as on key press
  - [ ] click and key press on the same frame → exactly one activation
  - [ ] holding the button (Pressed persists across frames) → exactly one activation, not one per frame
  - [ ] clicking a slot does not change `CurrentTarget` / `PlayerTarget` (guards the `click_select_system` early-return assumption)
  - [ ] two bars sharing a slot key: clicking one fires only that slot
  - [ ] existing bare-`ActionSlotUi` (no `Interaction`) key-press tests still pass
- [ ] `cargo test -p ironhold_core --test '*'` (one-file-at-a-time loop per root `CLAUDE.md` — disk!)
- [ ] `cargo check -p ironhold_cli` (mandatory gate even though no schema change is expected)
- [ ] Docs (plan-review, ux B2 + non-blocking):
  - `docs/20_data_formats.md` ~L996: replace "No mouse-click binding"; add an "Activating slots" note
    listing keyboard, `gamepad_key` and left-click/tap as the three sources, the shared-mouse
    ownership rule next to the world-click viewport rule, "no hover/pressed highlight in v1", "only
    left button/touch activates (right/middle do nothing)", and "every ActionBar is clickable; a
    decorative bar needs slots with empty `do_actions`"
  - `docs/20_data_formats.md` ~L1027 "Accepted key names": clarify a mouse button can't be a `key`
    string but clicking the slot itself always works
  - event tables (`docs/20` ~L1151-1154, `docs/30_runtime_events_and_logic.md` ~L130-134): reword
    "key pressed" to "slot activated (key, `gamepad_key` or click)"; event name always uses `key`
  - `docs/20` "Per-player action bars (split-screen)" (~L1255): one-line pointer that a click acts
    for the bar's `owner_player` whichever half the cursor is in
  - `crates/ironhold_core/src/CLAUDE.md`: paragraph next to "Gamepad-routed action-bar slots"
  - do not promise touch support unless tap-to-target is confirmed to work (see playtest)
- [ ] `3rd_person_game_demo/scenes/main.scene.ron` ~L418-421: comments say "Press 2..." and claim a
      missing target is a silent no-op — reword to "Press or click" and fix the stale no-op claim
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
- **Required — orbit/strafe check, in a project using the real defaults.** `3rd_person_game_demo` and
  `local_coop_demo` override `orbit_button` to `"Right"`/`"None"`, so they can't reveal the bug. Use
  `primitive_world` (`scenes/main.scene.ron` ~L658) or `stats_demo` (~L239): player has no `camera:`
  block, so `orbit_button: "Either"` and `strafe_mouse_button: Some("Left")`.
  - Hold LMB on a slot and move the mouse slightly: camera must not orbit / character must not rotate.
  - Hold LMB on a slot and press A/D: character must not switch to strafing (`input_translator_system`
    reads LMB directly with no UI guard; `strafe_mouse_button` defaults to `Left`).
  - If either bites, prefer one shared fix (a `UiPointerCaptured` resource/run-condition derived from
    `Interaction`, read by orbit, strafe and click-select) logged as its own backlog item, over
    per-system guards. Record results in the playtest notes.
- Click empty bar padding/gaps between slots: note whether the world target clears (Approach step 10).
- Open the pause overlay / inventory / dialogue over the bar and click where a slot is: the slot must not fire.
- With the inspector enabled (`--all-features`), click an egui pane over the bar: no skill fires.
- `local_coop_demo` rooms 9 and 10 (two bars each) alongside room3.
- Web build: repeat on Chrome; if touch hardware is available, tap an enemy then tap a `{target}` slot
  (`click_select_system` reads the mouse button — confirm before documenting touch support).

## Decisions (Frank, 2026-09-30)
- **Fire on press**, not release — parity with keyboard `just_pressed`, lower latency.
- **No opt-out field.** Every action bar is clickable; add `mouse_click: bool` on `ActionBarDef`
  later only if a real need appears.
- **One physical mouse may operate any player's bar** in split-screen (like the keyboard); no
  viewport→player resolution.
- **Camera orbit/strafe while clicking is a playtest requirement, not an open question:**
  `camera_orbit_system` and `input_translator_system` have no UI-hover guard. Verify in a
  default-bindings project (see Playtest checklist); if it bites, fix in this feature or log a
  separate backlog item before merging (decide at playtest).
- **Inspector gating (proposed by plan-review, Frank to confirm):** clicks gated on
  `InspectorEnabled`; keyboard presses ungated.

## Playtest results (Frank, 2026-10-01, dev WASM build of `534aed6`)
- `3rd_person_game_demo` basic checks (click fires, cooldown, repeat click, target kept, empty-world deselect, key+click once): **pass**.
- Orbit/strafe check in a default-bindings project: **fails** (camera orbits / character strafes while holding LMB on a slot) — pre-existing, not introduced here; split into its own backlog bug (`## Bugs`, "Left mouse button on any UI node also orbits the camera / strafes the character") per the plan's Decisions.
- Click on bar padding/gaps: acceptable (world target behavior as expected).
- Overlay/window over the bar: not practical to test (windows are not movable by the player).
- Inspector gate: **not yet verified** (web toggle is `` ` ``/Backquote, not F9 — F9 is the collider wireframes).
- `local_coop_demo` rooms with per-player bars: **pass**.
- Touch: **untested** (no device) — docs already say touch is unverified.

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
