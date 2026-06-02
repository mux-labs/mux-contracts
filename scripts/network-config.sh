#!/usr/bin/env bash
# ==============================================================================
# scripts/network-config.sh
#
# Network configuration loader for Mux Protocol deployment scripts.
# Sources network passphrase and RPC URL from config/networks.toml.
#
# Usage (source into other scripts):
#   source scripts/network-config.sh
#   load_network_config testnet      # sets NETWORK_PASSPHRASE, RPC_URL, etc.
#
# Or standalone:
#   bash scripts/network-config.sh testnet   # prints resolved config
# ==============================================================================

NETWORKS_TOML="${NETWORKS_TOML:-$(dirname "${BASH_SOURCE[0]}")/../config/networks.toml}"

SUPPORTED_NETWORKS=("testnet" "mainnet" "futurenet" "local")

# Inline TOML parser for simple [section] / key = "value" format.
# Avoids requiring an external TOML dependency.
_parse_toml_section() {
  local file="$1" section="$2" key="$3"
  awk -v section="[$section]" -v key="$key" '
    /^\[/ { in_section = ($0 == section) }
    in_section && $1 == key { gsub(/^[^=]*=\s*"?|"?\s*$/, ""); print; exit }
  ' "$file"
}

# load_network_config <network>
# Exports: NETWORK_PASSPHRASE, NETWORK_RPC_URL, NETWORK_HORIZON_URL,
#          NETWORK_FRIENDBOT_URL, NETWORK_NATIVE_ASSET_ISSUER
load_network_config() {
  local network="${1:-testnet}"

  [ -f "$NETWORKS_TOML" ] || {
    echo "ERROR: config/networks.toml not found at $NETWORKS_TOML" >&2
    return 1
  }

  local valid=false
  for n in "${SUPPORTED_NETWORKS[@]}"; do
    [ "$n" = "$network" ] && valid=true && break
  done
  [ "$valid" = true ] || {
    echo "ERROR: Unknown network '$network'. Supported: ${SUPPORTED_NETWORKS[*]}" >&2
    return 1
  }

  NETWORK_PASSPHRASE=$(_parse_toml_section         "$NETWORKS_TOML" "$network" "passphrase")
  NETWORK_RPC_URL=$(_parse_toml_section             "$NETWORKS_TOML" "$network" "rpc_url")
  NETWORK_HORIZON_URL=$(_parse_toml_section         "$NETWORKS_TOML" "$network" "horizon_url")
  NETWORK_FRIENDBOT_URL=$(_parse_toml_section       "$NETWORKS_TOML" "$network" "friendbot_url")
  NETWORK_NATIVE_ASSET_ISSUER=$(_parse_toml_section "$NETWORKS_TOML" "$network" "native_asset_issuer")

  [ -z "$NETWORK_PASSPHRASE" ] && {
    echo "ERROR: passphrase not found for network '$network' in $NETWORKS_TOML" >&2
    return 1
  }

  export NETWORK_PASSPHRASE NETWORK_RPC_URL NETWORK_HORIZON_URL \
         NETWORK_FRIENDBOT_URL NETWORK_NATIVE_ASSET_ISSUER
}

# Standalone mode: print resolved config for the given network
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  NETWORK="${1:-testnet}"
  load_network_config "$NETWORK"

  echo "Network:       $NETWORK"
  echo "Passphrase:    $NETWORK_PASSPHRASE"
  echo "RPC URL:       $NETWORK_RPC_URL"
  echo "Horizon URL:   $NETWORK_HORIZON_URL"
  echo "Friendbot URL: $NETWORK_FRIENDBOT_URL"
  echo "Native Issuer: $NETWORK_NATIVE_ASSET_ISSUER"
fi
