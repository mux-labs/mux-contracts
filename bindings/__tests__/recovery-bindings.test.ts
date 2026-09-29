import { describe, it, expect, beforeEach } from 'vitest';
import {
  RecoveryClient,
  RecoveryError,
  RecoveryErrorCode,
  GuardianThresholdConfig,
  validateGuardianThreshold,
} from '../recovery';

/**
 * Guardian N-of-M threshold specification tests (issue #846).
 *
 * Invariants under test:
 *  - Threshold is expressed as M-of-N: M approvals required out of N guardians.
 *  - 1 <= M <= N and N >= 1; zero guardians or zero approvals are rejected.
 *  - Guardian sets must be unique (no duplicate guardian addresses).
 *  - Only the owner (or an authorized guardian) may set/change the threshold.
 *  - Invalid configs fail closed with stable error codes.
 */

describe('guardian N-of-M threshold specification', () => {
  const owner = 'GOWNER000000000000000000000000000000000000000000000000000';
  const guardianA = 'GAAAA0000000000000000000000000000000000000000000000000000';
  const guardianB = 'GBBBB0000000000000000000000000000000000000000000000000000';
  const guardianC = 'GCCCC0000000000000000000000000000000000000000000000000000';
  const stranger = 'GSTRAN000000000000000000000000000000000000000000000000000';

  describe('validateGuardianThreshold', () => {
    it('accepts a valid M-of-N configuration', () => {
      const config: GuardianThresholdConfig = {
        threshold: 2,
        guardians: [guardianA, guardianB, guardianC],
      };
      expect(() => validateGuardianThreshold(config)).not.toThrow();
    });

    it('accepts a 1-of-1 configuration', () => {
      expect(() =>
        validateGuardianThreshold({ threshold: 1, guardians: [guardianA] }),
      ).not.toThrow();
    });

    it('rejects zero guardians (N = 0)', () => {
      expect(() =>
        validateGuardianThreshold({ threshold: 1, guardians: [] }),
      ).toThrowError(RecoveryError);
      try {
        validateGuardianThreshold({ threshold: 1, guardians: [] });
      } catch (err) {
        expect((err as RecoveryError).code).toBe(
          RecoveryErrorCode.InvalidGuardianThreshold,
        );
      }
    });

    it('rejects a zero threshold (M = 0)', () => {
      try {
        validateGuardianThreshold({ threshold: 0, guardians: [guardianA] });
        throw new Error('expected validation to fail');
      } catch (err) {
        expect((err as RecoveryError).code).toBe(
          RecoveryErrorCode.InvalidGuardianThreshold,
        );
      }
    });

    it('rejects M > N (threshold exceeds guardian count)', () => {
      try {
        validateGuardianThreshold({
          threshold: 3,
          guardians: [guardianA, guardianB],
        });
        throw new Error('expected validation to fail');
      } catch (err) {
        expect((err as RecoveryError).code).toBe(
          RecoveryErrorCode.InvalidGuardianThreshold,
        );
      }
    });

    it('rejects duplicate guardians', () => {
      try {
        validateGuardianThreshold({
          threshold: 2,
          guardians: [guardianA, guardianA],
        });
        throw new Error('expected validation to fail');
      } catch (err) {
        expect((err as RecoveryError).code).toBe(
          RecoveryErrorCode.DuplicateGuardian,
        );
      }
    });
  });

  describe('RecoveryClient.setGuardianThreshold', () => {
    let client: RecoveryClient;

    beforeEach(() => {
      client = new RecoveryClient({ owner });
    });

    it('allows the owner to set a valid threshold', async () => {
      const result = await client.setGuardianThreshold(
        { threshold: 2, guardians: [guardianA, guardianB, guardianC] },
        { caller: owner },
      );
      expect(result.threshold).toBe(2);
      expect(result.guardians).toHaveLength(3);
    });

    it('denies a non-owner caller by default', async () => {
      await expect(
        client.setGuardianThreshold(
          { threshold: 1, guardians: [guardianA] },
          { caller: stranger },
        ),
      ).rejects.toMatchObject({ code: RecoveryErrorCode.Unauthorized });
    });

    it('fails closed on an invalid threshold config', async () => {
      await expect(
        client.setGuardianThreshold(
          { threshold: 5, guardians: [guardianA, guardianB] },
          { caller: owner },
        ),
      ).rejects.toMatchObject({
        code: RecoveryErrorCode.InvalidGuardianThreshold,
      });
    });

    it('is idempotent for a repeated identical request', async () => {
      const config: GuardianThresholdConfig = {
        threshold: 2,
        guardians: [guardianA, guardianB],
      };
      const first = await client.setGuardianThreshold(config, { caller: owner });
      const second = await client.setGuardianThreshold(config, { caller: owner });
      expect(second).toEqual(first);
    });
  });

  describe('RecoveryClient.cancelRecovery', () => {
    let client: RecoveryClient;

    beforeEach(() => {
      client = new RecoveryClient({ owner });
    });

    it('allows the owner to cancel an in-flight recovery', async () => {
      const result = await client.cancelRecovery(
        { recoveryId: 'rec-1' },
        { caller: owner },
      );
      expect(result.recoveryId).toBe('rec-1');
      expect(result.status).toBe('cancelled');
    });

    it('denies a non-owner caller by default', async () => {
      await expect(
        client.cancelRecovery({ recoveryId: 'rec-1' }, { caller: stranger }),
      ).rejects.toMatchObject({ code: RecoveryErrorCode.Unauthorized });
    });

    it('fails closed on an unknown recovery id', async () => {
      await expect(
        client.cancelRecovery({ recoveryId: 'missing' }, { caller: owner }),
      ).rejects.toMatchObject({ code: RecoveryErrorCode.RecoveryNotFound });
    });

    it('is idempotent for a repeated cancel of the same recovery', async () => {
      const first = await client.cancelRecovery(
        { recoveryId: 'rec-1' },
        { caller: owner },
      );
      const second = await client.cancelRecovery(
        { recoveryId: 'rec-1' },
        { caller: owner },
      );
      expect(second).toEqual(first);
    });
  });

  describe('RecoveryClient.advanceRecovery', () => {
    let client: RecoveryClient;

    beforeEach(() => {
      client = new RecoveryClient({ owner });
    });

    it('allows the owner to advance a recovery to the next stage', async () => {
      const result = await client.advanceRecovery(
        { recoveryId: 'rec-1' },
        { caller: owner },
      );
      expect(result.recoveryId).toBe('rec-1');
      expect(result.status).toBe('advanced');
    });

    it('denies a non-owner caller by default', async () => {
      await expect(
        client.advanceRecovery({ recoveryId: 'rec-1' }, { caller: stranger }),
      ).rejects.toMatchObject({ code: RecoveryErrorCode.Unauthorized });
    });

    it('fails closed on an unknown recovery id', async () => {
      await expect(
        client.advanceRecovery({ recoveryId: 'missing' }, { caller: owner }),
      ).rejects.toMatchObject({ code: RecoveryErrorCode.RecoveryNotFound });
    });

    it('is idempotent for a repeated advance of the same recovery', async () => {
      const first = await client.advanceRecovery(
        { recoveryId: 'rec-1' },
        { caller: owner },
      );
      const second = await client.advanceRecovery(
        { recoveryId: 'rec-1' },
        { caller: owner },
      );
      expect(second).toEqual(first);
    });
  });
});
