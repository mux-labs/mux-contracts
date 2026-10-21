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
import * as MuxAccountModule from '../src/generated/mux-account';
import * as MuxAccountFactoryModule from '../src/generated/mux-account-factory';
import * as MuxBatcherModule from '../src/generated/mux-batcher';
import * as MuxDelegationModule from '../src/generated/mux-delegation';
import * as MuxPermissionsModule from '../src/generated/mux-permissions';
import * as MuxRecoveryModule from '../src/generated/mux-recovery';
import * as MuxRegistryModule from '../src/generated/mux-registry';
import * as MuxSpendingPolicyModule from '../src/generated/mux-spending-policy';
import * as MuxWalletRegistryModule from '../src/generated/mux-wallet-registry';

const GENERATED_DIR = path.resolve(__dirname, '..', 'src', 'generated');
const DOCS_PATH = path.resolve(__dirname, '..', '..', 'docs', 'access-control-checklist.md');

/**
 * Read a file by its absolute path, failing with a clear, actionable error
 * message rather than letting a bare ENOENT from fs.readFileSync bubble up
 * when the file has moved or been renamed (#1006).
 */
function readFileOrThrow(fullPath: string): string {
  if (!fs.existsSync(fullPath)) {
    throw new Error(
      `access-control-checklist.test.ts: expected file not found at ${fullPath}. ` +
        `The file may have moved or been renamed — update the path in this test.`
    );
  }
  return fs.readFileSync(fullPath, 'utf8');
}

/**
 * Documented authorization mapping for every mutating entrypoint.
 *
 * Key: `<module>#<entrypoint>` where `<module>` is the generated bindings file
 * basename without extension (e.g. `mux-registry`) and `<entrypoint>` is the
 * snake_case contract method name (derived from the client's camelCase method
 * name). Value: the required authorization role(s) enforced before the mutation is
 * submitted. An empty/unknown value is treated as unguarded and fails closed.
 *
 * Adding a new mutating entrypoint to the bindings REQUIRES adding an entry
 * here (and to docs/access-control-checklist.md) or CI will fail.
 */
