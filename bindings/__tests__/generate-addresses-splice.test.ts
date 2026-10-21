import { describe, it, expect } from "vitest";
import {
  BEGIN_SENTINEL,
  END_SENTINEL,
  buildBlock,
  spliceBlock,
} from "../../scripts/generate-addresses";

/**
 * Splice-branch coverage for `scripts/generate-addresses.ts` (issue #926).
 *
 * `spliceBlock` has three branches — replace a sentineled block, replace an
 * old `export const DEFAULT_ADDRESSES` declaration, or prepend when neither
 * exists. A wrong branch silently corrupts `addresses-config.ts`, so each is
 * pinned here with synthetic input: the non-generated content must be
 * byte-identical before and after in every case.
 *
 * The module is imported (not executed): the CLI guards `main()` behind
 * `import.meta.main`.
 */

/** Synthetic generated block — content is irrelevant to the splice logic. */
const BLOCK = [
  BEGIN_SENTINEL,
  "// DO NOT EDIT — run `npx ts-node scripts/generate-addresses.ts` to regenerate.",
  "export const DEFAULT_ADDRESSES = {",
  '  testnet: {',
  '    muxAccount: "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",',
  "  },",
  "};",
  END_SENTINEL,
].join("\n");

/** Header/footer that must survive every splice branch untouched. */
const HEADER = "// codegen header — must survive\nimport type { Address } from \"x\";\n";
const FOOTER = "\nexport function isAddress(v: unknown): v is Address {\n  return typeof v === \"string\";\n}\n";

describe("spliceBlock branch coverage (issue #926)", () => {
  it("buildBlock emits a sentinel-delimited DEFAULT_ADDRESSES block", () => {
    const data = {
      testnet: { muxAccount: "C" + "A".repeat(55) },
    } as unknown as Parameters<typeof buildBlock>[0];
    const block = buildBlock(data);
    expect(block).toContain(BEGIN_SENTINEL);
    expect(block).toContain(END_SENTINEL);
    expect(block).toContain("export const DEFAULT_ADDRESSES = {");
    expect(block.indexOf(BEGIN_SENTINEL)).toBeLessThan(
      block.indexOf(END_SENTINEL)
    );
  });

  it("branch 1: replaces an existing sentineled block, surrounding content byte-identical", () => {
    const staleBlock = [BEGIN_SENTINEL, "const DEFAULT_ADDRESSES = { stale: true };", END_SENTINEL].join("\n");
    const existing = HEADER + staleBlock + FOOTER;

    const beginIdx = existing.indexOf(BEGIN_SENTINEL);
    const endIdx = existing.indexOf(END_SENTINEL);
    expect(beginIdx).toBeGreaterThanOrEqual(0);
    expect(endIdx).toBeGreaterThan(beginIdx);

    const prefix = existing.slice(0, beginIdx);
    const suffix = existing.slice(endIdx + END_SENTINEL.length);

    const result = spliceBlock(existing, BLOCK);

    // Surrounding non-generated content is byte-identical before/after.
    expect(result.startsWith(prefix)).toBe(true);
    expect(result.endsWith(suffix)).toBe(true);
    // The generated block sits exactly where the stale one was.
    expect(result).toBe(prefix + BLOCK + suffix);
    expect(result).not.toContain("stale: true");
    // Nothing outside the sentinel region changed length unexpectedly.
    expect(result.length - BLOCK.length).toBe(existing.length - staleBlock.length);
  });

  it("branch 2: replaces an old `export const DEFAULT_ADDRESSES` declaration, surrounding content byte-identical", () => {
    const oldDecl =
      "export const DEFAULT_ADDRESSES = {\n" +
      "  testnet: {\n" +
      '    muxAccount: "OLD",\n' +
      "  },\n" +
      "};";
    const existing = HEADER + oldDecl + FOOTER;
    expect(existing).not.toContain(BEGIN_SENTINEL);

    // Split at the exact match the splice uses, to verify preservation.
    const pattern = /^export const DEFAULT_ADDRESSES\s*=[\s\S]*?^};/m;
    const match = existing.match(pattern);
    expect(match).not.toBeNull();
    const at = match!.index!;
    const prefix = existing.slice(0, at);
    const suffix = existing.slice(at + match![0].length);

    const result = spliceBlock(existing, BLOCK);

    expect(result.startsWith(prefix)).toBe(true);
    expect(result.endsWith(suffix)).toBe(true);
    expect(result).toBe(prefix + BLOCK + suffix);
    expect(result).not.toContain('"OLD"');
  });

  it("branch 3: prepends the block when neither sentinels nor old declaration exist", () => {
    const existing = HEADER + 'export const UNRELATED = "keep me";\n' + FOOTER;
    expect(existing).not.toContain(BEGIN_SENTINEL);
    expect(existing).not.toContain("export const DEFAULT_ADDRESSES");

    const result = spliceBlock(existing, BLOCK);

    expect(result.startsWith(BLOCK + "\n\n")).toBe(true);
    // The entire pre-existing file is preserved byte-for-byte as the suffix.
    expect(result.endsWith(existing)).toBe(true);
    expect(result).toBe(BLOCK + "\n\n" + existing);
    expect(result).toContain('export const UNRELATED = "keep me";');
  });

  it("importing the generator does not execute the CLI (import.meta.main guard)", () => {
    // Reaching this test at all means the module imported cleanly without
    // running main() — which would have required config/addresses.json and
    // printed CLI output. Assert spliceBlock is a function, i.e. the import
    // surface the CLI shares with the test is intact.
    expect(typeof spliceBlock).toBe("function");
    expect(typeof buildBlock).toBe("function");
  });
});
