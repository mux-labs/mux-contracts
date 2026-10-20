/**
 * Guard against silent export-name collisions in bindings/src/index.ts.
 *
 * index.ts re-exports every generated contract client with `export *`:
 *
 *   export * from "./generated/mux-account";
 *   export * from "./generated/mux-account-factory";
 *   export * from "./generated/mux-batcher";
 *   ...
 *
 * `export *` is a silent operation: if two of these generated modules ever
 * export a symbol with the same name (e.g. two contracts both adding a
 * `CONTRACT_VERSION` constant, or two error classes both named
 * `ValidationError`), TypeScript treats the colliding name as ambiguous and
 * it is simply dropped from the module's export list — no build error, no
 * runtime error, just one contract's symbol silently shadowing (or being
 * shadowed by) the other's.
 *
 * This test collects every top-level exported name from each generated
 * client module and asserts there are no duplicates across them, so that if
 * a future codegen run or manual edit introduces a collision, it fails loudly
 * here instead of silently vanishing from the public API (#981).
 */
import * as muxAccount from "../src/generated/mux-account";
import * as muxAccountFactory from "../src/generated/mux-account-factory";
import * as muxBatcher from "../src/generated/mux-batcher";
import * as muxDelegation from "../src/generated/mux-delegation";
import * as muxPermissions from "../src/generated/mux-permissions";
import * as muxPolicy from "../src/generated/mux-policy";
import * as muxRegistry from "../src/generated/mux-registry";
import * as muxWalletRegistry from "../src/generated/mux-wallet-registry";
import * as muxSpendingPolicy from "../src/generated/mux-spending-policy";
import * as muxRecovery from "../src/generated/mux-recovery";

const GENERATED_MODULES: Record<string, Record<string, unknown>> = {
  "./generated/mux-account": muxAccount,
  "./generated/mux-account-factory": muxAccountFactory,
  "./generated/mux-batcher": muxBatcher,
  "./generated/mux-delegation": muxDelegation,
  "./generated/mux-permissions": muxPermissions,
  "./generated/mux-policy": muxPolicy,
  "./generated/mux-registry": muxRegistry,
  "./generated/mux-wallet-registry": muxWalletRegistry,
  "./generated/mux-spending-policy": muxSpendingPolicy,
  "./generated/mux-recovery": muxRecovery,
};

describe("index.ts `export *` generated client collision guard (#981)", () => {
  it("no exported name is shared by two or more generated client modules", () => {
    const ownerOf = new Map<string, string[]>();

    for (const [modulePath, moduleExports] of Object.entries(GENERATED_MODULES)) {
      for (const exportedName of Object.keys(moduleExports)) {
        const owners = ownerOf.get(exportedName) ?? [];
        owners.push(modulePath);
        ownerOf.set(exportedName, owners);
      }
    }

    const collisions = [...ownerOf.entries()].filter(([, owners]) => owners.length > 1);

    if (collisions.length > 0) {
      const detail = collisions
        .map(([name, owners]) => `  "${name}" exported by: ${owners.join(", ")}`)
        .join("\n");
      throw new Error(
        `Found export-name collision(s) across generated client modules re-exported ` +
          `via "export *" in bindings/src/index.ts. A colliding name is dropped ` +
          `silently by TypeScript instead of raising a build error, so it must be ` +
          `fixed with named/aliased exports:\n${detail}`,
      );
    }

    expect(collisions).toEqual([]);
  });

  it("every generated module exports at least one symbol (sanity check that imports resolved)", () => {
    for (const [modulePath, moduleExports] of Object.entries(GENERATED_MODULES)) {
      expect(Object.keys(moduleExports).length).toBeGreaterThan(0);
      void modulePath;
    }
  });
});
