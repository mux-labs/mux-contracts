/**
 * Access-control checklist automation for mutating entrypoints.
 *
 * Scans the generated bindings for mutating entrypoints (functions that
 * submit a transaction / change on-chain state) and asserts that each one is
 * covered by an explicit authorization check. The check is deny-by-default:
 * any mutating entrypoint that is not present in the documented authz mapping
 * fails the test rather than passing silently.
 *
 * See docs/access-control-checklist.md for the authoritative mapping and the
 * invariants this test enforces.
 */

import * as fs from 'fs';
import * as path from 'path';

const GENERATED_DIR = path.resolve(__dirname, '..', 'src', 'generated');

/**
 * Documented authorization mapping for every mutating entrypoint.
 *
 * Key: `<module>#<entrypoint>` where `<module>` is the generated bindings file
 * basename without extension (e.g. `mux-registry`).
 * Value: the required authorization role(s) enforced before the mutation is
 * submitted. An empty/unknown value is treated as unguarded and fails closed.
 *
 * Adding a new mutating entrypoint to the bindings REQUIRES adding an entry
 * here (and to docs/access-control-checklist.md) or CI will fail.
 */
const AUTHZ_MAPPING: Record<string, string[]> = {
  'mux-registry#register': ['owner', 'delegate'],
  'mux-registry#deregister': ['owner', 'delegate'],
  'mux-registry#update': ['owner', 'delegate'],
  'mux-wallet-registry#register': ['owner', 'delegate'],
  'mux-wallet-registry#deregister': ['owner', 'delegate'],
  'mux-wallet-registry#update': ['owner', 'delegate'],
};

/**
 * Names that indicate a function mutates on-chain state. Read-only helpers
 * (getters, lookups, simulations) are intentionally excluded.
 */
const MUTATING_PREFIXES = ['register', 'deregister', 'update', 'set', 'add', 'remove', 'revoke', 'grant', 'transfer', 'withdraw', 'deposit', 'mint', 'burn', 'pause', 'unpause', 'upgrade', 'initialize', 'init'];

/**
 * Names that are explicitly read-only even if they share a mutating prefix.
 */
const READ_ONLY_NAMES = new Set(['get', 'lookup', 'simulate', 'query', 'read', 'fetch', 'list', 'has', 'is']);

interface Entrypoint {
  module: string;
  name: string;
  key: string;
}

function listGeneratedModules(): string[] {
  if (!fs.existsSync(GENERATED_DIR)) {
    return [];
  }
  return fs
    .readdirSync(GENERATED_DIR)
    .filter((f) => f.endsWith('.ts') && !f.endsWith('.d.ts'))
    .map((f) => f.replace(/\.ts$/, ''));
}

/**
 * Extract exported function names from a generated bindings module. We look for
 * `export function <name>` / `export async function <name>` declarations, which
 * is the shape emitted by the bindings generator for entrypoints.
 */
function extractExportedFunctions(source: string): string[] {
  const names = new Set<string>();
  const re = /export\s+(?:async\s+)?function\s+([A-Za-z0-9_]+)/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(source)) !== null) {
    names.add(match[1]);
  }
  return Array.from(names);
}

function isMutating(name: string): boolean {
  const lower = name.toLowerCase();
  if (READ_ONLY_NAMES.has(lower)) {
    return false;
  }
  return MUTATING_PREFIXES.some((prefix) => lower.startsWith(prefix));
}

function collectMutatingEntrypoints(): Entrypoint[] {
  const entrypoints: Entrypoint[] = [];
  for (const module of listGeneratedModules()) {
    const file = path.join(GENERATED_DIR, `${module}.ts`);
    const source = fs.readFileSync(file, 'utf8');
    for (const name of extractExportedFunctions(source)) {
      if (isMutating(name)) {
        entrypoints.push({ module, name, key: `${module}#${name}` });
      }
    }
  }
  return entrypoints;
}

describe('access-control checklist for mutating entrypoints', () => {
  const entrypoints = collectMutatingEntrypoints();

  it('discovers at least one mutating entrypoint to guard', () => {
    // Guards against a silent pass if the generator output shape changes and
    // the scanner stops finding entrypoints entirely.
    expect(entrypoints.length).toBeGreaterThan(0);
  });

  it('documents an authorization mapping for every mutating entrypoint', () => {
    const undocumented = entrypoints
      .filter((ep) => !(ep.key in AUTHZ_MAPPING))
      .map((ep) => ep.key);

    expect(undocumented).toEqual([]);
  });

  it('requires a non-empty authorization role for every mutating entrypoint', () => {
    const unguarded = entrypoints
      .filter((ep) => {
        const roles = AUTHZ_MAPPING[ep.key];
        return !roles || roles.length === 0;
      })
      .map((ep) => ep.key);

    expect(unguarded).toEqual([]);
  });

  it('does not carry stale authz mappings for removed entrypoints', () => {
    const discovered = new Set(entrypoints.map((ep) => ep.key));
    const stale = Object.keys(AUTHZ_MAPPING).filter((key) => !discovered.has(key));

    expect(stale).toEqual([]);
  });
});
