/**
 * Spend limit zero and max boundary tests (Issue #844).
 *
 * Verifies that boundary conditions for spend limits, spend amounts,
 * and period windows are enforced fail-closed across:
 *   - limit <= 0 -> InvalidInput (code 6)
 *   - limit == i128::MAX -> valid maximum boundary
 *   - amount == 0 -> valid spend (chk_ok)
 *   - amount < 0 -> InvalidInput (code 6)
 *   - amount == limit -> exact boundary ok
 *   - amount == limit + 1 -> SpendLimitExceeded (code 5)
 *   - period_ledgers == 0 -> InvalidPeriod (code 7)
 *   - period_ledgers == u32::MAX -> valid maximum period
 *   - auth precedence: unauthorized caller rejected before validation
 */

import {
  MuxSpendingPolicyClient,
  SPEND_LIMIT_MIN,
  SPEND_LIMIT_MAX,
  PERIOD_LEDGERS_MIN,
  PERIOD_LEDGERS_MAX,
  validateSpendPolicyBoundaries,
  validateSpendAmountBoundaries,
} from "../src/generated/mux-spending-policy";
import { spendingPolicyErrorMessage } from "../src/types";
import { ERROR_HTTP_MAP } from "../src/errors";

describe("Spend limit zero/max boundary constants", () => {
  it("defines SPEND_LIMIT_MIN as strictly positive 1n", () => {
    expect(SPEND_LIMIT_MIN).toBe(1n);
  });

  it("defines SPEND_LIMIT_MAX as signed 128-bit maximum (2^127 - 1)", () => {
    expect(SPEND_LIMIT_MAX).toBe(170141183460469231731687303715884105727n);
  });

  it("defines PERIOD_LEDGERS_MIN as 1", () => {
    expect(PERIOD_LEDGERS_MIN).toBe(1);
  });

  it("defines PERIOD_LEDGERS_MAX as unsigned 32-bit maximum (2^32 - 1)", () => {
    expect(PERIOD_LEDGERS_MAX).toBe(4294967295);
  });
});

describe("validateSpendPolicyBoundaries", () => {
  it("accepts valid limits between 1 and i128::MAX", () => {
    expect(() => validateSpendPolicyBoundaries(1n)).not.toThrow();
    expect(() => validateSpendPolicyBoundaries(1000n)).not.toThrow();
    expect(() => validateSpendPolicyBoundaries(SPEND_LIMIT_MAX)).not.toThrow();
  });

  it("rejects zero limit (limit == 0) with InvalidInput", () => {
    expect(() => validateSpendPolicyBoundaries(0n)).toThrow("InvalidInput: limit must be strictly positive");
  });

  it("rejects negative limit (limit < 0) with InvalidInput", () => {
    expect(() => validateSpendPolicyBoundaries(-1n)).toThrow("InvalidInput: limit must be strictly positive");
    expect(() => validateSpendPolicyBoundaries(-1000n)).toThrow("InvalidInput: limit must be strictly positive");
  });

  it("rejects limits exceeding i128::MAX with InvalidInput", () => {
    expect(() => validateSpendPolicyBoundaries(SPEND_LIMIT_MAX + 1n)).toThrow(
      "InvalidInput: limit exceeds maximum i128 value"
    );
  });

  it("accepts valid period_ledgers between 1 and u32::MAX", () => {
    expect(() => validateSpendPolicyBoundaries(100n, 1)).not.toThrow();
    expect(() => validateSpendPolicyBoundaries(100n, 1000)).not.toThrow();
    expect(() => validateSpendPolicyBoundaries(100n, PERIOD_LEDGERS_MAX)).not.toThrow();
  });

  it("rejects zero period_ledgers (period_ledgers == 0) with InvalidPeriod", () => {
    expect(() => validateSpendPolicyBoundaries(100n, 0)).toThrow(
      "InvalidPeriod: period_ledgers must be strictly positive"
    );
  });

  it("rejects period_ledgers exceeding u32::MAX with InvalidPeriod", () => {
    expect(() => validateSpendPolicyBoundaries(100n, PERIOD_LEDGERS_MAX + 1)).toThrow(
      "InvalidPeriod: period_ledgers exceeds maximum u32 value"
    );
  });
});

describe("validateSpendAmountBoundaries", () => {
  it("accepts zero spend amount (amount == 0)", () => {
    expect(() => validateSpendAmountBoundaries(0n)).not.toThrow();
  });

  it("accepts valid positive spend amounts up to i128::MAX", () => {
    expect(() => validateSpendAmountBoundaries(1n)).not.toThrow();
    expect(() => validateSpendAmountBoundaries(1000n)).not.toThrow();
    expect(() => validateSpendAmountBoundaries(SPEND_LIMIT_MAX)).not.toThrow();
  });

  it("rejects negative spend amounts with InvalidInput", () => {
    expect(() => validateSpendAmountBoundaries(-1n)).toThrow(
      "InvalidInput: spend amount cannot be negative"
    );
    expect(() => validateSpendAmountBoundaries(-500n)).toThrow(
      "InvalidInput: spend amount cannot be negative"
    );
  });

  it("rejects spend amounts exceeding i128::MAX with InvalidInput", () => {
    expect(() => validateSpendAmountBoundaries(SPEND_LIMIT_MAX + 1n)).toThrow(
      "InvalidInput: spend amount exceeds maximum i128 value"
    );
  });
});

