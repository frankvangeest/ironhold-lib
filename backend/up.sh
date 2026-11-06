#!/usr/bin/env bash
# LOCAL DEVELOPMENT ONLY: starts a throwaway Nakama + Postgres on 127.0.0.1 with Nakama's default keys. Never deploy this.
#   backend/up.sh                 # every project under assets/projects/ that has a backend_path
#   backend/up.sh backend_demo    # only the named projects (folder names under assets/projects/)
# Needs: Docker Desktop running, the cached CLI (tools/bin/ironhold, see root CLAUDE.md), Git Bash on Windows.
# Re-running while the stack is up regenerates modules/ and restarts only the nakama container; the database is never touched.
# There is ONE local stack per machine (fixed ports, project name ironhold-dev): whichever worktree ran up.sh last owns its modules.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
cd "$repo"   # everything below is relative to the repo root, whatever folder the script was started from

# The scripts own these settings: a value exported in your shell must not change which stack or ports they act on.
export NAKAMA_PORT=7350 NAKAMA_CONSOLE_PORT=7351
unset NAKAMA_MODULES COMPOSE_PROJECT_NAME

fail() { echo "backend/up.sh: $*" >&2; exit 1; }
compose() { docker compose -p ironhold-dev -f backend/docker-compose.yml --env-file backend/.env "$@"; }
listening() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

# 1. Docker first, before anything is generated or written.
command -v docker >/dev/null 2>&1 || fail "Docker is not installed. Install Docker Desktop, start it, and run this again."
docker info >/dev/null 2>&1 || fail "Docker is installed but not running. Start Docker Desktop, wait until it reports running, and run this again."

running=0
if [ -n "$(compose ps --status running -q nakama 2>/dev/null)" ]; then
  running=1
else
  for port in "$NAKAMA_PORT" "$NAKAMA_CONSOLE_PORT"; do
    if listening "$port"; then
      fail "something is already listening on 127.0.0.1:$port (another Nakama?). Stop it, or run backend/down.sh if it is this stack, and try again."
    fi
  done
fi

# 2. The cached CLI (it validates the projects and writes the game list). A relative IRONHOLD_CLI is relative to the repo root.
cli="${IRONHOLD_CLI:-tools/bin/ironhold}"
if [ ! -f "$cli" ] && [ -f "$cli.exe" ]; then cli="$cli.exe"; fi
[ -f "$cli" ] || fail "the cached CLI is missing ($cli). Build it once from the repo root:
  cargo build -p ironhold_cli --release
  mkdir -p tools/bin && cp \"\${CARGO_TARGET_DIR:-target}/release/ironhold.exe\" tools/bin/ironhold.exe   (drop .exe outside Windows)"

# 3. Which projects: folder names under assets/projects/. With no arguments, every project whose own .project.ron sets a backend_path
#    (a string, not None). The CLI validates them and rejects duplicate game_ids; a fake:// project is included on purpose,
#    because a game must be on the server's list before it can sign in.
projects=()
if [ "$#" -gt 0 ]; then
  for arg in "$@"; do
    name="${arg%/}"; name="${name%\\}"; name="${name##*[/\\]}"
    [ -d "assets/projects/$name" ] || fail "no project folder assets/projects/$name (give folder names under assets/projects/)."
    projects+=("assets/projects/$name")
  done
else
  for dir in assets/projects/*/; do
    name="$(basename "$dir")"
    if grep -qE '^[[:space:]]*backend_path:[[:space:]]*"' "assets/projects/$name/$name.project.ron" 2>/dev/null; then
      projects+=("assets/projects/$name")
    fi
  done
fi
[ "${#projects[@]}" -gt 0 ] || fail "no project with a backend_path found under assets/projects/."

# Generate into a temporary folder first, so a failure or Ctrl-C can never leave a half-written or stale backend/modules/.
tmp="$(mktemp -d "${TMPDIR:-/tmp}/ironhold-modules.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
"$cli" backend config "${projects[@]}" --out "$tmp"
# The cached CLI embeds the module at its build time; the source file is what a local run must test.
if ! cmp -s "$tmp/ironhold.lua" crates/ironhold_nakama/server/ironhold.lua; then
  echo "warning: the cached CLI is stale (its embedded server module differs from crates/ironhold_nakama/server/ironhold.lua)." >&2
  echo "         The source module is used. Rebuild the CLI (see 'What you need first' in backend/README.md) so the game list matches the current schema." >&2
fi
cp crates/ironhold_nakama/server/ironhold.lua "$tmp/ironhold.lua"
mkdir -p backend/modules
cp "$tmp/ironhold_games.lua" "$tmp/ironhold.lua" backend/modules/

# 4. Start (or restart only nakama).
if [ "$running" -eq 1 ]; then
  echo "The stack is already up: restarting nakama to load the regenerated modules (the database is untouched)."
  compose restart nakama >/dev/null
else
  echo "Starting Nakama + Postgres. The first run downloads the images (about 200 MB), so it can take a few minutes."
fi
compose up -d --wait --wait-timeout 240 || {
  echo >&2
  echo "backend/up.sh: Nakama did not become healthy in time. Last log lines:" >&2
  compose logs --tail 30 nakama >&2 || true
  exit 1
}

first="$(basename "${projects[0]}")"
echo
echo "================================================================="
echo " LOCAL ONLY - default keys - never deploy this"
echo "================================================================="
echo " Projects on this server: $(for p in "${projects[@]}"; do printf '%s ' "$(basename "$p")"; done)"
echo " Client API : http://127.0.0.1:$NAKAMA_PORT   (server key: defaultkey)"
echo " Console    : http://127.0.0.1:$NAKAMA_CONSOLE_PORT   (local-only login: admin / password)"
echo
echo " Run a game against it (native; the hosted web build cannot use a local server):"
echo "   NAKAMA_SERVER_KEY=defaultkey cargo run -p ironhold_native -- --project $first --backend http://127.0.0.1:$NAKAMA_PORT"
echo
echo " Stop: backend/down.sh      Stop and delete all local accounts and scores: backend/down.sh -v"
