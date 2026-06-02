#!/usr/bin/env bash
# local-invoke.sh
# Run the local invoke helper using the bindings package.
# Usage:
#   bash scripts/local-invoke.sh --contract-name mux-account --function owner --secret-key S...
#   bash scripts/local-invoke.sh --contract-id C... --function initialize --secret-key S... --arg '{"type":"address","value":"G..."}'

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}/bindings"

if [[ ! -d "node_modules" ]]; then
  echo "Error: Node dependencies are not installed. Run 'cd bindings && npm ci' first."
  exit 1
fi

npm run local-invoke -- "$@"
