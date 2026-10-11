# Feature: `SetUiVisible` action (show/hide a UI node by id)

_Status: Draft (split out of `responsive_ui.md` on 2026-10-11 by Frank's decision; not yet reviewed as its own plan)_
_Planned at: `e3a2ff2` (2026-10-11)_

Origin: plan-review of `planning/features/responsive_ui.md` (rounds 1-3). The architect recommended
splitting this primitive out because it is a standalone engine action that the orientation work
**and** Flex `Group` v2 / `mobile_ui_demo` v2 ("runtime show/hide of a `Group` by UI id") both need,
and because testing it alone separates its failures from orientation failures. All the detail below
was reviewed as part of the responsive-UI rounds; it still needs its own `/plan-review` pass.

## What

A designer can show or hide any id'd UI node, including a whole `Group` subtree, at runtime from a
state-machine rule:

```ron
( event: "some.event", do_actions: [
    SetUiVisible(id: "hud_portrait",  visible: true),
    SetUiVisible(id: "hud_landscape", visible: false),
] ),
```

RON is always the struct form (`SetUiVisible(id: ..., visible: ...)`), matching
`SetEntityVisible(entity:, visible:)`; every doc example uses it. A hidden `Group` stops taking space
in flex layout (absolute-positioned leaves never took flex space either way; say so in the docs).

## Why

No UI-id-based visibility action exists: `SetEntityVisible` only looks up the spawn registry. Overlays
are not a substitute (one global slot, `UnloadOverlay` despawns every overlay, each adds a
click-blocking backdrop). This is the missing primitive behind touch-only controls, portrait/landscape
HUD swaps (`responsive_ui.md` Phase 3) and Group v2.

## Approach

- **`Action::SetUiVisible { id: String, visible: bool }`**, shows/hides a UI node by its `id`; a
  `Group` hides its whole subtree.
- **Hide mechanism is single: `Display::None`.** A plain `Visibility::Hidden` child still takes flex
  space (docs trip-up #10). Nothing in core writes `Node.display` today, so there is no competing
  writer, and no conflicting `&mut Node` query exists in `scene_manager/` (architect, round 3).
- **id -> entity lookup does not exist today** (`spawn_ui_element_node`, `scene_loader.rs:1788`,
  attaches only a `Name`, and the Label arm's `Name` is its text). Add a `UiNodeId(String)` component
  inserted once per node in **every** arm (all 13 `UiNodeDef` variants, `scene_v2.rs:679-691`: Group,
  Rect, Label, Button, IconButton, StatBar, StatSpread, StatRadar, the four panels, ActionBar). Insert
  only for non-empty ids; id-less Groups stay legal and cannot be targeted. The executor arm queries
  `(&UiNodeId, &mut Node)` through a params struct.
- **Rejected targets (validate).** A `SetUiVisible` is rejected when its target is a dialogue /
  inventory / shop / container panel id, or when its subtree contains an `ActionBar` or any of those
  panels. Reasons: action-bar key/gamepad slots fire whether or not the bar is visible, and two bars
  sharing keys is already a `cross_bar_duplicate_key` error; panels open and close by writing
  `Visibility` (`action_executor.rs:1264-1289`) and `panels_open`, so a `Display::None` ancestor would
  show nothing yet leave interact / pickup / tab-target locked. (`panel_nested_in_group`,
  `scene_v2.rs:573-592`, is only a Warning, so panels and bars can legally sit inside Groups; the
  subtree check is needed.) Message: "SetUiVisible target 'hud_landscape' (scene main) contains
  ActionBar 'skill_bar'; move the bar outside the Group. Its keys keep firing while hidden."
- **Unknown ids.** `global_on` is project-wide, so a missing id is *expected* in scenes without that
  HUD: the runtime skips it silently (debug log only). `validate` catches typos instead: an id passes
  if it exists in **any** scene or overlay `ui:` tree of the project, scanning nested Groups with
  `walk_ui_nodes` (several existing checks only scan the top level). Because an id may occur in
  several scenes, the panel/`ActionBar`-subtree rejection checks **every** occurrence. Duplicate ids in
  one scene are already a validate error (UI ids are unique across nesting).
- **`{self}`/`{target}` substitution:** a UI id is not substituted. The four substitution sites listed
  in `schema/CLAUDE.md` get explicit pass-through arms; `rewrite_self`/`rewrite_target` have no
  wildcard so the compiler forces arms there, but check the dialogue and action-bar sites by hand
  (they can fall through silently).
- **`initially_hidden` is not in this plan.** Whether it is needed depends on the `responsive_ui.md`
  flash spike, which needs the Phase 3 orientation swap that only exists after this plan ships. If that
  spike shows a flash, `responsive_ui.md` Phase 3 adds it as an extra field on `Group`/leaf nodes using
  this plan's `Display::None` mechanism (default `false`, unlike the panel field's `true`).
- **Presentation-only classification.** `Action::is_presentation_only()` (shared predicate under
  `schema/`, an exhaustive `match` with no `_` arm) is introduced with this action and classifies
  `SetUiVisible` as presentation-only; `responsive_ui.md` Phase 3 consumes it for its `--strict` check.
- **CLI:** new variant must appear in `ironhold query actions`; `cargo check -p ironhold_cli`.

## Tasks

- [ ] Plan-review this document on its own (it inherits the responsive-UI review but is a new action).
- [ ] Schema: `Action::SetUiVisible`, `UiNodeId` component, `{self}`/`{target}` pass-through arms.
- [ ] Spawner: insert `UiNodeId` in all 13 arms (non-empty ids only).
- [ ] Executor arm (`Display::None`/`Display::Flex`), silent skip on unknown id, params struct.
- [ ] Validate: id exists in any scene/overlay (nested), panel and `ActionBar`-subtree rejection across
      every occurrence, message text above; `Action::is_presentation_only()` + shared tests.
- [ ] Tests: hide/show a Group subtree, flex space collapses, absolute leaves unaffected, unknown id
      skipped, id-less Group untouched, validate cases; `cargo check -p ironhold_cli`.
- [ ] Docs: `SetUiVisible` row in **all five** action lists in `docs/`, `docs/STATUS.md`, `docs/15`
      (validate codes), crate/`schema/` `CLAUDE.md`.
- [ ] A small demo: extend `ui_demo` with a button that toggles a Group (no orientation needed).

## Open questions

- Does Group v2 want a `toggle` form (`SetUiVisible` vs a `ToggleUiVisible`)? Not needed for
  orientation; decide when `mobile_ui_demo` v2 is planned.

## Acceptance criteria

- Given a `Group` id'd `hud_a` containing leaves, when `SetUiVisible(id: "hud_a", visible: false)` runs,
  then the whole subtree disappears and the siblings reflow into the freed space; `visible: true` restores it.
- Given an id present in some other scene only, then validate passes and the runtime skips it silently here.
- Given a typo'd id present in no scene or overlay, then `ironhold validate` errors.
- Given a target that is a panel, or a `Group` containing an `ActionBar` or panel, then validate errors
  with the drafted message.
