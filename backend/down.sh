#!/usr/bin/env bash
# LOCAL DEVELOPMENT ONLY. Stops the local Nakama stack started by backend/up.sh (the one stack per machine, project ironhold-dev).
#   backend/down.sh       # stop; accounts and scores are kept in the database volume
#   backend/down.sh -v    # stop and DELETE the database volume: all local accounts and scores are gone
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
cd "$here/.."
unset NAKAMA_PORT NAKAMA_CONSOLE_PORT NAKAMA_MODULES COMPOSE_PROJECT_NAME

case "${1:-}" in
  "") flag=""; note="Stopped. Local accounts and scores are kept (backend/down.sh -v deletes them)." ;;
  -v) flag="-v"; note="Stopped and deleted the database volume: all local accounts and scores are gone." ;;
  *) echo "usage: backend/down.sh [-v]" >&2; exit 2 ;;
esac

command -v docker >/dev/null 2>&1 || { echo "backend/down.sh: Docker is not installed." >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "backend/down.sh: Docker is not running. Start Docker Desktop first." >&2; exit 1; }

# $flag is deliberately unquoted: empty means no argument (and avoids an empty-array expansion under `set -u` on old Bash).
# shellcheck disable=SC2086
docker compose -p ironhold-dev -f backend/docker-compose.yml --env-file backend/.env down $flag
echo "$note"
