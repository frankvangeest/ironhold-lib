---
name: ui-set-visible-pattern
description: Action::SetUiVisible + UiNodeId (UI-id lookup, Display::None) — touchpoints, empty-id runtime match footgun, validate-message whitespace trap, plan-vs-impl gaps found 2026-10-12
metadata:
  type: project
---

`Action::SetUiVisible { id, visible }` (feature/ui_set_visible, reviewed 2026-10-12) is the first
UI-node-by-scene-id action. `UiNodeId(String)` lives in `runtime/scene_manager/mod.rs` and is
inserted in all 13 `UiNodeDef` arms of `spawn_ui_element_node`; the executor iterates
`SceneStateParams.ui_nodes_q` (only `&mut Node` query in scene_manager) and writes `Display`.

**Footguns to check on any follow-up (Group v2, responsive_ui Phase 3, initially_hidden):**
- ~~Empty-id runtime match~~ FIXED before merge: `action_executor.rs:~812` now `warn!`s and skips
  on `id.is_empty()` (verified 2026-10-13). Keep the guard if the arm is refactored.
- Still open (verified 2026-10-13): overlays never fire `scene.ready` (`scene_loader.rs:~1332`
  `if !is_overlay`), so the start-hidden recipe doesn't work for overlay UI; ActionBar cannot be
  hidden at all (validate-rejected).
- validate.rs message literals were written with `\`-newline continuations that got flattened into
  runs of ~27 spaces inside the string (designer-visible). Grep new CLI messages for `"  {5,}`.
- `action_needs_target` (action_bar.rs) and `substitute_self_in_action` (dialogue.rs) still use `_`
  wildcards; the plan asked for explicit pass-through arms that were not added.
- `Action::is_presentation_only()` (schema/actions.rs) doc says "cosmetic UI/audio/camera feel" but
  only SetUiVisible returns true (CameraShake/PlaySound/ShowFloatingText are false). Consumer is
  responsive_ui Phase 3 `--strict`; recheck classification when it lands.
- Overlay ids: validate only sees overlays found by `discover_extra_scenes` from `collect_actions`,
  which skips ActionBar slot do_actions, so an overlay opened only from a slot is invisible to the
  unknown-id check (see [[validate-cross-file-blind-spots]]).

**Positive reference:** guarded `if node.display != target` write, no ActionQueue push from a
capability, exhaustive `rewrite_self`/`rewrite_target` arms, own slot-action scan + `walk_ui_nodes`
nesting, every-occurrence panel/ActionBar rejection. Related: [[ui-flex-group-walker-pattern]],
[[ui-trigger-reachability-pattern]].
