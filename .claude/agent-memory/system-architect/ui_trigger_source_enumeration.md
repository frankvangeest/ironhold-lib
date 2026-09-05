---
name: ui-trigger-source-enumeration
description: The complete set of things that emit UiEvent::ButtonPressed (6 authored sources + 6 engine-generated triggers), why forward (unreachable_trigger) and reverse (orphan_rule) reachability checks have opposite failure-mode polarity, and the re-parse-vs-already-parsed divergence class
metadata:
  type: project
---

**`ui.button_pressed:{trigger}` has six *designer-authored* sources, not four.** Any check or doc
claiming to cover "every UI trigger" must enumerate all six:

1. `GameSceneV2.ui[] :: UiNodeDef::Button.action` — `strip_prefix("ui.")` applied
   (`scene_loader.rs` ~1753)
2. `GameSceneV2.ui[] :: UiNodeDef::IconButton.action` — same stripping (~1792)
3. `ProjectConfig.global_key_bindings` — value used verbatim, **no** `ui.` stripping
   (`runtime/input.rs::global_input_system`)
4. `GameSceneV2.scene_key_bindings` — same, overlays (3) per-key
5. `ProjectConfig.global_unclaimed_gamepad_bindings` — same emission path
   (`runtime/input.rs::unclaimed_gamepad_trigger_system`), only fires on an unclaimed pad
6. `GameSceneV2.scene_unclaimed_gamepad_bindings` — overlays (5) per-key; **live in shipped
   content**: `local_coop_demo/scenes/room8.scene.ron` has `{"South": "join"}`

**Correction (verified 2026-09-05 against `check_ui_trigger_reachability`):** all **six** authored
sources are covered by the shipped forward check — the earlier "5-6 consciously out of scope" note
is stale; both `global_unclaimed_gamepad_bindings` and `scene_unclaimed_gamepad_bindings` are
iterated.

**Engine-generated triggers never appear in scene RON**: `dialogue_choice:{n}`
(`capabilities/dialogue.rs` ~280, spawned as a real `UiAction::Trigger` button so it *does* reach
`message_interpreter` and *can* be matched by a designer rule — it is merely also consumed by
`dialogue_tick_system`), `close_inventory`, `close_shop`, `close_container`,
`take_all_from_container` (`scene_loader.rs` ~2362/2538/2666/2764), `buy_item:{item_key}`
(`action_executor.rs` ~1439). The five panel ones are enumerated by
`collect_reachable_ui_triggers` (reverse check, gated on `InventoryPanel`/`ShopPanel`/
`ContainerPanel` presence + `MerchantDef.stock[]`); `dialogue_choice:{n}` is **not** enumerated
anywhere. The forward check covers none of the six.

**The load-bearing asymmetry: forward and reverse checks have opposite failure-mode polarity over
the *same* enumeration.** `unreachable_trigger` (forward) asks "does this button's event resolve to
a rule"; `orphan_rule` (reverse, `feature/orphan_rule_check` 2026-09-05) asks "does this rule's
`on:` resolve to some button". An **under-approximation of the reachable-trigger set is a benign
false-negative forward, but a harmful false-positive reverse** — it reports correct, live content
as dead. This is why the five panel triggers were harmless to omit forward but had to be added
before the reverse check could ship (it flagged `3rd_person_game_demo`'s real close/buy rules).
Every remaining forward-direction "no false-positive risk, so out of scope" note must be
re-evaluated when copied into a reverse check: `dialogue_choice:{n}` and `{self}` substitution both
flip to false-positive sources. Cheap guard for the substitution class: skip any event containing
`{` in the reverse check.

**Event *handling* is exactly four schema fields, all in `schema/project.rs`:** `LogicRule.on`
(:311), `StateDef.on[].event` via `FsmEventBinding.event` (:137/:153), `FsmTransition.on` (:146).
Small and stable — the "two independent walks will drift" risk is real but low-velocity.

**`{self}` substitution is a latent false-positive source.** `entity_fsm_interpreter_system`
does `binding.event.replace("{self}", spawn_id)` before comparing, so a behavior handling
`ui.button_pressed:{self}_open` would never literal-match a CLI-collected event string. No shipped
behavior handles `ui.button_pressed` today, so this is theoretical — but it is the same
substitution-enumeration trap noted in [[capability-patterns]].

**Divergence class worth flagging on any new `validate.rs` check: re-parsing from disk instead of
using `do_validate`'s already-parsed bindings.** `do_validate` already holds
`rules: Option<LogicRulesAsset>`, `state_machine: Option<StateMachineAsset>`, and
`behaviors: Vec<(String, StateMachineAsset)>` (parse errors already reported via `try_parse` →
`file_results`). A helper that instead takes `project_dir` and re-reads via `utils::silent_parse`
swallows parse errors, so a malformed `logic/*.ron` yields *both* the real parse error *and* a
flood of downstream false errors (3rd_person_game_demo: 16 handlers in rules.ron + 34 in
state_machine.ron feeding 22 buttons). This inverts the convention documented in
[[cli-validate-coverage-model]] (missing/malformed input ⇒ check silently vanishes). The fix shape
is always the same: mirror `collect_actions`' signature — take the parsed structs as parameters.

**Why:** the trigger-source set grew from 2 to 6 over several features without any single place
enumerating it, and each new validate check re-derives its own view of the logic files.

**How to apply:** when reviewing anything touching UI triggers or key/gamepad bindings, check the
six-source list above for completeness; when reviewing any new `validate.rs` check, confirm it
consumes `do_validate`'s parsed values rather than re-reading from `project_dir`; and for any
reverse/orphan-direction check, confirm every "safe to omit" exclusion inherited from the forward
check was re-justified rather than copied.

**Accidental drift guard worth preserving:** the `valid_ui_trigger` fixture exercises all six
authored sources, so `valid_ui_trigger_strict_exits_0` fails if `collect_reachable_ui_triggers`
ever *loses* a site type — this is what makes the deliberate duplication between
`collect_reachable_ui_triggers` and `check_ui_trigger_reachability` tolerable. Adding a *seventh*
site type still diverges silently.
