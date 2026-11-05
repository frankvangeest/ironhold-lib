---
name: backend-dev-stack-singleton-and-scan
description: backend/up.sh dev Nakama is one compose project (name: ironhold-dev) shared by every worktree; its no-arg project scan is a line grep, not a RON parse
metadata:
  type: project
---

Found reviewing feature/backend_dev_stack (2026-11-05):
- `backend/docker-compose.yml` hardcodes `name: ironhold-dev`, so the primary checkout and every `../ironhold-lib-{slug}` worktree drive the SAME stack. up.sh in worktree B sees A's running nakama (running=1), restarts it with A's bind mount, and then `up -d` recreates it pointing at B/backend/modules. down.sh -v from any worktree deletes everyone's local DB.
- up.sh's no-arg scan is `grep '^[[:space:]]*backend_path:'`: `backend_path: None,` or a line inside a `/* */` block matches, then the CLI's `load()` hard-errors for that project and the whole run aborts (all-or-nothing `collect::<Result>`).
- The contract run.sh relies on `-p ironhold-contract` overriding `name:`; COMPOSE_PROJECT_NAME in the env is lower precedence than -p, but up.sh/down.sh pass no -p, so an exported COMPOSE_PROJECT_NAME would retarget them.
- backend/.env reads are blocked by a hook (credential-file rule) even though it only holds image pins; verify pins via the plan/README instead.

**How to apply:** when reviewing backend/ scripts or contract tooling, check cross-worktree collisions and scan-vs-parse divergence first. Related: [[cli-plain-ron-from-str-implicit-some]].
