import {
  assertMainnetAddressReview,
  DEFAULT_ADDRESSES,
  hashMainnetAddresses,
  MainnetAddressReviewError,
  MAINNET_REVIEW_ERROR_CODES,
  type AddressBook,
  type AddressesConfig,
  type MainnetReviewMarker,
} from "../src/addresses-config";

const CORRELATION_ID = "test-mainnet-review";

function changedMainnetConfig(
  mainnetReview?: MainnetReviewMarker,
): AddressesConfig {
  return {
    localnet: { ...DEFAULT_ADDRESSES.localnet },
    testnet: { ...DEFAULT_ADDRESSES.testnet },
    mainnet: {
      ...DEFAULT_ADDRESSES.mainnet,
      muxAccount: "CNEW_MAINNET_ACCOUNT",
    },
    ...(mainnetReview ? { mainnetReview } : {}),
  };
}

function expectReviewError(
  run: () => void,
  code: (typeof MAINNET_REVIEW_ERROR_CODES)[keyof typeof MAINNET_REVIEW_ERROR_CODES],
): void {
  let thrown: unknown;
  try {
    run();
  } catch (error) {
    thrown = error;
  }

  expect(thrown).toBeInstanceOf(MainnetAddressReviewError);
  if (thrown instanceof MainnetAddressReviewError) {
    expect(thrown.code).toBe(code);
    expect(thrown.correlationId).toBe(CORRELATION_ID);
  }
}

describe("Mainnet address review enforcement", () => {
  it("rejects an address change without a review marker", () => {
    expectReviewError(
      () =>
        assertMainnetAddressReview(
          changedMainnetConfig(),
          DEFAULT_ADDRESSES,
          CORRELATION_ID,
        ),
      MAINNET_REVIEW_ERROR_CODES.UNREVIEWED_MAINNET_CHANGE,
    );
  });

  it.each(["approvedBy", "pr", "addressesHash"] as const)(
    "rejects a marker with an empty %s field",
    (field) => {
      const addresses = changedMainnetConfig();
      const marker: MainnetReviewMarker = {
        approvedBy: "reviewer",
        pr: "https://github.com/mux-labs/mux-contracts/pull/1",
        addressesHash: hashMainnetAddresses(addresses.mainnet),
      };
      marker[field] = "";

      expectReviewError(
        () =>
          assertMainnetAddressReview(
            { ...addresses, mainnetReview: marker },
            DEFAULT_ADDRESSES,
            CORRELATION_ID,
          ),
        MAINNET_REVIEW_ERROR_CODES.INVALID_REVIEW_MARKER,
      );
    },
  );

  it("rejects a marker whose hash does not match the current addresses", () => {
    const addresses = changedMainnetConfig({
      approvedBy: "reviewer",
      pr: "https://github.com/mux-labs/mux-contracts/pull/1",
      addressesHash: "00000000",
    });

    expectReviewError(
      () =>
        assertMainnetAddressReview(
          addresses,
          DEFAULT_ADDRESSES,
          CORRELATION_ID,
        ),
      MAINNET_REVIEW_ERROR_CODES.REVIEW_MARKER_MISMATCH,
    );
  });

  it("accepts a marker that matches the reviewed mainnet addresses", () => {
    const addresses = changedMainnetConfig();
    const marker: MainnetReviewMarker = {
      approvedBy: "reviewer",
      pr: "https://github.com/mux-labs/mux-contracts/pull/1",
      addressesHash: hashMainnetAddresses(addresses.mainnet),
    };

    expect(() =>
      assertMainnetAddressReview(
        { ...addresses, mainnetReview: marker },
        DEFAULT_ADDRESSES,
        CORRELATION_ID,
      ),
    ).not.toThrow();
  });

  it("hashes equivalent address books independently of key order", () => {
    const first: AddressBook = {
      muxAccount: "A",
      muxAccountFactory: "B",
      muxBatcher: "C",
      muxDelegation: "D",
      muxPermissions: "E",
      muxPolicy: "F",
      muxRecovery: "G",
      muxRegistry: "H",
      muxSpendingPolicy: "I",
      muxWalletRegistry: "J",
    };
    const reordered: AddressBook = {
      muxWalletRegistry: "J",
      muxSpendingPolicy: "I",
      muxRegistry: "H",
      muxRecovery: "G",
      muxPolicy: "F",
      muxPermissions: "E",
      muxDelegation: "D",
      muxBatcher: "C",
      muxAccountFactory: "B",
      muxAccount: "A",
    };

    expect(hashMainnetAddresses(first)).toBe(hashMainnetAddresses(reordered));
  });
});
