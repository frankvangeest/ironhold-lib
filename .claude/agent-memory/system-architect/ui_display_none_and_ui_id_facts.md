---
name: ui-display-none-and-ui-id-facts
description: Verified facts for SetUiVisible/UiNodeId (plan ui_set_visible.md, 2026-10-11) - Display::None hit-test, overlay id collision, validate action coverage gap, exhaustive Action match sites
metadata:
  type: project
---

Verified 2026-10-11 at 33186ba while plan-reviewing `planning/features/ui_set_visible.md`.

- bevy_ui 0.18 `ui_focus_system` gates on `InheritedVisibility` only; `Display::None` does NOT touch visibility,
  the node just gets a zero-size `ComputedNode` so `contains_point` fails (picking_backend.rs:185 says the same).
  Stale `Interaction` is cleared by the normal not-hovered path, not the visibility reset.
- Nothing in core writes `Node.display` (grep `Display::` hits only `TargetHudDisplay`). Unfiltered `&mut Node`
  queries exist in camera.rs (split label / target HUD) but only write left/top.
- Overlays are ADDITIVE: overlay UI root spawns alongside the base scene's UI root, and `GameSceneV2::validate`
  uniqueness is per scene, so a base-scene id and an overlay id can collide -> an id->entity lookup returns >1.
- SceneEvent::Ready is NOT written for overlays; UI spawns in `spawn_scene_v2` (deferred), so an action queued in the
  same list as LoadScene/LoadSceneOverlay cannot see the new scene's UI nodes.
- validate `collect_actions` covers state_machine + behaviors + dialogue choices only; ActionBar slot `do_actions`
  (scene_v2.rs ~1465) are NOT collected - every action-level check silently skips them (pre-existing gap).
- Exhaustive `match` over Action (no wildcard): executor, `rewrite_self`, `rewrite_target`, CLI `query.rs` action
  kind (~543). Wildcarded (silent): dialogue `substitute_self_in_action` (`other => other`), action_bar
  `action_needs_target` (`_ => false`), validate.rs per-action loops.
- `GameSceneV2::validate` allows empty id only on `Group`; every other UiNodeDef must have a non-empty unique id.
- Headless layout test precedent: `tests/ui_panel_blocker.rs` adds `UiPlugin`.

Related: [[ui-scale-and-overlay-slot-facts]], [[ui-flat-scan-and-taffy-facts]], [[cli-validate-coverage-model]].
