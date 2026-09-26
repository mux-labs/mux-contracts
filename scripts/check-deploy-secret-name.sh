#!/usr/bin/env bash
# check-deploy-secret-name.sh
#
# Guards against docs/#702-class drift: the GitHub Actions secret name wired
# into .github/workflows/deploy.yml's `env:` block must match the env var
# name that scripts/deploy.sh actually reads. If these diverge, the deploy
# workflow silently passes an empty/unset key to the script.
#
# Usage:
#   bash scripts/check-deploy-secret-name.sh
#
# Exit 0 on success, 1 on mismatch or if either name can't be found.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKFLOW="${REPO_ROOT}/.github/workflows/deploy.yml"
DEPLOY_SCRIPT="${REPO_ROOT}/scripts/deploy.sh"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workflow) WORKFLOW="${2:?'--workflow requires a value'}"; shift 2 ;;
    --deploy-script) DEPLOY_SCRIPT="${2:?'--deploy-script requires a value'}"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

echo "==> Checking deploy.yml secret name matches deploy.sh's expected env var"

workflow_name="$(grep -oE '^\s*[A-Z_]+:\s*\$\{\{\s*secrets\.[A-Z_]+\s*\}\}' "$WORKFLOW" \
  | grep -i 'deployer' | head -1 \
  | sed -E 's/^\s*([A-Z_]+):.*$/\1/' || true)"

script_name="$(grep -oE 'DEPLOYER_[A-Z_]+' "$DEPLOY_SCRIPT" | sort -u | head -1 || true)"

if [[ -z "$workflow_name" ]]; then
  echo "  FAIL: could not find a deployer secret env var in ${WORKFLOW}"
  exit 1
fi

if [[ -z "$script_name" ]]; then
  echo "  FAIL: could not find a DEPLOYER_* env var read by ${DEPLOY_SCRIPT}"
  exit 1
fi

echo "  workflow env var: ${workflow_name}"
echo "  deploy.sh expects: ${script_name}"
echo ""

if [[ "$workflow_name" != "$script_name" ]]; then
  echo "ERROR: deploy.yml sets '${workflow_name}' but deploy.sh reads"
  echo "       '${script_name}' — the deploy step will run with an unset key."
  exit 1
fi

echo "OK: deploy.yml and deploy.sh agree on '${workflow_name}'."
