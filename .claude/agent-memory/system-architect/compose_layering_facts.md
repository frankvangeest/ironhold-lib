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


Implementation review (2026-11-05, feature/backend_dev_stack, uncommitted):
- Project-name precedence is `-p` > `COMPOSE_PROJECT_NAME` env > top-level `name:`; a script relying on `name:` alone can be redirected by an exported env var. Shell env also beats `--env-file` values (NAKAMA_PORT etc. leak in).
- `"${arr[@]}"` on an empty array under `set -u` errors on bash < 4.4 (macOS /usr/bin/env bash = 3.2); Git Bash is 5.x so Windows hides it. Use `${arr[@]+"${arr[@]}"}`.
- ironhold_native's only feature is `inspector`, so `--all-features` in a printed run command = an egui build nobody asked for.
- `ironhold backend config --out` validates first, then writes games file, then the EMBEDDED ironhold.lua; comparing that written copy to the source is an mtime-free staleness signal for the cached CLI.
- Tracked `.yml`/`.sh` at that point: only `.woodpecker.yml` (index LF, worktree CRLF) and contract run.sh (LF). `.githooks/*` are CRLF in the worktree and extensionless, so `*.sh eol=lf` does not cover them.

**How to apply:** any future compose layering, contract-test rework or script-on-Windows review. Related: [[nakama-spike-facts]], [[backend-abstraction-review-facts]].
