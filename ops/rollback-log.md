# Rollback Log

Append-only record of executed rollbacks, one entry per incident. This
closes the tracking gap for the "Pre-Rollback Checklist" and "Post-Rollback
Steps" in
[docs/rollback-deploy.md](../docs/rollback-deploy.md#pre-rollback-checklist)
— those checklists previously had no completion record.

Validated by `scripts/check-rollback-log.sh` (run in CI on every PR) —
every entry below must contain all fields in the template, and both
checklist confirmations must be checked (`[x]`, not `[ ]`).

## Entry Template

Copy this block, fill it in, and add it under "New entries" below
immediately after a rollback is verified complete (see
[rollback-guide.md §4](../docs/rollback-guide.md#4-verify-the-rollback-succeeded)):

```
## <contract-or-scope> - YYYY-MM-DD
- Strategy used: <1|2|3> (see docs/rollback-deploy.md#rollback-strategies)
- Broken contract ID: C...
- Restored/new contract ID: C...
- Pre-Rollback Checklist completed: [x]
- Post-Rollback Steps completed: [x]
- Incident report: <link>
- Follow-up issue: <link>
```

---

## New entries

<!-- Add new entries above this line, most recent first. -->
