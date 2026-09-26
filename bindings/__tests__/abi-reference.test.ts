/**
 * Verifies that docs/abi_reference.md documents the mux-account-factory
 * public interface (#221) and the max ops constant (#870).
 */

import * as fs from "fs";
import * as path from "path";

const ABI_DOC = path.resolve(__dirname, "../../docs/abi_reference.md");
const content = fs.readFileSync(ABI_DOC, "utf-8");

describe("docs/abi_reference.md — mux-account-factory", () => {
  it("has a mux-account-factory section", () => {
    expect(content).toMatch(/##\s+mux-account-factory/);
  });

  it("documents deploy_account", () => {
    expect(content).toContain("deploy_account");
  });

  it("documents deploy_account_with_metadata", () => {
    expect(content).toContain("deploy_account_with_metadata");
  });

  it("documents get_accounts", () => {
    expect(content).toContain("get_accounts");
  });

  it("documents get_account_metadata", () => {
    expect(content).toContain("get_account_metadata");
  });

  it("documents account_count", () => {
    expect(content).toContain("account_count");
  });

  it("documents AccountMetadata type", () => {
    expect(content).toContain("AccountMetadata");
  });

  it("documents all four error variants", () => {
    expect(content).toContain("Unauthorized");
    expect(content).toContain("InvalidAccount");
    expect(content).toContain("TooManyAccounts");
    expect(content).toContain("MetadataNotFound");
  });

  it("documents the deployed event", () => {
    expect(content).toContain("deployed");
  });

  it("documents MAX_ACCOUNTS_PER_OWNER cap", () => {
    expect(content).toContain("MAX_ACCOUNTS_PER_OWNER");
  });
});

describe("docs/abi_reference.md — max ops constant (#870)", () => {
  it("documents the MAX_OPS constant name", () => {
    expect(content).toContain("MAX_OPS");
  });

  it("documents the MAX_OPS value", () => {
    expect(content).toMatch(/MAX_OPS[^\n]*\b100\b/);
  });

  it("documents MAX_OPS semantics (per-transaction op cap)", () => {
    expect(content).toMatch(/MAX_OPS[\s\S]{0,400}per[- ]transaction/i);
  });
});

describe("docs/abi_reference.md — mux-batcher", () => {
  it("has a mux-batcher section", () => {
    expect(content).toMatch(/##\s+mux-batcher/);
  });

  it("documents batcher methods", () => {
    expect(content).toContain("execute_batch");
    expect(content).toContain("submit_batch");
    expect(content).toContain("simulate_batch");
    expect(content).toContain("estimate_fees");
    expect(content).toContain("max_batch_size");
  });

  it("documents batcher errors", () => {
    expect(content).toContain("EmptyBatch");
    expect(content).toContain("BatchTooLarge");
    expect(content).toContain("RequiredOperationFailed");
  });

  it("documents batcher events", () => {
    expect(content).toContain("bat_start");
    expect(content).toContain("executed");
    expect(content).toContain("bat_ok");
    expect(content).toContain("bat_abort");
  });
});

describe("docs/abi_reference.md — mux-account", () => {
  it("has a mux-account section", () => {
    expect(content).toMatch(/##\s+mux-account/);
  });

  it("documents account entrypoints", () => {
    expect(content).toContain("initialize");
    expect(content).toContain("execute");
    expect(content).toContain("execute_with_session");
    expect(content).toContain("execute_with_session_sponsored");
    expect(content).toContain("set_delegate");
    expect(content).toContain("remove_delegate");
    expect(content).toContain("set_spend_limit");
    expect(content).toContain("debit_spend");
    expect(content).toContain("register_session_key");
    expect(content).toContain("revoke_session_key");
  });

  it("documents account errors", () => {
    expect(content).toContain("NotInitialized");
    expect(content).toContain("AlreadyInitialized");
    expect(content).toContain("SpendLimitExceeded");
    expect(content).toContain("InvalidNonce");
    expect(content).toContain("ScopeNotGranted");
  });
});

describe("docs/abi_reference.md — mux-permissions", () => {
  it("has a mux-permissions section", () => {
    expect(content).toMatch(/##\s+mux-permissions/);
  });

  it("documents permissions methods", () => {
    expect(content).toContain("create_role");
    expect(content).toContain("grant_role");
    expect(content).toContain("revoke_role");
    expect(content).toContain("has_permission");
    expect(content).toContain("set_admin_threshold");
    expect(content).toContain("approve_admin");
  });
});

describe("docs/abi_reference.md — mux-spending-policy", () => {
  it("has a mux-spending-policy section", () => {
    expect(content).toMatch(/##\s+mux-spending-policy/);
  });

  it("documents spending policy methods", () => {
    expect(content).toContain("set_policy");
    expect(content).toContain("get_policy");
    expect(content).toContain("check_spend");
  });
});

describe("docs/abi_reference.md — mux-wallet-registry", () => {
  it("has a mux-wallet-registry section", () => {
    expect(content).toMatch(/##\s+mux-wallet-registry/);
  });

  it("documents wallet registry methods", () => {
    expect(content).toContain("register_wallet");
    expect(content).toContain("get_wallet");
    expect(content).toContain("list_wallets");
  });
});

describe("docs/abi_reference.md — mux-registry", () => {
  it("has a mux-registry section", () => {
    expect(content).toMatch(/##\s+mux-registry/);
  });

  it("documents registry methods", () => {
    expect(content).toContain("register");
    expect(content).toContain("get_version");
    expect(content).toContain("list_contracts");
  });
});

describe("docs/abi_reference.md — mux-recovery", () => {
  it("has a mux-recovery section", () => {
    expect(content).toMatch(/##\s+mux-recovery/);
  });

  it("documents recovery methods", () => {
    expect(content).toContain("initiate_recovery");
    expect(content).toContain("approve_recovery");
    expect(content).toContain("execute_recovery");
    expect(content).toContain("cancel_recovery");
  });
});

describe("docs/abi_reference.md — mux-policy", () => {
  it("has a mux-policy section", () => {
    expect(content).toMatch(/##\s+mux-policy/);
  });

  it("documents policy methods", () => {
    expect(content).toContain("set_daily_limit");
    expect(content).toContain("get_daily_limit");
    expect(content).toContain("record_spend");
    expect(content).toContain("reset_daily_counter");
  });
});

describe("docs/abi_reference.md — mux-delegation", () => {
  it("has a mux-delegation section", () => {
    expect(content).toMatch(/##\s+mux-delegation/);
  });

  it("documents delegation methods", () => {
    expect(content).toContain("grant_delegate");
    expect(content).toContain("revoke_delegate");
    expect(content).toContain("get_delegate_permissions");
    expect(content).toContain("check_delegate");
  });
});
