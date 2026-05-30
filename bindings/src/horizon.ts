import { SorobanRpc } from "@stellar/stellar-sdk";

/** Milliseconds between each poll attempt. */
const POLL_INTERVAL_MS = 1_500;

/** Maximum number of poll attempts before giving up (~30 s at 1.5 s interval). */
const MAX_ATTEMPTS = 20;

export class TransactionTimeoutError extends Error {
  constructor(public readonly hash: string) {
    super(`Transaction ${hash} not confirmed after ${MAX_ATTEMPTS} attempts`);
    this.name = "TransactionTimeoutError";
  }
}

export class TransactionFailedError extends Error {
  constructor(
    public readonly hash: string,
    public readonly resultXdr: string
  ) {
    super(`Transaction ${hash} failed on-chain`);
    this.name = "TransactionFailedError";
  }
}

/**
 * Poll the Soroban RPC `getTransaction` endpoint until the transaction is
 * confirmed (SUCCESS), fails (FAILED), or the attempt limit is reached.
 *
 * @returns The confirmed transaction response.
 * @throws  TransactionFailedError   if the network reports FAILED.
 * @throws  TransactionTimeoutError  if MAX_ATTEMPTS is exhausted.
 */
export async function pollTransaction(
  server: SorobanRpc.Server,
  hash: string
): Promise<SorobanRpc.Api.GetSuccessfulTransactionResponse> {
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const response = await server.getTransaction(hash);

    if (response.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
      return response as SorobanRpc.Api.GetSuccessfulTransactionResponse;
    }

    if (response.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
      const resultXdr =
        (response as SorobanRpc.Api.GetFailedTransactionResponse).resultXdr?.toXDR("base64") ?? "";
      throw new TransactionFailedError(hash, resultXdr);
    }

    // NOT_FOUND or still PENDING — wait and retry
    await sleep(POLL_INTERVAL_MS);
  }

  throw new TransactionTimeoutError(hash);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
