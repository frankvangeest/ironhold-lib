# backend/ - a local Nakama for development

**LOCAL ONLY. Default keys. Never deploy this.** The stack here uses Nakama's default server key (`defaultkey`) and console login, a throwaway
database password and ports bound to `127.0.0.1`. Production and staging servers are configured in the homelab repo, not here.

## What you need first

This is an **engine developer** tool, not a designer path. You need:

- a clone of this repository, and Rust (the native runner and the CLI are built from it)
- [Docker Desktop](https://www.docker.com/products/docker-desktop/), **running**
- Git Bash on Windows (the scripts are Bash)
- the cached CLI: `tools/bin/ironhold` (`.exe` on Windows). Build it once from the repo root:
  ```bash
  cargo build -p ironhold_cli --release
  mkdir -p tools/bin && cp "${CARGO_TARGET_DIR:-target}/release/ironhold.exe" tools/bin/ironhold.exe   # drop .exe outside Windows
  ```

The hosted web build cannot use a local server: a web page gets its server only from the project's `backend.ron`, and the hosted projects point at the
public demo server or at `fake://`. To play against this stack, use the native runner (below). If you only author games in the browser, use `fake://`.

## Run it

```bash
backend/up.sh                  # every project under assets/projects/ that has a backend_path
backend/up.sh backend_demo     # only the named projects
```

`up.sh` checks Docker, generates `backend/modules/` (the game list plus the generic server module, with `ironhold backend config`), starts Nakama and
Postgres and waits until Nakama is healthy. The first run downloads the images (about 200 MB). It ends by printing a complete command, for example:

```bash
NAKAMA_SERVER_KEY=defaultkey cargo run -p ironhold_native -- --project backend_demo --backend http://127.0.0.1:7350
```

`--backend` replaces the endpoint in the project's `backend.ron`, and the key is **not** read from that file when you use it, so the command carries
`NAKAMA_SERVER_KEY`. The console is at `http://127.0.0.1:7351` (login `admin` / `password`, local only).

**Re-running** `backend/up.sh` while the stack is up regenerates `backend/modules/` and restarts only the Nakama container (Nakama cannot reload modules
while it runs). The database is never touched, so adding a leaderboard to a `backend.ron` or a game to the list is one command.

**Stop:** `backend/down.sh` keeps your local accounts and scores; `backend/down.sh -v` deletes them.

## Rules worth knowing

- **A game must be on the server's list before it can sign in.** `ironhold_games.lua` is generated from the projects you name, so a game left out gets
  "the server rejected the sign-in". Re-run `backend/up.sh <project>...` with every project you want (naming some replaces the default set).
- **Each `game_id` may appear once.** Two projects that share a `game_id` make `ironhold backend config` fail; give a local variant of a demo its own
  `game_id`, or name only one of them.
- **One local stack per machine.** The ports and the project name (`ironhold-dev`) are fixed, so several worktrees share the same server and database: whichever ran `backend/up.sh` last owns the loaded modules, and `backend/down.sh -v` from any of them deletes the shared database.
- Arguments are folder names under `assets/projects/` (not paths). With none, a project is included when its own `.project.ron` sets `backend_path` to a file name (a commented-out or `None` entry is skipped).
- A `fake://` project is included on purpose: it is known to the server, so `--backend` can point it at this stack.
- The module is always copied from `crates/ironhold_nakama/server/ironhold.lua` (the source), so a local run tests the current module even if the cached CLI is old.

## Troubleshooting

| You see | Do this |
|---|---|
| `Docker is not installed` / `not running` | install or start Docker Desktop, wait until it reports running, run `backend/up.sh` again |
| `something is already listening on 127.0.0.1:7350` | another Nakama is running; stop it, or run `backend/down.sh` if it is this stack |
| `the cached CLI is missing` | build it once, see "What you need first" |
| warning that the CLI is older than the sources | rebuild it if `backend config` fails or misses a new field |
| `no project with a backend_path found` | add a `backend_path` to the project's `.project.ron`, or name a project that has one |
| `the server rejected the sign-in` | the game is not on the list (run `backend/up.sh <project>`), or the key is missing or wrong (the command needs `NAKAMA_SERVER_KEY=defaultkey`) |
| Nakama did not become healthy | `docker compose -f backend/docker-compose.yml --env-file backend/.env logs nakama` |

## Versions and the contract test

The Nakama and Postgres versions are pinned once, in `backend/.env`. `crates/ironhold_nakama/tests/contract/run.sh` (the contract test of the adapter
and the server module) reuses `backend/docker-compose.yml` under its own project name and ports (7360/7361), so it can run while your dev stack is up and never
touches its database.
