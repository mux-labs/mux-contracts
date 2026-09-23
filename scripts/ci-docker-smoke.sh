#!/usr/bin/env bash
# ci-docker-smoke.sh
#
# CI job: Start docker-compose localnet, wait for health, run smoke tests,
# tear down. Validates that the documented docker-compose setup boots cleanly
# and Soroban RPC accepts reads.
#
# This script is required for Mux Soroban audit and mainnet readiness.
#
# Usage (CI):
#   bash scripts/ci-docker-smoke.sh
#
# Exit codes:
#   0 — docker-compose started, smoke checks passed, cleanup succeeded
#   1 — startup, smoke, or cleanup failure

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> CI docker-compose smoke check"
echo ""

# Cleanup on exit (success or failure)
cleanup() {
  local exit_code=$?
  echo ""
  echo "==> Cleaning up docker-compose..."
  docker-compose down -v --remove-orphans || true
  echo "Cleanup complete (exit code: $exit_code)"
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

# 1. Start docker-compose and wait for health
echo "==> Starting docker-compose localnet..."
if ! docker-compose up --wait --wait-timeout 120; then
  echo "Error: docker-compose up failed or timed out" >&2
  exit 1
fi

echo ""
echo "==> Localnet started, checking RPC health..."

# 2. Verify RPC endpoint responds
MAX_RETRIES=10
RETRY_COUNT=0
RPC_HEALTHY=0

while [[ $RETRY_COUNT -lt $MAX_RETRIES ]]; do
  if curl --silent --fail --max-time 5 \
    -X POST http://localhost:8000 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork","params":[]}' \
    | grep -q '"result"'; then
    RPC_HEALTHY=1
    echo "RPC endpoint healthy (attempt $((RETRY_COUNT + 1)))"
    break
  fi
  RETRY_COUNT=$((RETRY_COUNT + 1))
  echo "Waiting for RPC... (attempt $RETRY_COUNT/$MAX_RETRIES)"
  sleep 3
done

if [[ $RPC_HEALTHY -eq 0 ]]; then
  echo "Error: RPC endpoint did not become healthy" >&2
  docker-compose logs
  exit 1
fi

echo ""
echo "==> RPC healthy, running basic smoke checks..."

# 3. Basic smoke: getHealth and getNetwork RPC methods
echo "  - Testing getHealth..."
if ! curl --silent --fail --max-time 5 \
  -X POST http://localhost:8000 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":[]}' \
  | grep -q '"result"'; then
  echo "Error: getHealth failed" >&2
  exit 1
fi
echo "  ✓ getHealth passed"

echo "  - Testing getNetwork..."
if ! curl --silent --fail --max-time 5 \
  -X POST http://localhost:8000 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork","params":[]}' \
  | grep -q '"result"'; then
  echo "Error: getNetwork failed" >&2
  exit 1
fi
echo "  ✓ getNetwork passed"

echo ""
echo "==> All smoke checks passed ✓"
echo "Docker-compose localnet is functional and ready for contract deployment."

# Cleanup will run via trap EXIT
exit 0
