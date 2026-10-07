---
name: docs-audience-split
description: core_claude_md_split Phase A (docs/dev/ move + new docs/15_authoring_tools.md + docs/README.md); mixed-audience facts, Pages raw-markdown trap, "Checks performed" citation trap
metadata:
  type: project
---

Plan `planning/features/core_claude_md_split.md` Phase A. 1st review at 1d616bc, 2nd review at dd62494 (2026-10-07).
Designer docs keep paths (00, 05, 20, 25, 30, STATUS); 10/40/50/60/70/browser_tests move to `docs/dev/` with stubs;
CLI section of docs/60 (~138-345) extracted to designer-side `docs/15_authoring_tools.md` (Frank: some designers use the CLI).

Traps found in 2nd review (check if still true):
- **"Checks performed" citations**: root CLAUDE.md Tools table, tools/asset_checker/CLAUDE.md:7, active plans
  attacker_identity_on_hit_events.md:391, ui_flex_group.md:491 cite "docs/60 Checks performed" -> a mechanical A2
  rewrite to docs/dev/60 points at a page that no longer has the list; must go to docs/15.
- **Gallery "Docs" link on GitHub Pages**: `.nojekyll` -> docs/*.md served RAW; a relative link to docs/README.md shows
  plain text with dead links. Use the github.com/.../blob/main/docs/README.md URL, target=_blank (play.html: don't kill
  the running game). test_web.py always uses ?testing=1 (bar hidden) -> no baseline risk. assets.html has a 3rd nav.
- **No prebuilt CLI distribution exists** -> docs/15 "how to get it" is cargo-only; persona open question.
- Docs say both `ironhold_cli validate` (~81x in docs/20) and `ironhold validate`; docs/15 must equate them.
- Only 4 designer-doc refs into moved docs: docs/20:106, docs/20:3881 (60), docs/30:313 (40), 00_overview:174/177.
- docs/30 header "Design Doc (vision)"; designer anchors `#project-logic-state_machineron`, `#entity-fsm-beta-04`.
- STATUS "Engine ABI (today)" is dev; "Feature Matrix", "Project Logic", "UI v1 Scope (authoring)" are designer.
- docs/70 "Browser DevTools" section designer-usable; backlog item links designers into docs/dev/70 on purpose.
- Root README doc table omits 05_art_style and 70_profiling.

Older facts: 00_overview "Getting started in 5 minutes" is the best onboarding but needs local serve.py + pkg/;
particle warmup/ParticleBudget + preload timing conflict tracked as backlog item "Designer docs: web loading".

Related: [[project-pkg-rebuild-required]], [[project-validate-coverage-gaps]], [[project-ron-comments-cite-dev-paths]]
