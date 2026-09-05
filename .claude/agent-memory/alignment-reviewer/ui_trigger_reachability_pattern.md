---
name: ui-trigger-reachability-pattern
description: The complete map of what fires `ui.button_pressed:{trigger}` — 4 runtime emit sites, 4 UiEvent consumers — and which surfaces validate.rs's forward (check_ui_trigger_reachability) and reverse (orphan_rule) checks cover vs. miss
metadata:
  type: project
---

Established reviewing `feature/ui_trigger_reachability_check` (2026-09-04, verdict ALIGNED).
Reference map for any future work on UI triggers, `validate` coverage, or `UiEvent`.

**Only 4 places in the whole engine construct `UiEvent::ButtonPressed`** (grep
`UiEvent::ButtonPressed(` — the `(` matters, it filters out the 4 read sites and the doc hits):
1. `lib.rs::button_system` — scene `UiNodeDef::Button`, trigger = `UiAction::Trigger` component.
2. `lib.rs::icon_button_click_system` — scene `UiNodeDef::IconButton`, same component.
3. `runtime/input.rs::global_input_system` — `LoadedKeyBindings` (= `ProjectConfig.
   global_key_bindings` overlaid per-key by `GameSceneV2.scene_key_bindings`). Value used **raw,
   no `ui.` stripping**. `InGame` state only.
4. `runtime/input.rs::unclaimed_gamepad_trigger_system` — `LoadedGamepadBindings` (=
   `global_unclaimed_gamepad_bindings` / `scene_unclaimed_gamepad_bindings`). Also raw value.

`UiAction::Trigger` itself is constructed in 8 places: the 2 scene_loader button arms
(`strip_prefix("ui.").unwrap_or(&action)` — the *only* place the `ui.` prefix is stripped), plus
**5 engine-hardcoded panel triggers the designer must still write rules for**:
`close_inventory` / `close_shop` / `close_container` / `take_all_from_container` (scene_loader.rs
~2358/2534/2662/2760) and `buy_item:{item_key}` (action_executor.rs ~1439, one per
`MerchantDef.stock[]`), plus `dialogue_choice:{n}` (dialogue.rs, consumed internally).

**Only 4 systems read `UiEvent`**: the 3 interpreters (`message_interpreter_system`,
`fsm_interpreter_system`, `entity_fsm_interpreter_system`) and `dialogue_tick_system` (which
consumes only the `dialogue_choice:` prefix). Every interpreter match is **exact string equality**
— no wildcards, no prefix matching — after `{self}` substitution *in the `on:` pattern itself*
(entity FSMs only, message_interpreter.rs:152/166/181). So a `HashSet<String>` of raw `on:`
strings is a faithful "handled" oracle; the `{self}`-in-pattern case can only cause false
*negatives*, never false positives.

**`check_ui_trigger_reachability` (validate.rs ~1005) covers sites 1-3 and correctly excludes
`dialogue_choice` and ActionBar slots** (`ActionSlotDef.do_actions` is inline, never a UiEvent).
**Known gaps as of this review** — both purely additive, neither produces new failures on any
shipped project:
- **Site 4, the unclaimed-gamepad maps, is not checked.** Identical shape to `global_key_bindings`
  (~8 lines to add). Live surface: `local_coop_demo/scenes/room8.scene.ron:54`
  `scene_unclaimed_gamepad_bindings: {"South": "join"}` → `rules.ron:58`.
- **UPDATE 2026-09-05:** the unclaimed-gamepad gap is now CLOSED — the forward check covers all 4
  emit sites.
- **The 5 hardcoded panel triggers are not checked**, and they are the *higher*-value catch: the
  designer never types the trigger string, so there's nothing to grep for. Derivable at design
  time from the presence of `UiNodeDef::InventoryPanel`/`ShopPanel`/`ContainerPanel` in a scene and
  from `MerchantDef.stock[].item_key` for `buy_item:*`. `3rd_person_game_demo/logic/
  state_machine.ron:150-172` is the reference wiring.

**The reverse direction shipped as `feature/orphan_rule_check` (2026-09-05, `--strict` kind
`orphan_rule`, verdict ALIGNED):** `collect_reachable_ui_triggers` (union across scenes) +
`check_orphan_ui_rules`/`check_fsm_orphans`/`check_orphan_event`. It mirrors the forward check's 6
sites exactly **and additionally** treats the 5 panel triggers as reachable when the matching
`InventoryPanel`/`ShopPanel`/`ContainerPanel` node is in some scene's `ui` (+ `buy_item:{item_key}`
from every `PrefabDef.merchant.stock[]`) — without that, `3rd_person_game_demo`'s live rules
false-positive. **Key asymmetry: an exclusion that is harmless in the forward direction becomes a
false positive in the reverse one.** Two remain open there:
- `dialogue_choice:{n}` (dialogue.rs:~280) is a real, rule-reachable event (`dialogue_tick_system`
  doesn't consume it — EventReaders are independent), but is not in the reachable set.
- Behavior-file `on:`/`event:` patterns are `{self}`-substituted (message_interpreter.rs
  152/166/181 — **only** those three; scene-level rules/FSM are not), so a
  `ui.button_pressed:{self}_open` binding would be flagged. `check_orphan_event` has no `{`-skip.

**`scene.ui` is a flat `Vec<UiNodeDef>` with no nesting** and `ui_panel` is styling-only — so a
single non-recursive loop over `scene.ui` is a complete button enumeration (verified 2026-09-05).
Panels are only ever spawned from `scene.ui`, never dynamically by `OpenShop`/`OpenContainer`, so
gating the panel triggers on node presence matches the runtime exactly.

**Gate-placement trap:** both directions are gated on `logic_files_parsed_cleanly`, but the reverse
check's noise source is a **scene** parse failure (`do_validate:~1952` silently drops an unparseable
scene from `scenes`, shrinking the reachable set), not a logic one — a logic parse failure yields
*zero* rules and therefore zero orphan warnings. Gate direction is per-check; don't copy it blindly.

**Drift risk to watch:** the `strip_prefix("ui.")` one-liner now exists twice (scene_loader.rs:1739
/1765 and validate.rs:1042/1046). The codebase's established fix for exactly this is to hoist a
predicate into `schema/` so both crates share it (`PrefabDef::is_flycam()` precedent, see
[[diagnostic-only-feature-pattern]]) — e.g. `impl ButtonDef { pub fn trigger(&self) -> &str }`.

**Structural note:** `collect_handled_events` (`cli/commands/utils.rs`) re-reads and re-parses
rules/state_machine/behaviors from disk even though `do_validate` already has all three parsed in
locals (it hands them to `collect_actions` two lines earlier). Consequence: a parse error in
`rules.ron` makes `collect_handled_events` silently see zero handlers, so the real one-line parse
error is buried under one bogus `unreachable_trigger` per button. There are now **three** parallel
logic-file walkers (`validate::collect_actions`, `utils::collect_handled_events`,
`query::collect_logic`) — see [[validate-cross-file-blind-spots]].

**`crates/ironhold_cli/tests/validate_projects.rs` only smoke-validates 9 of the 17 shipped
`*.project.ron`s** — missing `blank_project`, `camera_modes`, `dynamic_animation_control`,
`foliage_demo`, `integration_tests` (x3 project files), `stats_demo`. Pre-existing, but it means
"we ran the new check against every shipped project" is not something the test suite enforces.
