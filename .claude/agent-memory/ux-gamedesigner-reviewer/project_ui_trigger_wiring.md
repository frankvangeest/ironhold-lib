---
name: ui-trigger-wiring
description: How a Button/key-binding trigger reaches a rule (ui.button_pressed:{trigger}), which doc surfaces describe it, and the blind spots of the `unreachable_trigger` validate check
metadata:
  type: project
---

**Four** authoring surfaces all emit the *same* `UiEvent::ButtonPressed` → `"ui.button_pressed:{trigger}"`
event, and a designer must hand-author a matching `on:`/`event:` string somewhere in
`logic/rules.ron`, `logic/state_machine.ron`, or `behaviors/*.behavior.ron` or the click is silently
dropped:

| Surface | Trigger derivation |
|---|---|
| `UiNodeDef::Button.action` | `strip_prefix("ui.")` — `"ui.dance"` and `"dance"` both give `dance` |
| `UiNodeDef::IconButton.action` | same |
| `ProjectConfig.global_key_bindings` / `GameSceneV2.scene_key_bindings` | value used **raw**, no `ui.` stripping |
| `global_unclaimed_gamepad_bindings` / `scene_unclaimed_gamepad_bindings` | value used raw |

Both `ui.`-prefixed and bare `action:` values ship (`effect_mayhem_demo` uses bare `"btn_mayhem"`,
everything else uses `"ui.*"`), so the prefix is decorative — never flag a bare one as wrong.

`message_interpreter.rs::match_rules` is **exact string equality**, no wildcard/prefix matching
anywhere. So a set-membership check is sound; there is no "catch-all rule" idiom to worry about.

## `unreachable_trigger` validate check — known blind spots

Shipped 2026-09-04 (`check_ui_trigger_reachability` in `validate.rs`). Covers Button, IconButton,
`global_key_bindings`, `scene_key_bindings`. Blind spots worth re-checking on any follow-up:

- **Gamepad bindings are NOT covered** (`*_unclaimed_gamepad_bindings`) — deliberate scope call in
  the feature plan, but the same dead-trigger bug class. Canonical usage is `local_coop_demo`
  room8 `{"South": "join"}`.
- **State gating is not modelled.** A rule with `when: "menu"`, an FSM `on:` inside one state, or a
  transition with `from:` all count as "handled" — so validate can pass while the button is still
  dead in the state the player is actually in.
- **`behaviors/` is scanned non-recursively.** `PrefabDef.behavior` accepts any project-relative
  path, so a behavior file in a subfolder makes its handled events invisible → false positives.
- **A parse failure in `rules.ron`/`state_machine.ron` empties the handled set** (`silent_parse`
  swallows the error), producing one bogus "button will do nothing" error per button on top of the
  real parse error.

Related: [[validate-coverage-gaps]], [[ron-parse-failure-diagnostics]], [[docs-lag-actions]].