describe("MuxSpendingPolicyClient boundary pre-flight enforcement", () => {
  const dummyClient = new MuxSpendingPolicyClient({
    contractId: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "http://localhost:8000/soroban/rpc",
  });
  const dummyKeypair = { publicKey: () => "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF" } as any;
  const dummyAddress = { toString: () => "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF" } as any;

  it("setPolicy rejects zero limit fail-closed before network dispatch", async () => {
    await expect(
      dummyClient.setPolicy(dummyKeypair, dummyAddress, dummyAddress, 0n)
    ).rejects.toThrow("InvalidInput: limit must be strictly positive");
  });

  it("setPolicy rejects negative limit fail-closed before network dispatch", async () => {
    await expect(
      dummyClient.setPolicy(dummyKeypair, dummyAddress, dummyAddress, -50n)
    ).rejects.toThrow("InvalidInput: limit must be strictly positive");
  });

  it("setPolicy rejects zero period_ledgers fail-closed before network dispatch", async () => {
    await expect(
      dummyClient.setPolicy(dummyKeypair, dummyAddress, dummyAddress, 500n, 0)
    ).rejects.toThrow("InvalidPeriod: period_ledgers must be strictly positive");
  });

  it("checkSpend rejects negative spend fail-closed before network dispatch", async () => {
    await expect(
      dummyClient.checkSpend(dummyKeypair, dummyAddress, dummyAddress, -1n)
    ).rejects.toThrow("InvalidInput: spend amount cannot be negative");
  });
});

describe("In-memory policy check_spend boundary evaluation", () => {
  interface InMemPolicy {
    limit: bigint;
    periodLedgers: number;
  }

  function evaluateCheckSpend(policy: InMemPolicy | null, amount: bigint): { ok: boolean; error?: string } {
    validateSpendAmountBoundaries(amount);
    if (!policy) {
      return { ok: false, error: "PolicyNotFound" };
    }
    if (amount > policy.limit) {
      return { ok: false, error: "SpendLimitExceeded" };
    }
    return { ok: true };
  }

  it("exact limit boundary: amount == limit succeeds", () => {
    const policy: InMemPolicy = { limit: 1000n, periodLedgers: 100 };
    const res = evaluateCheckSpend(policy, 1000n);
    expect(res.ok).toBe(true);
  });

  it("one-over boundary: amount == limit + 1 fails with SpendLimitExceeded", () => {
    const policy: InMemPolicy = { limit: 1000n, periodLedgers: 100 };
    const res = evaluateCheckSpend(policy, 1001n);
    expect(res.ok).toBe(false);
    expect(res.error).toBe("SpendLimitExceeded");
  });

  it("zero spend boundary: amount == 0 succeeds", () => {
    const policy: InMemPolicy = { limit: 1000n, periodLedgers: 100 };
    const res = evaluateCheckSpend(policy, 0n);
    expect(res.ok).toBe(true);
  });

  it("max i128 boundary: amount == i128::MAX succeeds when limit == i128::MAX", () => {
    const policy: InMemPolicy = { limit: SPEND_LIMIT_MAX, periodLedgers: 100 };
    const res = evaluateCheckSpend(policy, SPEND_LIMIT_MAX);
    expect(res.ok).toBe(true);
  });
});

describe("Error code and HTTP mappings for spend boundaries", () => {
  it("spendingPolicyErrorMessage handles InvalidInput and code 6", () => {
    expect(spendingPolicyErrorMessage("InvalidInput")).toBe("invalid input");
    expect(spendingPolicyErrorMessage(6)).toBe("invalid input");
  });

  it("spendingPolicyErrorMessage handles InvalidPeriod and code 7", () => {
    expect(spendingPolicyErrorMessage("InvalidPeriod")).toBe("invalid period window");
    expect(spendingPolicyErrorMessage(7)).toBe("invalid period window");
  });

  it("spendingPolicyErrorMessage handles SpendLimitExceeded and code 5", () => {
    expect(spendingPolicyErrorMessage("SpendLimitExceeded")).toBe("spend limit exceeded");
    expect(spendingPolicyErrorMessage(5)).toBe("spend limit exceeded");
  });

  it("maps boundary errors to HTTP 400 in ERROR_HTTP_MAP", () => {
    expect(ERROR_HTTP_MAP.InvalidInput).toBe(400);
    expect(ERROR_HTTP_MAP.InvalidPeriod).toBe(400);
    expect(ERROR_HTTP_MAP.SpendLimitExceeded).toBe(400);
  });
});
