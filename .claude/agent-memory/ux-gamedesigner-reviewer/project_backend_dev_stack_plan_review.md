---
name: backend-dev-stack-plan-review
description: backend_dev_stack plan-review 2026-11-05 - local Nakama under backend/; up.sh default project set excludes fake:// demos; hosted web build cannot reach it; needs cached CLI + Rust
metadata:
  type: project
---

Plan `planning/features/backend_dev_stack.md` reviewed 2026-11-05: Needs more design work (doc side).

Open items to re-check at impl review:
- up.sh default games = projects with non-fake endpoint -> only backend_demo_online; `--backend http://127.0.0.1:7350` on backend_demo
  then fails "the server rejected the sign-in". up.sh must print a runnable line naming an INCLUDED project + NAKAMA_SERVER_KEY=defaultkey
  (file's server_key is not sent with --backend).
- up.sh needs tools/bin/ironhold (gitignored, local build) -> repo clone + Rust; not a designer-only path. Docs must say so.
- Hosted web build loads its own hosted assets/backend.ron -> cannot target a local server at all; needs local serve.py + edited
  endpoint. docs/20 ~5130 "https page cannot reach http" vs localhost exception needs verifying (Chrome local-network-access prompt).
- No docs/dev index file exists; the index is docs/README.md dev rows.
- Warning must also be in up.sh stdout; Docker-not-installed / port-in-use / slow first pull messages unspecified.

**Why:** first local-server workflow; designers were promised "a designer or engine developer can run up.sh".
**How to apply:** at impl review, run the printed up.sh instructions literally against a fake:// demo. Related:
[[public-backend-demo-plan-review]], [[backend-abstraction-plan-review]], [[project_docs_audience_split]].
