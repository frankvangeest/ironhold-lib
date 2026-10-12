# Feature: `SetUiVisible` action (show/hide a UI node by id)

_Status: Done (2026-10-12)_
_Planned at: `e3a2ff2` (2026-10-11)_

Origin: plan-review of `planning/features/responsive_ui.md` (rounds 1-3). The architect recommended
splitting this primitive out because it is a standalone engine action that the orientation work
**and** Flex `Group` v2 / `mobile_ui_demo` v2 ("runtime show/hide of a `Group` by UI id") both need,
and because testing it alone separates its failures from orientation failures. Its own review
(system-architect + ux-gamedesigner-reviewer, 2026-10-11) confirmed the design and asked for the
fixes now folded in below.

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
`SetEntityVisible(entity:, visible:)`; every doc example uses it. `id` is the UI node's own `id:`
field (the one a `Button`/`Label`/`Group` declares), **not** a spawn id. A hidden `Group` stops taking
space in flex layout (absolute-positioned leaves never took flex space either way; say so in the
docs). An id-less `Group` (legal) cannot be targeted.

There is no toggle form: one action sets one state. A toggle needs FSM states or two rules (see the
`ui_demo` demo below).

## Why

No UI-id-based visibility action exists: `SetEntityVisible` only looks up the spawn registry, and
`docs/20_data_formats.md` (~3215-3220) says a scene-authored UI `Label` cannot be hidden in v1.
Overlays are not a substitute (one global slot, `UnloadOverlay` despawns every overlay, each adds a
click-blocking backdrop). This is the missing primitive behind touch-only controls, portrait/landscape
HUD swaps (`responsive_ui.md` Phase 3) and Group v2.

## Approach

- **`Action::SetUiVisible { id: String, visible: bool }`**, shows/hides a UI node by its `id`; a
  `Group` hides its whole subtree. It flows through the normal pipeline (message -> interpreter ->
  `ActionQueue` -> executor); it is cosmetic, UI-only and safe for determinism.
