import { Networks } from "@stellar/stellar-sdk";
import type { MuxContractIds } from "./types";

export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contracts: MuxContractIds;
}

/** Well-known Mux Protocol contract deployments. */
export const NETWORK_CONFIGS: Record<string, NetworkConfig> = {
  localnet: {
    rpcUrl: process.env.LOCALNET_RPC_URL || "http://localhost:8000",
    networkPassphrase: process.env.LOCALNET_NETWORK_PASSPHRASE || "Standalone Network ; February 2025",
    contracts: {
      muxAccount: process.env.LOCALNET_MUX_ACCOUNT_ID || "",
      muxBatcher: process.env.LOCALNET_MUX_BATCHER_ID || "",
      muxPermissions: process.env.LOCALNET_MUX_PERMISSIONS_ID || "",
    },
  },
  testnet: {
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: Networks.TESTNET,
    contracts: {
      muxAccount: "",
      muxBatcher: "",
      muxPermissions: "",
    },
  },
  mainnet: {
    rpcUrl: "https://soroban-mainnet.stellar.org",
    networkPassphrase: Networks.PUBLIC,
    contracts: {
      muxAccount: "",
      muxBatcher: "",
      muxPermissions: "",
    },
  },
};
