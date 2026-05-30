import { Networks } from "@stellar/stellar-sdk";
import type { MuxContractIds } from "./types";

export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contracts: MuxContractIds;
}

/** Well-known Mux Protocol contract deployments. */
export const NETWORK_CONFIGS: Record<string, NetworkConfig> = {
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
