#!/usr/bin/env bash
# Contract test against a local, throwaway Nakama (needs Docker). Not part of the normal test run or any hosted CI:
#   crates/ironhold_nakama/tests/contract/run.sh
# It copies the module under test next to the fixture config, starts Nakama 3.41.0 + Postgres bound to 127.0.0.1, runs the
# ignored tests in tests/contract.rs and tears everything down again.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here"
cp ../../server/ironhold.lua modules/ironhold.lua
trap 'docker compose down -v >/dev/null 2>&1 || true; rm -f modules/ironhold.lua' EXIT
docker compose up -d
echo "waiting for Nakama..."
for _ in $(seq 1 60); do
  if curl -s -o /dev/null -m 2 http://127.0.0.1:7350/healthcheck; then break; fi
  sleep 2
done
curl -s -o /dev/null -m 2 http://127.0.0.1:7350/healthcheck || { docker compose logs nakama | tail -40; exit 1; }
cd ../../../..
NAKAMA_URL=http://127.0.0.1:7350 NAKAMA_SERVER_KEY=defaultkey \
  cargo test -p ironhold_nakama --test contract -- --ignored --nocapture --test-threads=1
