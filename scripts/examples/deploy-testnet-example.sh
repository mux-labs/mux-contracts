#!/usr/bin/env bash
# ==============================================================================
# scripts/examples/deploy-testnet-example.sh
#
# EXAMPLE — Deploying Mux Protocol contracts to Stellar Testnet.
#
# This file is a walkthrough-style copy of scripts/deploy-testnet.sh with
# extended comments explaining every step. Use it as a reference when setting
# up a new deployment environment or onboarding to the deploy workflow.
#
# DO NOT run this file directly in production. Use scripts/deploy-testnet.sh
# for real deployments (it has the same logic, without the tutorial comments).
#
# Prerequisites
# -------------
#   - Rust + wasm32-unknown-unknown target:  rustup target add wasm32-unknown-unknown
#   - Stellar CLI:  https://developers.stellar.org/docs/tools/stellar-cli
#   - A funded testnet account (run scripts/fund-accounts.sh if needed)
#
# Usage
# -----
#   # 1. Copy the example env file and fill in your values
#   cp scripts/examples/deploy-testnet-example.env .env.testnet
#   # Edit .env.testnet — replace placeholder values with real ones
#
#   # 2. Source the env file so the variables are available in your shell
#   source .env.testnet
#
#   # 3. Dry-run first — confirms everything is wired up before touching the network
#   bash scripts/deploy-testnet.sh --dry-run
#
#   # 4. Real deploy
#   bash scripts/deploy-testnet.sh
# ==============================================================================

set -euo pipefail
# set -e  — exit immediately if any command fails
# set -u  — treat unset variables as errors
# set -o pipefail — catch failures in piped commands, not just the last command

# ------------------------------------------------------------------------------
# Step 1: Load environment variables
#
# All secrets and network config live in a .env file — never hardcode them.
# The .env file must NOT be committed to version control (.gitignore covers it).
# ------------------------------------------------------------------------------
ENV_FILE="${1:-.env.testnet}"   # accept an env file path as the first argument, default to .env.testnet

if [ ! -f "$ENV_FILE" ]; then
  echo "[ERROR] Env file not found: $ENV_FILE"
  echo "        Copy scripts/examples/deploy-testnet-example.env to $ENV_FILE and fill in your values."
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"
echo "[INFO]  Loaded environment from $ENV_FILE"

# ------------------------------------------------------------------------------
# Step 2: Validate required variables
#
# Fail fast here rather than halfway through a deploy.
# ------------------------------------------------------------------------------
: "${MUX_DEPLOYER_SECRET:?MUX_DEPLOYER_SECRET is required — set it in $ENV_FILE}"
: "${MUX_ADMIN_ADDRESS:?MUX_ADMIN_ADDRESS is required — set it in $ENV_FILE}"
: "${MUX_RPC_URL:?MUX_RPC_URL is required — set it in $ENV_FILE}"
: "${MUX_NETWORK_PASSPHRASE:?MUX_NETWORK_PASSPHRASE is required — set it in $ENV_FILE}"

echo "[INFO]  Network:    ${MUX_NETWORK:-testnet}"
echo "[INFO]  RPC URL:    $MUX_RPC_URL"
echo "[INFO]  Admin:      $MUX_ADMIN_ADDRESS"

# ------------------------------------------------------------------------------
# Step 3: Check that required tools are installed
# ------------------------------------------------------------------------------
command -v stellar   >/dev/null 2>&1 || { echo "[ERROR] 'stellar' CLI not found."; exit 1; }
command -v cargo     >/dev/null 2>&1 || { echo "[ERROR] 'cargo' not found. Install Rust: https://rustup.rs"; exit 1; }
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || { echo "[ERROR] sha256sum not found"; exit 1; }

echo "[INFO]  Tool checks passed"

