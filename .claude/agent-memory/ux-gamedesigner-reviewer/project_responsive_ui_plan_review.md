---
name: project-responsive-ui-plan-review
description: responsive_ui plan-review r1-r3 + Phase 2 ui_scale impl review (2026-10-11) - invalid ui_scale only error!-logged at load then still used (min>max -> f32::clamp panic); snap-down vs min_scale false warning; Percent vs px scaling undocumented
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
  coords. ui_demo has no ActionBar, so the limit is never exercised.
- No drafted message text for the panel/ActionBar rejection; behaviour for duplicate ids in one scene unspecified.

Phase 2 (`ProjectConfig.ui_scale`) implementation review, 2026-10-11 (feature/responsive_ui):
- Docs good: docs/20 table row ~117, example comment ~139, section "Responsive UI: `ui_scale`" ~4737; anchors
  #group (explicit <a id>) and #responsive-ui-ui_scale resolve.
- BLOCKER found: project_loader.rs only `error!`s on ProjectConfig::validate() failure and continues;
  capabilities/ui_scale.rs reads the raw config, so min_scale > max_scale hits f32::clamp panic (crash) and
  reference (0,0) gives NaN/inf. docs/15 row claims consequence "zero or inverted scale". Designers without CLI hit it.
- snap-down happens BEFORE clamp: min_scale not a multiple of snap (e.g. 0.6) floors early and fires a false
  "UI will overflow" warning. Warning suffix "give this aspect its own layout" is not actionable until phase 3.
- Undocumented: Percent()/flex sizes follow the window while px/position scale; absolute-position UI scales from
  the top-left (ultrawide/off-aspect leaves an empty strip); 844x390 snaps 0.66 -> 0.5 (22px text -> 11px).

**Why:** responsive UI touches every HUD; these are the designer traps most likely to ship.
**How to apply:** on later phase reviews, re-check the blocker fix and that layout switching (phase 3) resolves the
warning wording. Related: [[project-ui-label-button-font-and-clip]], [[project-world-label-legibility]],
[[project-ui-nesting-flat-scan-sites]], [[project-same-frame-event-order]], [[project-docs-lag-actions]],
[[project-warn-vs-silent-fallback-principle]].
