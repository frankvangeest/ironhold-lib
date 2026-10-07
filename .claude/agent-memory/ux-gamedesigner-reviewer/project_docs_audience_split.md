---
name: docs-audience-split
description: core_claude_md_split Phase A (docs/dev/ move + docs/15_authoring_tools.md + docs/README.md); Pages raw-markdown trap, cross-refs as code spans, Getting-started persona mismatch
metadata:
  type: project
---

Plan `planning/features/core_claude_md_split.md` Phase A. Plan reviews at 1d616bc / dd62494; implementation
reviewed 2026-10-07 on `feature/docs_audience_split`.

Shipped shape (verified 2026-10-07): designer docs keep paths (00, 05, 15, 20, 25, 30, STATUS); 10/40/50/60/70/
browser_tests moved to `docs/dev/` with "# Moved" stubs that all point at docs/README.md (60 stub also -> 15; 70 stub
-> `dev/70_profiling.md#browser-devtools--gpu-timing-web`, anchor correct). Gallery "Docs" nav (index/assets/play.html)
uses the github.com blob/main/docs/README.md URL, target=_blank. No designer doc or asset RON still cites old paths
or "Checks performed". docs/15 diagnostic codes all verified present in CLI/core source.

Remaining review findings (check if still true next time):
- Cross-refs in docs/15 and 00_overview "Where to read next" are `code spans`, not links -> dead on GitHub render,
  which is now the Docs-nav landing surface. docs/README.md itself uses real links (good).
- docs/README "Making a game (no Rust needed)" routes to 00 "Getting started in 5 minutes", which assumes a repo
  clone + `cp`/`python serve.py`/`cargo run`; nothing tells a hosted-WASM-only designer how to run their own project.
- docs/15 §1 "Getting the tool" is cargo-only (no prebuilt CLI); says "Ask an engineer".
- docs/15 jargon leftovers: `crates/ironhold_cli`, `Action::Spawn`, `PrefabDef`, "undefined behaviour".
- Root README doc table now lists 05 and dev/70 (old omission fixed); dev/70's designer-usable DevTools section is
  flagged only in docs/README and the 70 stub, not in root README.
- `.project.ron` parse failure still hangs the loading screen (backlog Bugs); docs/15's "RON typo shows up as an
  asset-load error" is console-true but never mentions the stuck spinner.

Related: [[project-pkg-rebuild-required]], [[project-validate-coverage-gaps]], [[project-ron-comments-cite-dev-paths]]
