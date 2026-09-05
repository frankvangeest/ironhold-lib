---
name: reverse-reachability-false-positive-asymmetry
description: validate.rs's reverse reachability check (orphan_rule) inverts every failure mode of the forward one (unreachable_trigger) — an unenumerated emitter or an unparsed input file becomes a false positive instead of a silent miss
metadata:
  type: project
---

`check_ui_trigger_reachability` (forward: button -> is any rule handling it?) and
`check_orphan_ui_rules`/`collect_reachable_ui_triggers` (reverse: rule -> can any button fire
it?) look symmetric but their error behaviour is inverted. **Anything the reverse direction
fails to enumerate becomes a false-positive warning; the same omission in the forward direction
is only a silent miss.**

Three concrete consequences, all verified 2026-09-05 on `feature/orphan_rule_check`:

1. **Unenumerated emitters.** The complete set of `UiAction::Trigger(...)` construction sites is
   `scene_loader.rs` (Button/IconButton, and the four hardcoded panel buttons),
   `action_executor.rs` (`buy_item:{item_key}`, sourced only from `PrefabDef.merchant.stock` —
   there is no scene-level merchant override), and **`capabilities/dialogue.rs`
   (`dialogue_choice:{n}`)**. The forward check documents dialogue choices as "no false-positive
   risk from that surface" — that reasoning is forward-only and does not carry over.
2. **`{self}` templating.** Event patterns are `{self}`-substituted at match time
   (`binding.event.replace("{self}", id)` in `entity_fsm_interpreter_system`); `{target}` is
   applied to Actions only, never to event patterns. A templated `ui.button_pressed:{self}_x`
   can *never* appear in a reachable set built from concrete strings, so the reverse check
   false-positives unconditionally, whereas the forward one only misfires if a matching concrete
   button also exists.
3. **Parse-failure cascades.** The forward check is gated on `logic_files_parsed_cleanly`
   because it depends on logic files. The reverse check depends on **scenes and the prefab
   catalog** as well, and those are not gated: one malformed `.scene.ron` drops every button in
   it from the reachable set and floods the output with orphan warnings for live rules.

**Why:** this is the generalisable trap, not a one-off — the same inversion applies to any
future "defined but nothing can reach it" check (orphan effects, orphan spawn points, orphan
dialogue nodes).

**How to apply:** when reviewing or adding a reverse/orphan check, enumerate every *producer*
exhaustively (grep the constructor of the runtime component, not the schema), skip any authored
string containing `{`, and gate the check on clean parses of *every* input the reachable set is
built from — not just the one the forward check happened to need. Related:
[[logic-file-on-disk-is-not-loaded]], [[validate-reference-checks-token-blind]],
[[validate-hardcoded-source-file-literals]].