- **Hide mechanism is single: `Display::None`.** A plain `Visibility::Hidden` child still takes flex
  space (docs trip-up #10). Nothing in core writes `Node.display` today, so there is no competing
  writer, and no conflicting `&mut Node` query exists in `scene_manager/` (architect).
  - **Why clicks stop working:** `ui_focus_system` only checks `InheritedVisibility` (`focus.rs:250`),
    but a `Display::None` node has a zero-size `ComputedNode`, so `contains_point` fails and it is
    never hit-tested (`picking_backend.rs:185` says the same). Record this as the reason it works;
    it is not because the node counts as invisible.
  - **Guarded write:** `if node.display != target { node.display = target }` (the crate's
    change-detection rule). Each real toggle causes one relayout of one tree, which is cheap. Restore
    with `Display::Flex` (nothing sets any other value today).
- **id -> entity lookup does not exist today** (`spawn_ui_element_node`, `scene_loader.rs:1788`,
  attaches only a `Name`, and the Label arm's `Name` is its text, `:1883`). Add a `UiNodeId(String)`
  component inserted once per node in **every** arm (all 13 `UiNodeDef` variants,
  `scene_v2.rs:367-382`, `id()` at `:679-691`: Group, Rect, Label, Button, IconButton, StatBar,
  StatSpread, StatRadar, the four panels, ActionBar). Every arm spawns exactly one root entity from
  its `node`, and nested Group children recurse through `build_child_node` back into the same function
  (`:1855-1860`), so nesting is covered. Insert only for non-empty ids. The `StatRadar` arm skips
  spawning when its material handle is missing (`:2179`), so no `UiNodeId` exists and the runtime skips
  silently; that is fine. The executor arm queries `(&UiNodeId, &mut Node)` through a params struct.
- **Every matching entity is affected.** Ids are unique *per scene*, not across a scene plus an open
  overlay (overlay UI roots spawn alongside the base scene's root, `scene_loader.rs:1232-1325`). The
  executor applies to **every** entity whose `UiNodeId` matches, base scene and overlay alike; document
  it.
- **Timing: author initial hides on `scene.ready:<scene>`.** UI is spawned later by `spawn_scene_v2`
  (deferred), so a `SetUiVisible` in the same action list as `LoadScene`/`LoadSceneOverlay` hits
  nothing, silently (a test asserts this). `SceneEvent::Ready` fires after the scene's UI exists, so
  `( event: "scene.ready:main", do_actions: [SetUiVisible(id: "station_b", visible: false)] )` is the
  "start hidden" recipe; whether it shows one frame of the node first is measured in the test below
  and documented either way. **Overlays emit no `Ready`** (`scene_loader.rs:1332`), so there is no
  start-hidden recipe for overlay nodes yet; document it.
- **Lifecycle.** `UiNodeId` is removed by the recursive despawn of the `LevelEntity`/`OverlayEntity`
  UI root. Hidden state is **not** kept across `LoadScene`, and an overlay comes back visible when
  reopened. Document it.
- **Rejected targets (validate).** A `SetUiVisible` is rejected when the target itself is, or its
  subtree contains, a dialogue / inventory / shop / container panel or an `ActionBar`. Reasons:
  action-bar key/gamepad slots fire whether or not the bar is visible, and two bars sharing keys is
  already a `cross_bar_duplicate_key` error; panels open and close by writing `Visibility`
  (`action_executor.rs:1264-1289`) and `panels_open`, so a `Display::None` ancestor would show nothing
  yet leave interact / pickup / tab-target locked. (`panel_nested_in_group`, `scene_v2.rs:573-592`, is
  only a Warning, so panels and bars can legally sit inside Groups; the subtree check is needed.)
  Messages:
  - ActionBar: "SetUiVisible target 'hud_landscape' (scene main) contains ActionBar 'skill_bar'; move
    the bar outside the Group. Its keys keep firing while hidden."
  - Panel: "SetUiVisible target 'X' (scene S) is/contains InventoryPanel 'Y'; panels open and close with
    OpenInventory/CloseInventory etc., not SetUiVisible."
  - **Documented limit:** an `ActionBar` cannot be hidden at runtime yet (cutscene, menu); say so in
    `docs/20` and log a backlog item.
- **Unknown ids.** `global_on` is project-wide, so a missing id is *expected* in scenes without that
  HUD: the runtime skips it silently (debug log only). `validate` catches typos instead: an id passes
  if it exists in **any** scene or overlay `ui:` tree of the project, scanning nested Groups with
  `walk_ui_nodes` (several existing checks only scan the top level). `cross_file_checks` already
  holds every scene (`scenes/*.scene.ron` plus overlays found via `LoadSceneOverlay`/`ToggleOverlay`),
  so this is feasible there. Because an id may occur in several scenes, the panel/`ActionBar` rejection
  checks **every** occurrence. Duplicate ids in one scene are already a validate error
  (`GameSceneV2::validate`, `scene_v2.rs:168-182`, allows an empty id only on a `Group`).
  - **Scan sources.** `collect_actions` (`validate.rs:807`) covers state machines, behaviours and
    dialogue choices **only**, not `ActionBar` slot `do_actions` (`scene_v2.rs` ~1465). Decision: the
    `SetUiVisible` checks use their own helper that also scans slot `do_actions` in every scene; fixing
    `collect_actions` for **all** action checks is out of scope (it could surface new errors in existing
    projects) and is logged as its own backlog item.
- **`{self}`/`{target}` substitution:** a UI id is not substituted. Sites the compiler forces:
  `action_executor_system`, `rewrite_self`, `rewrite_target` (no wildcard) and CLI `query.rs:543`.
  Silent wildcards to check by hand and give an explicit pass-through: `substitute_self_in_action`
  (`other => other`), `action_needs_target` (`_ => false`), and the `validate.rs` per-action loops
  (~907 and ~3122).
- **`initially_hidden` is not in this plan.** Whether it is needed depends on the `responsive_ui.md`
  flash check, which needs the Phase 3 orientation swap that only exists after this plan ships. If a
  flash shows, `responsive_ui.md` Phase 3 adds it as an extra field on `Group`/leaf nodes using this
  plan's `Display::None` mechanism (default `false`, unlike the panel field's `true`).
- **Presentation-only classification.** `Action::is_presentation_only()` does not exist yet; it is
  introduced here as a shared predicate under `schema/` (an exhaustive `match` with no `_` arm) and
  classifies `SetUiVisible` as presentation-only; `responsive_ui.md` Phase 3 consumes it for its
  `--strict` check.
- **CLI:** the new variant must appear in `ironhold query actions`; `cargo check -p ironhold_cli`.

## Docs

- `docs/20_data_formats.md`: Actions table (~3928-3965); rewrite the ~3215-3220 passage ("`SetEntityVisible`
  cannot target a scene-authored UI `Label`") to point to `SetUiVisible` (room8 `join_prompt` is the
  natural example); a one-line callout next to the panel `initially_hidden` rows (~1333): "Panels open
  and close only through their Open/Close/Toggle actions. `SetUiVisible` is for HUD nodes and Groups,
  and validate rejects it on panels."; the behaviours below.
- `docs/30_runtime_events_and_logic.md`: the category list (~193-200, **replace** the planned
  `ShowUi(panel_id)` / `HideUi(panel_id)` placeholder so designers do not see two names for one thing)
  and the implementation list (~355-380).
- `docs/STATUS.md` (~105). That is four designer surfaces, not five; `docs/15_authoring_tools.md`
  needs only the new validate-check rows.
- Behaviours to state: bound labels inside a hidden Group keep updating; a `scene_key_bindings` /
  `global_key_bindings` entry that emits the same trigger as a hidden Button still fires; hidden state
  resets on `LoadScene` and on overlay reopen; the same id in a base scene and an open overlay affects
  both; `id` is the UI node's `id:`, not a spawn id; an `ActionBar` cannot be hidden yet.

## Tasks

- [ ] Schema: `Action::SetUiVisible`, `UiNodeId` component, `Action::is_presentation_only()`, and the
      `{self}`/`{target}` pass-through arms (compiler-forced sites plus the silent wildcards above).
- [ ] Spawner: insert `UiNodeId` in all 13 arms (non-empty ids only).
- [ ] Executor arm: guarded `Display::None`/`Display::Flex` write on every matching entity, silent skip
      on unknown id, params struct.
- [ ] Validate: id exists in any scene/overlay (nested) via `cross_file_checks`, panel and
      `ActionBar` target/subtree rejection across every occurrence, both messages above, slot
      `do_actions` scanned by the plan's own helper.
- [ ] Tests (follow `tests/ui_panel_blocker.rs`, which adds `UiPlugin`; without a UI camera and window
      target `ComputedNode` stays zero and a layout test passes without testing anything):
      hide/show a Group subtree, flex space collapses (siblings reflow), absolute leaves unaffected,
      same id in base scene and an open overlay (both toggle), `SetUiVisible` after `LoadScene` in one
      list does not apply to the new scene, unknown id skipped, id-less Group untouched, hidden Button
      not hit-tested, bound labels in a hidden Group keep updating, one-frame-flash measurement for the
      `scene.ready` start-hidden recipe, validate cases (typo, panel target, panel/ActionBar in
      subtree, id only in another scene passes); `cargo check -p ironhold_cli`.
- [ ] Docs: the four surfaces and the behaviours above; `docs/15` validate rows; crate/`schema/`
      `CLAUDE.md`.
- [ ] Demo: in `ui_demo`'s `top_bar` add two Buttons, `Hide B` and `Show B` (ids `hide_b_btn` /
      `show_b_btn`, placed in the existing Add/Reset Group; every non-Group node needs a non-empty
      unique id), with `global_on` handlers on `ui.button_pressed:hide_b` / `ui.button_pressed:show_b`
      (the engine prefixes a button's action with `ui.button_pressed:`) running
      `SetUiVisible(id: "station_b", visible: false/true)`.
      `station_b` is a good target: the `stations` row is centre-justified, so A and C visibly close the
      gap. Keep `ui_demo`'s default state and baseline unchanged (the buttons are additive, so its
      baseline needs a deliberate, minimal update; capture only `ui_demo` with `--real-gpu`).
- [ ] Backlog: (1) "an `ActionBar` cannot be hidden at runtime" (needs the "flow inside a Group" item or a
      dedicated action); (2) "`collect_actions` does not scan `ActionBar` slot `do_actions`, so slot
      actions skip every action-level validate check".

## Open questions

- Does Group v2 want a `toggle` form (`ToggleUiVisible`)? Not needed for orientation; decide when
  `mobile_ui_demo` v2 is planned.

## Acceptance criteria

- Given a `Group` id'd `hud_a` containing leaves, when `SetUiVisible(id: "hud_a", visible: false)` runs,
  then the whole subtree disappears, is not hit-tested, and siblings reflow into the freed space;
  `visible: true` restores it.
- Given the same id in the base scene and an open overlay, then both toggle.
- Given `( event: "scene.ready:main", do_actions: [SetUiVisible(id: "station_b", visible: false)] )`,
  then `station_b` is hidden once the scene is ready, and whether it was drawn for one frame first is
  measured and documented.
- Given `SetUiVisible` after `LoadScene` in the same action list, then the new scene is unaffected.
- Given an id present in some other scene only, then validate passes and the runtime skips it silently here.
- Given a typo'd id present in no scene or overlay (including slot `do_actions`), then `ironhold validate`
  errors.
- Given a target that is, or whose subtree contains, a panel or an `ActionBar`, then validate errors with
  the matching drafted message.
- Given `ui_demo`, then the `Hide B` / `Show B` buttons hide and restore `station_b`.