const AUTHZ_MAPPING: Record<string, string[]> = {
  // mux-account — mirrors MUX_ACCOUNT_MUTATING_AUTHZ in generated/mux-account.ts.
  'mux-account#initialize': ['public'],
  'mux-account#set_delegate': ['owner'],
  'mux-account#remove_delegate': ['owner'],
  'mux-account#set_spend_limit': ['owner'],
  // mux-batcher — `set_registry_metadata` calls require_admin in lib.rs.
  'mux-batcher#set_registry_metadata': ['owner'],
  // mux-delegation — grant/revoke call owner.require_auth() in lib.rs.
  'mux-delegation#grant_delegate': ['owner'],
  'mux-delegation#revoke_delegate': ['owner'],
  // mux-permissions — grant/revoke/threshold call require_admin in lib.rs;
  // initialize is a one-time init signed by the founding admin.
  'mux-permissions#initialize': ['public'],
  'mux-permissions#grant_role': ['owner'],
  'mux-permissions#revoke_role': ['owner'],
  'mux-permissions#set_admin_threshold': ['owner'],
  // mux-registry — register* call require_admin in lib.rs; initialize is a
  // one-time init signed by the founding admin.
  'mux-registry#initialize': ['public'],
  'mux-registry#register': ['owner'],
  'mux-registry#register_with_metadata': ['owner'],
  // mux-spending-policy — mutations are admin/owner-only (or an authorized
  // delegate) per the generated client's documented invariants.
  'mux-spending-policy#initialize': ['public'],
  'mux-spending-policy#set_policy': ['owner'],
  'mux-spending-policy#remove_allowlisted_asset': ['owner', 'delegate'],
  'mux-spending-policy#set_relayer_sponsorship_limit': ['owner', 'delegate'],
  // mux-wallet-registry — register* call require_owner in lib.rs; initialize
  // is a one-time init signed by the founding owner.
  'mux-wallet-registry#initialize': ['public'],
  'mux-wallet-registry#register_wallet': ['owner'],
  'mux-wallet-registry#register_wallet_with_metadata': ['owner'],
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

/**
 * Methods that are never contract entrypoints: private client plumbing and
 * explicitly-excluded read-only views. Excluded here so they are not flagged
 * as unmapped mutating entrypoints.
 */
const NON_ENTRYPOINT_METHODS = new Set([
  'constructor',
  'buildTx',
  'submit',
  'submitAndRead',
  'simulate',
  'simulateRead',
  'transition',
  'readRecovery',
  // Read-only view (#938): excluded explicitly, not by omission.
  'owner',
]);

/** Map a client method name (camelCase) to its contract entrypoint (snake_case). */
function methodToEntrypoint(method: string): string {
  return method.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

const CLIENT_MODULES: Record<string, Record<string, unknown>> = {
  'mux-account': MuxAccountModule,
  'mux-account-factory': MuxAccountFactoryModule,
  'mux-batcher': MuxBatcherModule,
  'mux-delegation': MuxDelegationModule,
  'mux-permissions': MuxPermissionsModule,
  'mux-recovery': MuxRecoveryModule,
  'mux-registry': MuxRegistryModule,
  'mux-spending-policy': MuxSpendingPolicyModule,
  'mux-wallet-registry': MuxWalletRegistryModule,
};

/**
 * Enumerate mutating entrypoints from the prototype of every exported
 * `*Client` class in the generated bindings. Most entrypoints are exposed as
 * camelCase client methods rather than exported functions, so the exported
 * function scan alone cannot see them (#938).
 */
function collectClientMutatingEntrypoints(): Entrypoint[] {
  const entrypoints: Entrypoint[] = [];
  for (const [module, mod] of Object.entries(CLIENT_MODULES)) {
    for (const [exportName, exported] of Object.entries(mod)) {
      if (typeof exported !== 'function' || !exportName.endsWith('Client')) continue;
      const prototype = (exported as { prototype: object }).prototype;
      for (const method of Object.getOwnPropertyNames(prototype)) {
        if (NON_ENTRYPOINT_METHODS.has(method)) continue;
        const entrypoint = methodToEntrypoint(method);
        if (isMutating(entrypoint)) {
          entrypoints.push({ module, name: entrypoint, key: `${module}#${entrypoint}` });
        }
      }
    }
  }
  return entrypoints;
}

function collectMutatingEntrypoints(): Entrypoint[] {
  const entrypoints: Entrypoint[] = [];
  for (const module of listGeneratedModules()) {
    const file = path.join(GENERATED_DIR, `${module}.ts`);
    const source = readFileOrThrow(file);
    for (const name of extractExportedFunctions(source)) {
      if (isMutating(name)) {
        entrypoints.push({ module, name, key: `${module}#${name}` });
      }
    }
  }
  // Deduplicate: a module may expose the same entrypoint as both an exported
  // function and a client method.
  const seen = new Set<string>();
  for (const entrypoint of [...entrypoints, ...collectClientMutatingEntrypoints()]) {
    if (!seen.has(entrypoint.key)) {
      seen.add(entrypoint.key);
      entrypoints.push(entrypoint);
    }
  }
  return entrypoints;
}

describe('readFileOrThrow — missing-path guard (#1006)', () => {
  it('throws a clear, actionable error when the file does not exist', () => {
    const missing = path.join(GENERATED_DIR, 'does-not-exist.ts');
    expect(() => readFileOrThrow(missing)).toThrow(/expected file not found/);
  });

  it('reads the file normally when the path exists', () => {
    expect(() => readFileOrThrow(DOCS_PATH)).not.toThrow();
  });
});

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

// ── MuxAccountClient prototype coverage (#938) ────────────────────────────────

/**
 * Methods on `MuxAccountClient.prototype` that are NOT contract entrypoints
 * and must therefore not appear in `MUX_ACCOUNT_MUTATING_AUTHZ`:
 *
 * - `owner`    — read-only view function (simulation only, never submits).
 * - `buildTx`, `simulate`, `submit` — private client plumbing helpers that do
 *   not correspond to a contract entrypoint.
 *
 * This allowlist is explicit so the map cannot be padded: any other method on
 * the prototype is treated as a mutating entrypoint that requires an authz
 * mapping.
 */
const MUX_ACCOUNT_NON_ENTRYPOINT_METHODS = [
  'owner',
  'buildTx',
  'simulate',
  'submit',
];

function collectMuxAccountPrototypeEntrypoints(): string[] {
  return Object.getOwnPropertyNames(MuxAccountModule.MuxAccountClient.prototype)
    .filter((method) => method !== 'constructor')
    .filter((method) => !MUX_ACCOUNT_NON_ENTRYPOINT_METHODS.includes(method))
    .map(methodToEntrypoint)
    .sort();
}

describe('MUX_ACCOUNT_MUTATING_AUTHZ covers every mutating client method (#938)', () => {
  const mappedEntrypoints = Object.keys(MuxAccountModule.MUX_ACCOUNT_MUTATING_AUTHZ).sort();
  const prototypeEntrypoints = collectMuxAccountPrototypeEntrypoints();

  it('has an authz entry for every mutating method on MuxAccountClient.prototype', () => {
    const missing = prototypeEntrypoints.filter(
      (entrypoint) => !mappedEntrypoints.includes(entrypoint)
    );
    expect(
      missing,
      `MuxAccountClient exposes mutating methods without a MUX_ACCOUNT_MUTATING_AUTHZ entry: ${missing.join(', ')}`
    ).toEqual([]);
  });

  it('has no stale MUX_ACCOUNT_MUTATING_AUTHZ entries for absent methods', () => {
    const extra = mappedEntrypoints.filter(
      (entrypoint) => !prototypeEntrypoints.includes(entrypoint)
    );
    expect(
      extra,
      `MUX_ACCOUNT_MUTATING_AUTHZ lists entrypoints with no mutating method on MuxAccountClient: ${extra.join(', ')}`
    ).toEqual([]);
  });

  it('excludes read-only methods via the explicit allowlist, not by omission', () => {
    expect(MUX_ACCOUNT_NON_ENTRYPOINT_METHODS).toContain('owner');
    expect(mappedEntrypoints).not.toContain('owner');
    // Sanity: owner is genuinely on the prototype, so the allowlist is doing
    // the excluding rather than the method being absent.
    expect(Object.getOwnPropertyNames(MuxAccountModule.MuxAccountClient.prototype)).toContain('owner');
  });

  it('assigns a valid authorization level to every entry', () => {
    const validLevels = ['owner', 'delegate', 'guardian', 'public'];
    const invalid = Object.entries(MuxAccountModule.MUX_ACCOUNT_MUTATING_AUTHZ)
      .filter(([, level]) => !validLevels.includes(level))
      .map(([entrypoint]) => entrypoint);
    expect(invalid).toEqual([]);
  });

  it('documents every mux-account entrypoint in docs/access-control-checklist.md', () => {
    const doc = readFileOrThrow(DOCS_PATH);
    const undocumented = mappedEntrypoints.filter(
      (entrypoint) => !doc.includes(`| \`${entrypoint}\` | mux-account |`)
    );
    expect(
      undocumented,
      `docs/access-control-checklist.md is missing rows for: ${undocumented.join(', ')}`
    ).toEqual([]);
  });
});