# ------------------------------------------------------------------------------
# Step 4: Build all contracts as release WASM binaries
#
# --target wasm32-unknown-unknown  — cross-compile for the Soroban VM
# --release                        — optimised build (smaller WASM, required for mainnet)
# --workspace                      — build all workspace members at once
#
# The compiled .wasm files land in:
#   target/wasm32-unknown-unknown/release/<crate_name>.wasm
#
# If you only want to deploy a single contract, you can pass --package <name>
# instead of --workspace.
# ------------------------------------------------------------------------------
echo "[INFO]  Building contracts..."
cargo build --target wasm32-unknown-unknown --release --workspace
echo "[OK]    Build complete"

# ------------------------------------------------------------------------------
# Step 5: (Optional) Check WASM file sizes
#
# Stellar enforces a maximum contract size. Run the size check to catch
# oversized WASMs before attempting an upload.
# ------------------------------------------------------------------------------
echo "[INFO]  Checking WASM sizes..."
bash scripts/check-contract-sizes.sh
echo "[OK]    Size check passed"

# ------------------------------------------------------------------------------
# Step 6: Upload the WASM to the network
#
# `stellar contract upload` stores the WASM bytecode on-chain and returns a
# content-addressed hash. Multiple contract instances can share the same upload.
#
# --wasm              — path to the compiled .wasm file
# --source-account    — the secret key paying for the upload transaction
# --rpc-url           — Soroban RPC endpoint
# --network-passphrase — identifies the network (must match the RPC endpoint)
# ------------------------------------------------------------------------------
WASM_PATH="target/wasm32-unknown-unknown/release/mux_account.wasm"

echo "[INFO]  Uploading WASM: $WASM_PATH"
WASM_HASH=$(stellar contract upload \
  --wasm "$WASM_PATH" \
  --source-account "$MUX_DEPLOYER_SECRET" \
  --rpc-url "$MUX_RPC_URL" \
  --network-passphrase "$MUX_NETWORK_PASSPHRASE" \
  2>&1 | tail -1)

echo "[OK]    Uploaded WASM hash: $WASM_HASH"

# ------------------------------------------------------------------------------
# Step 7: Deploy a contract instance
#
# `stellar contract deploy` instantiates the uploaded WASM as a new contract
# and returns its contract ID (a 56-character address starting with C).
#
# --wasm-hash         — the hash returned by the upload step
# --source-account    — the secret key paying for the deploy transaction
# --rpc-url / --network-passphrase — same as upload
# ------------------------------------------------------------------------------
echo "[INFO]  Deploying mux-account..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm-hash "$WASM_HASH" \
  --source-account "$MUX_DEPLOYER_SECRET" \
  --rpc-url "$MUX_RPC_URL" \
  --network-passphrase "$MUX_NETWORK_PASSPHRASE" \
  2>&1 | tail -1)

echo "[OK]    mux-account deployed → $CONTRACT_ID"

# ------------------------------------------------------------------------------
# Step 8: Verify the deployment
#
# Call a read-only function (version) to confirm the contract is alive and
# responding on-chain.
# ------------------------------------------------------------------------------
echo "[INFO]  Verifying contract..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --rpc-url "$MUX_RPC_URL" \
  --network-passphrase "$MUX_NETWORK_PASSPHRASE" \
  --source-account "$MUX_DEPLOYER_SECRET" \
  -- version

echo "[OK]    Contract is live"

# ------------------------------------------------------------------------------
# Step 9: Record the contract ID
#
# Update config/addresses.json with the new contract ID so the rest of the
# toolchain knows where to find this contract. Then commit and push.
#
# In practice, scripts/deploy-testnet.sh does this automatically.
# ------------------------------------------------------------------------------
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "[OK]    Deployment complete"
echo ""
echo "  Contract ID : $CONTRACT_ID"
echo "  WASM Hash   : $WASM_HASH"
echo "  Network     : ${MUX_NETWORK:-testnet}"
echo ""
echo "  Next steps:"
echo "    1. Update config/addresses.json with the contract ID above"
echo "    2. Commit config/addresses.json and open a PR"
echo "    3. Regenerate TypeScript bindings:"
echo "         bash scripts/generate-bindings.sh --network testnet --skip-build"
echo "         cd bindings && npm test"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
