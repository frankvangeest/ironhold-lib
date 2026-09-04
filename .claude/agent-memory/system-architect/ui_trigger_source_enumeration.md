---
name: ui-trigger-source-enumeration
description: The complete set of things that emit UiEvent::ButtonPressed (6 authored sources + 6 engine-generated triggers), and the re-parse-vs-already-parsed divergence class introduced by validate.rs's UI-trigger-reachability check
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

`feature/ui_trigger_reachability_check` (2026-09-04) covers 1-4 only; 5-6 were consciously left out
of scope (noted in the plan's re-verification section), so a typo'd gamepad-join trigger is still
unguarded.

**Engine-generated triggers never appear in scene RON** and must NOT be scanned for (no
false-positive risk, but also no coverage): `dialogue_choice:{n}` (`capabilities/dialogue.rs` ~263,
consumed directly by `dialogue_tick_system`, *not* via rules), `close_inventory`, `close_shop`,
`close_container`, `take_all_from_container` (`scene_loader.rs` ~2358/2534/2662/2760),
`buy_item:{item_key}` (`action_executor.rs` ~1439). The last five *do* go through
rules/FSM matching, so a project that spawns those panels but has no rule for `close_inventory`
has a real dead button that nothing statically checks.

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
consumes `do_validate`'s parsed values rather than re-reading from `project_dir`.
