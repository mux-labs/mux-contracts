/**
 * Failing tests capturing gaps in audit-events.md coverage and contract event implementation.
 * This ensures every contract has proper audit event documentation and implementation.
 *
 * Gap: Event-topic conventions list contract tags, but audit-events.md may be missing
 * per-contract tables or have incomplete event coverage for mux-policy and mux-registry.
 *
 * Required for Mux Soroban audit and mainnet readiness.
 */

import * as fs from 'fs';
import * as path from 'path';

const REPO_ROOT = path.resolve(__dirname, '../..');

// Expected contracts based on event-topic-conventions.md and entrypoint-matrix.md
const EXPECTED_CONTRACTS = [
  'mux-account',
  'mux-account-factory', 
  'mux-permissions',
  'mux-delegation',
  'mux-batcher',
  'mux-policy',
  'mux-spending-policy',
  'mux-registry',
  'mux-wallet-registry',
  'mux-recovery'
];

// Expected contract tags from event-topic-conventions.md
const EXPECTED_CONTRACT_TAGS: Record<string, string> = {
  'mux-account': 'mux_acct',
  'mux-account-factory': 'mux_fac',
  'mux-permissions': 'mux_perm',
  'mux-delegation': 'mux_dlg',
  'mux-batcher': 'mux_bat',
  'mux-policy': 'mux_pol',
  'mux-spending-policy': 'mux_spend',
  'mux-registry': 'mux_reg',
  'mux-wallet-registry': 'mux_wreg',
  'mux-recovery': 'mux_recv'
};

describe('audit-events.md coverage validation', () => {
  let auditEventsContent: string;

  beforeAll(() => {
    auditEventsContent = fs.readFileSync(
      path.join(REPO_ROOT, 'docs/audit-events.md'),
      'utf8'
    );
  });

  it('should have audit event sections for all contracts', () => {
    for (const contractName of EXPECTED_CONTRACTS) {
      const sectionHeader = `## ${contractName} events`;
      expect(auditEventsContent).toContain(sectionHeader);
    }
  });

  it('should have contract tags for all contracts', () => {
    for (const [contractName, expectedTag] of Object.entries(EXPECTED_CONTRACT_TAGS)) {
      const tagPattern = new RegExp(`Contract tag: \`${expectedTag}\``);
      expect(auditEventsContent).toMatch(tagPattern);
    }
  });

  it('mux-policy events section should have complete event table', () => {
    const policySection = auditEventsContent.match(
      /## mux-policy events[\s\S]*?(?=## |$)/
    )?.[0];
    
    expect(policySection).toBeTruthy();
    
    // Check for required events based on implementation
    expect(policySection).toContain('`init`');
    expect(policySection).toContain('`lmt_set`');
    expect(policySection).toContain('`spent`');
    expect(policySection).toContain('`ctr_rst`');
    
    // Check for proper event table structure
    expect(policySection).toMatch(/\| Action \| Trigger \| Data payload \|/);
  });

  it('mux-registry events section should have complete event table', () => {
    const registrySection = auditEventsContent.match(
      /## mux-registry events[\s\S]*?(?=## |$)/
    )?.[0];
    
    expect(registrySection).toBeTruthy();
    
    // Check for required events based on implementation
    expect(registrySection).toContain('`init`');
    expect(registrySection).toContain('`reg`');
    expect(registrySection).toContain('`regmeta`');
    
    // Check for proper event table structure
    expect(registrySection).toMatch(/\| Action \| Trigger \| Data payload \|/);
  });

  it('should document read-only entrypoints that emit no events', () => {
    // mux-policy
    expect(auditEventsContent).toMatch(/`get_daily_limit`.*read-only.*no events/i);
    
    // mux-registry  
    expect(auditEventsContent).toMatch(/read-only.*no events/i);
  });

  it('should document upgrade behavior for TTL extension', () => {
    // Both contracts should document that upgrade extends TTL but emits no event
    const policySection = auditEventsContent.match(
      /## mux-policy events[\s\S]*?(?=## |$)/
    )?.[0];
    
    // Look for upgrade TTL behavior documentation
    expect(policySection).toMatch(/`upgrade`.*TTL.*does not emit/i);
  });
});

describe('contract implementation validation', () => {
  it('mux-policy should implement fail-closed auth', () => {
    const policyLibContent = fs.readFileSync(
      path.join(REPO_ROOT, 'contracts/mux-policy/src/lib.rs'),
      'utf8'
    );
    
    // Ensure require_auth is called in admin functions
    expect(policyLibContent).toMatch(/fn require_admin.*require_auth/s);
    expect(policyLibContent).toMatch(/set_daily_limit.*require_admin/s);
    expect(policyLibContent).toMatch(/reset_daily_counter.*require_admin/s);
  });

  it('mux-registry should implement fail-closed auth', () => {
    const registryLibContent = fs.readFileSync(
      path.join(REPO_ROOT, 'contracts/mux-registry/src/lib.rs'),
      'utf8'
    );
    
    // Ensure require_auth is called in admin functions
    expect(registryLibContent).toMatch(/fn require_admin.*require_auth/s);
    expect(registryLibContent).toMatch(/register.*require_admin/s);
    expect(registryLibContent).toMatch(/register_with_metadata.*require_admin/s);
  });

  it('should have no panic! or todo! on shipped paths', () => {
    const policyLibContent = fs.readFileSync(
      path.join(REPO_ROOT, 'contracts/mux-policy/src/lib.rs'),
      'utf8'
    );
    
    const registryLibContent = fs.readFileSync(
      path.join(REPO_ROOT, 'contracts/mux-registry/src/lib.rs'),
      'utf8'
    );
    
    // Check for panic! or todo! that shouldn't be on shipped paths
    expect(policyLibContent).not.toMatch(/panic!\s*\(/);
    expect(policyLibContent).not.toMatch(/todo!\s*\(/);
    expect(registryLibContent).not.toMatch(/panic!\s*\(/);
    expect(registryLibContent).not.toMatch(/todo!\s*\(/);
  });
});