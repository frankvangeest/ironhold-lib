# Action-bar input routing and per-player targeting

How targeting, the action bar and mouse clicks reach `{target}` and the interpreters, per player.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Per-player targeting Phase 1: `PlayerTarget` vs `CurrentTarget`

<!-- b:190 -->

**Per-player targeting (Phase 1, `planning/features/done/per_player_split_screen_targeting.md`)** —
each player entity carries its own `PlayerTarget(Option<String>)` component
(`capabilities/player.rs`), inserted in `entity_spawner.rs::spawn_player_entity_core`'s shared
post-model-source-dispatch code — as of `player_model_source_unification.md` v1 this covers both
GLB and primitive players spawned via the immediate scene-load path (see "Player-construction
sites" in `docs/dev/player-spawn-sites.md`). `CurrentTarget` (`capabilities/action_bar.rs`) was deliberately kept as a resource
rather than deleted — it is now "the primary player's `PlayerTarget`, mirrored". The **primary
player** is whichever player entity has `PlayerIndex(0)` or no `PlayerIndex` at all (see
"Player-construction sites" for when the latter is still reachable). `{target}` substitution (earlier on this page), and any `state_machine.ron`-
overridden slot intent's `do_actions` (see Phase 2 later on this page), keep reading `CurrentTarget` exactly as
before this feature — those two paths only ever resolve against the primary player. A non-primary
player's `PlayerTarget` drives their own visual feedback (ring, per-viewport HUD readout) *and*,
as of Phase 2, their own action bar's slots — but never `state_machine.ron`/behavior
actions fired outside the action bar, nor a rule that overrides a slot's intent event. This is a
documented scope boundary, not a bug.

## Per-player action bars Phase 2: `owner_player`, `owns_slot`, cost pools, duplicate-key detectors

<!-- b:206 -->

**Per-player action bars (Phase 2, `planning/features/done/per_player_split_screen_targeting.md`)** —
`ActionBarDef.owner_player: Option<u32>` (`#[serde(default)]`), copied onto `ActionSlotUi` at scene
load, scopes a bar's slots to whichever player entity carries `PlayerIndex(owner_player)`; `None`
(or `Some(0)`) means the primary player, same definition as earlier on this page. **This did *not* need player
identity threaded through the Message → Interpreter → Action → Executor pipeline** — the original
Phase 1 speculation about that (see `planning/claude_suggestions.md` ▸ Camera) turned out to be
wrong once `action_bar_input_system` was re-read: the action bar already calls `rewrite_target`
itself, locally, before anything reaches `ActionQueue`, so `{target}` is already a concrete entity
ID by the time the interpreter chain sees it. `action_bar_input_system` was rewritten from a single
`find`+`return` (which silently dropped one player's press if 2+ slots fired the same frame) to a
loop over **every** slot whose resolved key is `just_pressed`; for each, `owns_slot(owner_player,
player_index)` resolves the acting player, and that player's own `PlayerTarget` — not the global
`CurrentTarget` — drives the `{target}` rewrite, the no-target gate, and the
`intent.slot.*:{player_id}` event's player id. For the primary player this is a no-op in practice
(`PlayerTarget` is already kept in lockstep with `CurrentTarget` for the primary player). **The
`cost:`/`SlotCost` check/deduct is now per-player too** (`planning/features/done/per_player_stat_pools.md`):
it resolves against the acting player's own `StatMap` first — populated from `PlayerConfig.
stat_templates`, forwarded from `PrefabDef.stat_templates` exactly like any NPC/prop prefab, and
inserted by `spawn_player_entity_core` — falling back to the single shared `LoadedStats` resource
only when that player's prefab declares no matching `stat_templates` entry (see
`docs/20_data_formats.md`'s `SlotCost` section). The check and the deferred deduct action's key are
resolved **once** per firing slot (`resolve_cost_source` in `action_bar.rs`) and reused for both,
rather than independently re-resolved, so the two can never disagree about which pool a slot's cost
hits. A scene-load `warn!` (`scene_loader.rs::warn_missing_player_stat_templates`) plus an
`ironhold_cli validate` error (`missing_player_stat_template`) both flag the one likely-mistake
case: an `owner_player`-scoped bar's `cost.stat` isn't among that player's *own* declared
`stat_templates`, even though the player clearly opted into a per-player pool by declaring some.
Declaring **no** `stat_templates` at all is never flagged — that's the ordinary, unchanged global
fallback every single-player project (and any bar that doesn't opt in) still gets. **What remains
out of scope**: a `state_machine.ron` binding that intercepts a non-primary player's slot intent still resolves
its own replacement `do_actions`' `{target}` via the interpreter against `CurrentTarget` (the
primary player), not the firing player's `PlayerTarget` — only the slot's *own* built-in
`do_actions` (bypassed when a rule takes over) get the per-owning-player resolution. Two bars
sharing a slot key get both a scene-load `warn!` (`scene_loader.rs::warn_cross_bar_duplicate_keys`,
scene-wide, unlike the pre-existing per-bar-only check) and an `ironhold_cli validate` error
(`cross_bar_duplicate_key`), since `CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are
still keyed by the literal slot key string alone, scene-wide. Both detectors key their "same bar
vs. different bar" check by positional index, not `ActionBar.id` — nothing enforces `id`
uniqueness, so comparing by `id` would misclassify a real cross-bar collision if two bars happened
to share one (system-architect finding, plan-review).

## Gamepad-routed action-bar slots (`gamepad_key`, `BoundGamepad`)

<!-- b:256 -->

**Gamepad-routed action-bar slots (`planning/features/done/gamepad_action_bar_slots.md`)** —
`ActionSlotDef.gamepad_key: Option<String>` (parsed via `InputMap::parse_gamepad_button` at scene
load into `ActionSlotUi.resolved_gamepad_button`, same call site as `resolved_key`) lets a slot
also fire from gamepad, alongside its existing keyboard `key`. The two devices resolve
**differently** — keyboard is shared hardware (`key` fires from the one global
`ButtonInput<KeyCode>` regardless of `owner_player`, unchanged pre-existing behavior); a gamepad is
not shared the same way, so `gamepad_key` only fires from the **owning player's own** resolved
`BoundGamepad` (`bound.0.and_then(|e| gamepad_query.get(e).ok())` — see `docs/dev/gamepad-routing.md`;
post-`gamepad_player_binding_hardening.md`, this is no longer a live positional `resolve_gamepad`
lookup). `action_bar_input_system`'s query widened again to include `&BoundGamepad` and a new
`Query<&Gamepad>`. The fast-path skip (`!keyboard_fired &&
resolved_gamepad_button.is_none()`) preserves the exact perf profile and cooldown-event behavior of
every keyboard-only slot — the gamepad check, and the owning-player lookup it requires, only ever
run for slots that actually declare `gamepad_key`. The keyboard cooldown-gate-before-player-lookup
ordering is preserved byte-for-byte; a second, symmetric cooldown check runs after player
resolution to cover a gamepad-only fire (which couldn't be checked earlier, since it needs the
owning player's own `gamepad_index`) — the two checks are mutually exclusive on `keyboard_fired`, so
neither double-emits. `action_bar_visual_system` is untouched (cost/cooldown-driven only, no input
reads, `owns_slot`'s signature unchanged). Collision detection needs a **second, separately-scoped**
pass distinct from the existing scene-wide keyboard check: `gamepad_key` isn't part of the
intent/cooldown pipeline's key space at all, so the risk isn't cross-bar pipeline entanglement —
it's a same-player double-fire (one physical press activating 2 slots for the same player). Both
`scene_loader.rs::warn_same_player_gamepad_duplicate_slots` and `ironhold_cli validate`'s matching
check key by `(owner_player.unwrap_or(0), GamepadButton)` — the same "`None`/`Some(0)` both mean the
primary player" normalization `owns_slot`/`warn_missing_player_stat_templates` already use — so two
*different* players sharing a button name (each has their own physical pad) is correctly not
flagged.

## Mouse-click action-bar slots (`Option<Ref<Interaction>>`, `InspectorEnabled`)

<!-- b:284.ref -->

**Mouse-click action-bar slots (`planning/features/done/action_bar_mouse_click.md`)** — every slot is already a `Button`, so Bevy tracks an `Interaction` on it; `action_bar_input_system` treats the press edge as a third fire source next to keyboard and gamepad. Detection is per-entity, inside that system: its slot query is `(&ActionSlotUi, Option<Ref<Interaction>>)` and `click_fired = i.is_changed() && *i == Interaction::Pressed` — `Ref::is_changed()` compares against the system's own last run, so it is the edge detector (once per press; not while held, not on release) with no `Local` and no extra resource. **The `Option` is load-bearing:** many tests spawn a bare `ActionSlotUi` with no `Button`/`Interaction`, and a required `Ref<Interaction>` would silently drop them from the query. **Do not replace this with a `ClickedSlots`-style resource keyed by slot-key string** (the rejected first design): a stale key re-fires every frame and survives `LoadScene`, and two bars sharing a key would both fire from one click. Click and keyboard collapse into the same `keyboard_fired` decision (both are device-independent, gated before the owning player is resolved), so a click plus a key press on one frame activates once. A click acts for the slot's own `owner_player` (`owns_slot`) regardless of which split viewport the cursor is in — the mouse is shared hardware like the keyboard, unlike `click_select_system`'s viewport-based routing. Clicks (not key presses) are ignored while `InspectorEnabled` is true (bevy_egui and `ui_focus_system` both read the raw cursor with no arbitration, so a click reaches both stacks whichever draws on top; same gate as `button_system`). `click_select_system` already skips world targeting while any `Interaction` is `Pressed`, so a slot click never changes the world target; the bar's own root node is `FocusPolicy::Pass` with no `Interaction`, so clicks on padding between slots still fall through to world targeting. Known gaps shared with every other clickable UI node: `camera_orbit_system` and `input_translator_system`'s strafe (default `strafe_mouse_button: Left`) have no UI-hover guard. In tests, drive a click by writing `Interaction::Pressed` once after a warm-up update (never re-write it each frame — that counts as a new change); see `entity_logic_tests.rs`'s `test_click_*`.
