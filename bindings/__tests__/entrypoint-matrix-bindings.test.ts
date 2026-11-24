import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { MuxAccountClient } from "../src/generated/mux-account";
import { MuxAccountFactoryClient } from "../src/generated/mux-account-factory";
import { MuxBatcherClient } from "../src/generated/mux-batcher";
import { MuxDelegationClient } from "../src/generated/mux-delegation";
import { MuxPermissionsClient } from "../src/generated/mux-permissions";
import { MuxPolicyClient } from "../src/generated/mux-policy";
import { MuxRecoveryClient } from "../src/generated/mux-recovery";
import { MuxRegistryClient } from "../src/generated/mux-registry";
import { MuxSpendingPolicyClient } from "../src/generated/mux-spending-policy";
import { MuxWalletRegistryClient } from "../src/generated/mux-wallet-registry";

/**
 * Guards the generated TypeScript clients against `docs/entrypoint-matrix.md`.
 *
 * The Rust-side `tests/entrypoint_matrix_coverage.rs` treats the matrix as the
 * source of truth for which entrypoints exist. This suite mirrors that check on
 * the bindings side: every documented entrypoint must be exposed as a method on
 * the corresponding generated client class.
 */

const MATRIX_PATH = resolve(__dirname, "../../docs/entrypoint-matrix.md");

/** Maps a contract section heading in the matrix to its generated client. */
const CONTRACT_CLIENTS: Record<string, new (...args: never[]) => object> = {
  "mux-account": MuxAccountClient,
  "mux-account-factory": MuxAccountFactoryClient,
  "mux-batcher": MuxBatcherClient,
  "mux-delegation": MuxDelegationClient,
  "mux-permissions": MuxPermissionsClient,
  "mux-policy": MuxPolicyClient,
  "mux-recovery": MuxRecoveryClient,
  "mux-registry": MuxRegistryClient,
  "mux-spending-policy": MuxSpendingPolicyClient,
  "mux-wallet-registry": MuxWalletRegistryClient,
};

/**
 * Entrypoints that are deliberately not exposed as client methods.
 *
 * The generated clients currently omit read-only query entrypoints; they are
 * documented in the matrix but not surfaced on the client class. Keep this list
 * explicit so a genuinely missing method is never silently skipped.
 */
const EXCLUDED_ENTRYPOINTS: ReadonlySet<string> = new Set<string>([
  // e.g. "mux-account::get_balance"
]);

interface MatrixSection {
  contract: string;
  entrypoints: string[];
}

function stripBackticks(value: string): string {
  return value.replace(/`/g, "").trim();
}

function parseMatrix(markdown: string): MatrixSection[] {
  const sections: MatrixSection[] = [];

  for (const chunk of markdown.split(/^## /m).slice(1)) {
    const lines = chunk.split("\n");
    const heading = stripBackticks(lines[0] ?? "");
    const contract = heading.split(/\s+/)[0] ?? "";
    if (!contract) {
      continue;
    }

    const entrypoints: string[] = [];
    for (const line of lines.slice(1)) {
      const trimmed = line.trim();
      if (!trimmed.startsWith("|")) {
        continue;
      }
      const cells = trimmed.split("|").map((cell) => cell.trim());
      // Drop the leading/trailing empty cells produced by the surrounding pipes.
      const row = cells.slice(1, -1);
      if (row.length === 0) {
        continue;
      }
      const first = stripBackticks(row[0] ?? "");
      if (!first || /^[-:]+$/.test(first) || first.toLowerCase() === "entrypoint") {
        continue;
      }
      entrypoints.push(first);
    }

    sections.push({ contract, entrypoints });
  }

  return sections;
}

function snakeToCamel(name: string): string {
  return name.replace(/_([a-z0-9])/g, (_match, char: string) => char.toUpperCase());
}

describe("entrypoint matrix bindings coverage", () => {
  const markdown = readFileSync(MATRIX_PATH, "utf8");
  const sections = parseMatrix(markdown);

  it("parses at least one contract section from the matrix", () => {
    expect(sections.length).toBeGreaterThan(0);
  });

  for (const section of sections) {
    const Client = CONTRACT_CLIENTS[section.contract];

    it(`maps contract ${section.contract} to a generated client`, () => {
      expect(Client, `no generated client mapped for contract ${section.contract}`).toBeDefined();
    });

    if (!Client) {
      continue;
    }

    for (const entrypoint of section.entrypoints) {
      const key = `${section.contract}::${entrypoint}`;
      if (EXCLUDED_ENTRYPOINTS.has(key)) {
        continue;
      }

      it(`${section.contract} exposes ${entrypoint}`, () => {
        const method = snakeToCamel(entrypoint);
        const prototype = Client.prototype as Record<string, unknown>;
        expect(
          typeof prototype[method],
          `contract ${section.contract} is missing entrypoint ${entrypoint} (expected method ${method})`,
        ).toBe("function");
      });
    }
  }
});
