#!/usr/bin/env bash
# Contract test against a local, throwaway Nakama (needs Docker). Not part of the normal test run or any hosted CI:
#   crates/ironhold_nakama/tests/contract/run.sh
# It copies the module under test next to the fixture config and starts the backend/ stack under its own project name
# (ironhold-contract) and its own ports (7360/7361), so a dev stack started with backend/up.sh keeps running, with its data,
# next to it. The Nakama and Postgres versions are pinned in backend/.env. It runs the ignored tests in tests/contract.rs and
# tears its own stack down again.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
cd "$repo"

# Compose resolves a relative mount against the compose file's folder, and Docker Desktop wants a Windows-style path:
# pass the fixture folder as an absolute path (`pwd -W` is Git Bash's; plain `pwd` elsewhere).
modules="$(cd "$here/modules" && (pwd -W 2>/dev/null || pwd))"
port=7360
export NAKAMA_PORT="$port" NAKAMA_CONSOLE_PORT=7361 NAKAMA_MODULES="$modules"

compose() { docker compose -p ironhold-contract -f backend/docker-compose.yml --env-file backend/.env "$@"; }

cp crates/ironhold_nakama/server/ironhold.lua "$here/modules/ironhold.lua"
# -p on every call, the cleanup included: without it `down -v` would delete the dev stack's database volume.
trap 'compose down -v >/dev/null 2>&1 || true; rm -f "$here/modules/ironhold.lua"' EXIT

# A run that was killed hard never reached the trap, and its named volume would carry old accounts into this one: start clean.
compose down -v >/dev/null 2>&1 || true

echo "starting the contract Nakama on 127.0.0.1:$port..."
compose up -d --wait --wait-timeout 240 || { compose logs --tail 40 nakama; exit 1; }

NAKAMA_URL="http://127.0.0.1:$port" NAKAMA_SERVER_KEY=defaultkey \
  cargo test -p ironhold_nakama --test contract -- --ignored --nocapture --test-threads=1
