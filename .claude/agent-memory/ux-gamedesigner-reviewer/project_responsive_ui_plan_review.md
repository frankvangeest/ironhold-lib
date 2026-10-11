---
name: project-responsive-ui-plan-review
description: responsive_ui plan-review rounds 1-3 (2026-10-11) - r1+r2 resolved; r3 open: recipe uses on:/do: (real shape is event:/do_actions:), shared ActionBar/panels stay at landscape absolute px in portrait, ui_demo has no ActionBar
metadata:
  type: project
---

Plan-review of planning/features/responsive_ui.md (Planned at e3a2ff2).

Round 1+2 resolved in round-3 text: struct-form `SetUiVisible(id:, visible:)`; single Display::None hide mechanism
for SetUiVisible and new `initially_hidden` (default false, panels' field stays default true / Visibility);
unknown ids silent at runtime + validate "id exists in ANY scene/overlay" via walk_ui_nodes; ordering
.after(spawn).before(fsm); `Action::is_presentation_only()` list in docs/15; 5 action doc surfaces + STATUS.

Round 3 open:
- Recipe block writes `( on: "...", do: [...] )`. Real global_on/on shape (docs/30 ~217, every state_machine.ron)
  is `( event: "...", do_actions: [...] )`. Plan even hedges "check against docs/30" - fix in plan, not later.
- Validate rejects SetUiVisible on panels / Groups containing ActionBar or panel -> "one shared ActionBar outside
  swapped Groups". ActionBar + panels only have absolute `position:` (no anchor), so in portrait they keep landscape
  coords: 3rd_person main bar (416,580), DialoguePanel 1220 wide overflow a 720-wide portrait_reference. primitive_world
  and 3rd_person mains have NO Groups; ActionBar/panels are top-level siblings. ui_demo (the recipe test project) has
  no ActionBar, so the limit is never exercised. Needs "ActionBar/panels flow inside a Group" as prerequisite or
  honest scoping + both-rects arithmetic in docs.
- No drafted message text for the panel/ActionBar rejection; behaviour for duplicate ids in one scene unspecified.

**Why:** responsive UI touches every HUD; these are the designer traps most likely to ship.
**How to apply:** on the implementation review, verify each bullet was resolved or documented in docs/20/30/15.
Related: [[project-ui-label-button-font-and-clip]], [[project-world-label-legibility]], [[project-ui-nesting-flat-scan-sites]],
[[project-same-frame-event-order]], [[project-docs-lag-actions]].
