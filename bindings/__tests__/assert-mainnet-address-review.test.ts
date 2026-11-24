/**
 * Coverage for assertMainnetAddressReview (bindings/src/addresses-config.ts),
 * specifically the unreviewed-change and marker-mismatch fail-closed paths
 * (#980). These were exported with no direct test coverage despite guarding
 * the mainnet address PR review rule (#871).
 */
import {
  assertMainnetAddressReview,
  hashMainnetAddresses,
  MAINNET_REVIEW_ERROR_CODES,
  MainnetAddressReviewError,
  DEFAULT_ADDRESSES,
  type AddressesConfig,
} from "../src/addresses-config";

function baseline(): AddressesConfig {
  return {
    localnet: { ...DEFAULT_ADDRESSES.localnet },
    testnet: { ...DEFAULT_ADDRESSES.testnet },
    mainnet: { ...DEFAULT_ADDRESSES.mainnet },
  };
}

describe("assertMainnetAddressReview", () => {
  it("does not throw when mainnet addresses are unchanged from the baseline", () => {
    const candidate = baseline();
    expect(() => assertMainnetAddressReview(candidate, baseline())).not.toThrow();
  });

  describe("unreviewed-change path", () => {
    it("throws UNREVIEWED_MAINNET_CHANGE when a mainnet address changes with no mainnetReview marker", () => {
      const candidate = baseline();
      candidate.mainnet.muxAccount = "CCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHA";
      // No mainnetReview marker at all.

      expect(() => assertMainnetAddressReview(candidate, baseline())).toThrow(
        MainnetAddressReviewError,
      );

      try {
        assertMainnetAddressReview(candidate, baseline());
        throw new Error("expected assertMainnetAddressReview to throw");
      } catch (err) {
        expect(err).toBeInstanceOf(MainnetAddressReviewError);
        expect((err as MainnetAddressReviewError).code).toBe(
          MAINNET_REVIEW_ERROR_CODES.UNREVIEWED_MAINNET_CHANGE,
        );
      }
    });

    it("throws INVALID_REVIEW_MARKER when the marker is present but missing required fields", () => {
      const candidate = baseline();
      candidate.mainnet.muxAccount = "CCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHA";
      candidate.mainnetReview = { approvedBy: "", pr: "", addressesHash: "deadbeef" };

      try {
        assertMainnetAddressReview(candidate, baseline());
        throw new Error("expected assertMainnetAddressReview to throw");
      } catch (err) {
        expect(err).toBeInstanceOf(MainnetAddressReviewError);
        expect((err as MainnetAddressReviewError).code).toBe(
          MAINNET_REVIEW_ERROR_CODES.INVALID_REVIEW_MARKER,
        );
      }
    });
  });

  describe("marker-mismatch path", () => {
    it("throws REVIEW_MARKER_MISMATCH when the marker's addressesHash does not cover the current mainnet addresses", () => {
      const candidate = baseline();
      candidate.mainnet.muxAccount = "CCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHA";
      candidate.mainnetReview = {
        approvedBy: "reviewer-1",
        pr: "PR-123",
        addressesHash: "ffffffff", // does not match hashMainnetAddresses(candidate.mainnet)
      };

      try {
        assertMainnetAddressReview(candidate, baseline());
        throw new Error("expected assertMainnetAddressReview to throw");
      } catch (err) {
        expect(err).toBeInstanceOf(MainnetAddressReviewError);
        expect((err as MainnetAddressReviewError).code).toBe(
          MAINNET_REVIEW_ERROR_CODES.REVIEW_MARKER_MISMATCH,
        );
      }
    });

    it("does not throw when the marker's addressesHash correctly covers the changed mainnet addresses", () => {
      const candidate = baseline();
      candidate.mainnet.muxAccount = "CCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHANGEDCHA";
      candidate.mainnetReview = {
        approvedBy: "reviewer-1",
        pr: "PR-123",
        addressesHash: hashMainnetAddresses(candidate.mainnet),
      };

      expect(() => assertMainnetAddressReview(candidate, baseline())).not.toThrow();
    });
  });
});
