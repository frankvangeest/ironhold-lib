---
name: compose-layering-facts
description: Docker Compose multi-file facts that bite the backend/ dev stack + contract override (paths, ports, project name, .env) and repo CRLF state
metadata:
  type: project
---

Facts checked during the backend_dev_stack plan review (2026-11-05, integration 0d58af7):

- With several `-f` files, compose resolves relative paths in ALL files against the FIRST file's directory (project dir), and reads `.env` from the project dir. An override in tests/contract/ saying `./modules` would mount backend/modules, not the fixture.
- `ports:` lists MERGE (append) across files; an override cannot replace 7350 without `!override`/`!reset`. Parameterize the host port (`${NAKAMA_PORT:-7350}`) instead.
- Default project name = project dir basename, so dev stack and a contract run on the same base file share a project; contract run.sh's `trap ... docker compose down -v` would wipe the dev database. Always `-p` / top-level `name:`.
- Repo has no `.gitattributes`, `core.autocrlf=true`; tests/contract/run.sh is CRLF in the working tree. A `.env` or `.sh` that matters to Docker/bash needs `eol=lf`.
- Cached tools/bin/ironhold embeds ironhold.lua at build time (include_str!), so generating modules with it can silently ship a stale module.
- Native `--backend` never sends backend.ron's server_key to the overridden server; local runs need `NAKAMA_SERVER_KEY=defaultkey`.

**How to apply:** any future compose layering, contract-test rework or script-on-Windows review. Related: [[nakama-spike-facts]], [[backend-abstraction-review-facts]].
