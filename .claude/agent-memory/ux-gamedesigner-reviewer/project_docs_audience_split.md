---
name: docs-audience-split
description: 2026-10-07 plan to move 6 dev docs to docs/dev/ (core_claude_md_split Phase A); which docs are actually mixed; where designer-facing CLI + WASM warmup content lives/should live
metadata:
  type: project
---

Plan `planning/features/core_claude_md_split.md` Phase A (reviewed 2026-10-07 at 1d616bc): designer docs keep paths
(00, 05, 20, 25, 30, STATUS); 10/40/50/60/70/browser_tests move to `docs/dev/` with 3-line stubs; new `docs/README.md`.

Mixed-audience facts found (check if still true before citing):
- `docs/60_contributing.md` "CLI tooling" (~138-345) is the ONLY home of `ironhold validate/watch/inspect glb|audio` docs
  and the full "Checks performed" list; docs/20 (`state_machine_path` row ~106, RON-parse-error section ~3880) sends
  designers there. Moving 60 to dev/ without extracting this strands a designer tool. Same file (~246) says
  "a WASM-only designer (no CLI access)" — whether designers have the CLI at all is an unresolved persona question.
- `docs/00_overview.md`: "Getting started in 5 minutes" is the best designer onboarding; but "Repository layout",
  stale "Current implementation snapshot", "Planned next steps" are dev; "Where to read next" lists 10/50.
- `docs/30` header says "Design Doc (vision)" with planned/milestone sections + Rust type names; designer content is
  "Project logic: state_machine.ron" and "Entity FSM".
- `docs/70_profiling.md` "Web build is slow"/DevTools section is designer-usable.
- index.html/play.html contain NO docs links at all; `.nojekyll` exists so docs/*.md are served raw on Pages.

WASM first-use stall content: particle pipeline warmup + ParticleBudget interaction is NOT in designer docs
(docs/20 ~1761 cites an undefined "standard pattern"; docs/25 "Pipeline warmup" covers flame only, uses `Some((..))`,
names FlameParticleMaterial). Designer-terms variant mapping already exists: docs/20 material table ~1861-1863.
Preload timing conflicts: docs/20/30 say "fire on scene.ready", core CLAUDE.md says playing-state entry_actions.
SFX rules (WAV, trim silence) already covered in docs/20 ~1918-1928.

Related: [[project-pkg-rebuild-required]], [[project-validate-coverage-gaps]]
