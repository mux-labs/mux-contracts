import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "node",
    coverage: {
      provider: "v8",
      reporter: ["text", "json-summary", "html"],
      // Gate only the hand-written bindings modules; generated clients are
      // codegen output and are excluded from the report.
      include: [
        "src/errors.ts",
        "src/batcher.ts",
        "src/horizon.ts",
        "src/simulate-batch.ts",
        "src/network.ts",
        "src/addresses.ts",
      ],
      exclude: ["src/generated/**"],
      // Baseline measured on the current tree (issue #925), floored to whole
      // percent so small v8/version differences cannot flake the gate.
      thresholds: {
        "src/errors.ts": { statements: 87, branches: 100, functions: 0, lines: 87 },
        "src/batcher.ts": { statements: 100, branches: 100, functions: 100, lines: 100 },
        "src/horizon.ts": { statements: 100, branches: 95, functions: 100, lines: 100 },
        "src/simulate-batch.ts": { statements: 0, branches: 0, functions: 0, lines: 0 },
        "src/network.ts": { statements: 71, branches: 42, functions: 59, lines: 71 },
        "src/addresses.ts": { statements: 96, branches: 96, functions: 100, lines: 96 },
      },
    },
  },
});
