#!/usr/bin/env bash
# invoke-delegation.sh
# Example script for invoking mux-delegation contract functions on a Soroban network.
#
# Usage:
#   bash scripts/invoke-delegation.sh grant  <contract_id> <owner> <delegate> <perm1,perm2,...> [--network testnet]
#   bash scripts/invoke-delegation.sh revoke <contract_id> <owner> <delegate> [--network testnet]
#   bash scripts/invoke-delegation.sh query  <contract_id> <owner> <delegate> [--network testnet]
#   bash scripts/invoke-delegation.sh list   <contract_id> <owner> [--network testnet]

set -euo pipefail

ACTION="${1:?Usage: invoke-delegation.sh <grant|revoke|query|list> ...}"
CONTRACT_ID="${2:?Missing contract ID}"
OWNER="${3:?Missing owner address}"
NETWORK="${NETWORK:-testnet}"

# Parse optional --network flag from trailing args
for arg in "$@"; do
  case "$arg" in
    --network) shift; NETWORK="${1:-testnet}"; shift ;;
  esac
done

case "$ACTION" in
  grant)
    DELEGATE="${4:?Missing delegate address}"
    PERMS_CSV="${5:?Missing permissions (comma-separated symbols)}"
    IFS=',' read -ra PERM_ARR <<< "$PERMS_CSV"
    PERM_JSON="["
    for i in "${!PERM_ARR[@]}"; do
      [ "$i" -gt 0 ] && PERM_JSON+=","
      PERM_JSON+="{\"symbol\":\"${PERM_ARR[$i]}\"}"
    done
    PERM_JSON+="]"

    echo "Granting permissions [${PERMS_CSV}] from ${OWNER} to ${DELEGATE}..."
    stellar contract invoke \
      --id "$CONTRACT_ID" \
      --source "$OWNER" \
      --network "$NETWORK" \
      -- grant_delegate \
      --owner "$OWNER" \
      --delegate "$DELEGATE" \
      --permissions "$PERM_JSON"
    ;;

  revoke)
    DELEGATE="${4:?Missing delegate address}"
    echo "Revoking delegation from ${OWNER} to ${DELEGATE}..."
    stellar contract invoke \
      --id "$CONTRACT_ID" \
      --source "$OWNER" \
      --network "$NETWORK" \
      -- revoke_delegate \
      --owner "$OWNER" \
      --delegate "$DELEGATE"
    ;;

  query)
    DELEGATE="${4:?Missing delegate address}"
    echo "Querying permissions for delegate ${DELEGATE} under ${OWNER}..."
    stellar contract invoke \
      --id "$CONTRACT_ID" \
      --source "$OWNER" \
      --network "$NETWORK" \
      -- get_delegate_permissions \
      --owner "$OWNER" \
      --delegate "$DELEGATE"
    ;;

  list)
    echo "Listing all delegates for ${OWNER}..."
    stellar contract invoke \
      --id "$CONTRACT_ID" \
      --source "$OWNER" \
      --network "$NETWORK" \
      -- get_delegates \
      --owner "$OWNER"
    ;;

  *)
    echo "Unknown action: $ACTION"
    echo "Valid actions: grant, revoke, query, list"
    exit 1
    ;;
esac
