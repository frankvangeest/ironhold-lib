---
name: backend-dev-stack-plan-review
description: backend_dev_stack plan+impl review 2026-11-05 - local Nakama under backend/; doc checklist delivered; open nits = CLI-build link from dev section, CARGO_TARGET_DIR-unset cp, "web build cannot" overbroad
metadata:
  type: project
---

Plan reviewed 2026-11-05 (needs more doc design); impl docs reviewed same day: all plan items delivered.

Resolved at impl: default up.sh set now includes fake:// projects (all with backend_path), printed command carries
NAKAMA_SERVER_KEY=defaultkey; prerequisites first in backend/README.md; docs/20 ~5132 one-sentence pointer + prereqs,
links only dev/backend_dev_stack.md#run-a-game-against-it; up.sh fail() messages match backend/README troubleshooting rows;
"tested version pinned in backend/.env" at docs/20 ~5101; docs/15 ~97 links ../backend/README.md.

Open nits (impl review): dev "Run a game against it" section says "needs the cached CLI" with no link to how to build it
(the docs/20 reader lands there); backend/README cp line breaks if CARGO_TARGET_DIR unset; dev doc "the web build cannot use
a local server" is broader than true (a local serve.py with edited endpoint can); "no project with a backend_path" error
not in the troubleshooting table; port check covers only 7350.

**Why:** first local-server workflow; docs/20 points designers at a dev page.
**How to apply:** if backend/ docs change again, re-check the docs/20 -> dev anchor and the up.sh message/table parity.
Related: [[public-backend-demo-plan-review]], [[backend-abstraction-plan-review]], [[project_docs_audience_split]].
