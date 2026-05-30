# Breaking Change Policy

This document defines what constitutes a breaking change in the Mux Protocol smart contracts, the required deprecation periods, and how breaking changes must be communicated to users.

## Table of Contents

- [What Constitutes a Breaking Change](#what-constitutes-a-breaking-change)
- [Deprecation Period](#deprecation-period)
- [Communication Requirements](#communication-requirements)
- [Versioning Strategy](#versioning-strategy)
- [Process for Breaking Changes](#process-for-breaking-changes)

## What Constitutes a Breaking Change

A breaking change is any modification to a deployed contract that alters its public interface, storage structure, or error behavior in a way that causes existing client code or data to stop working correctly.

### Contract ABI Changes

Breaking changes to the contract's Application Binary Interface (ABI) include:

- **Function Signature Changes**: Modifying the name, parameter types, parameter order, or return type of any public contract function
- **Function Removals**: Removing a public function without a deprecation period
- **Parameter Type Changes**: Changing the type of any function parameter (even if serialization is compatible)
- **New Required Parameters**: Adding new non-optional parameters to existing functions

**Non-breaking examples:**
- Adding a new optional parameter (if the SDK supports it)
- Adding new public functions
- Changing internal (non-public) functions

### Storage Key Changes

Breaking changes to contract storage include:

- **Storage Key Renames**: Changing the identifier of any storage key (e.g., renaming `DataKey::Owner` to `DataKey::AccountOwner`)
- **Storage Key Removals**: Removing access to stored data without migration
- **Storage Schema Changes**: Modifying the structure of types stored in contract state (e.g., adding required fields to a struct without default values)

**Non-breaking examples:**
- Adding new optional storage keys
- Extending structs with optional fields that have sensible defaults

### Error Code Changes

Breaking changes to error handling include:

- **Error Code Value Changes**: Modifying the numeric value of any error code (e.g., changing `NotInitialized = 1` to `NotInitialized = 2`)
- **Error Removal**: Removing an error type that existing code might catch or handle
- **New Required Errors**: Adding new errors without providing a migration path

**Non-breaking examples:**
- Adding new error codes (especially at the end of the enum)
- Improving error messages without changing the error code value

### Behavior Changes

Breaking changes to function behavior include:

- **Authorization Changes**: Modifying which account(s) are required to authorize a function call
- **Logic Changes**: Altering the core business logic of a function in a way that changes its observable behavior
- **Validation Changes**: Adding new validation rules that reject previously-valid inputs
- **Result Changes**: Modifying what result is returned for the same input (affecting contract invariants)

**Non-breaking examples:**
- Fixing bugs that violated documented behavior
- Improving performance without changing observable behavior
- Adding additional authorization requirements if old behavior is still supported as a fallback

## Deprecation Period

All breaking changes must follow a mandatory deprecation period before removal or modification. This gives users time to update their code.

### Deprecation Timeline

- **Major Release with Deprecation Warning** (e.g., v2.0.0): Announce the breaking change, mark the old function/field as deprecated, provide alternative(s)
- **Minimum 30 days**: Allow at least one month for users to migrate
- **Next Major Release** (e.g., v3.0.0): Remove the deprecated feature or implement the breaking change

### Exceptions

Deprecation periods may be shortened (with clear communication) in cases of:
- **Security vulnerability fixes**: A critical security issue may require immediate breaking changes
- **Major bugs**: If existing behavior violates documented specifications, this should be documented and communicated clearly

## Communication Requirements

All breaking changes must be communicated clearly through multiple channels:

### CHANGELOG.md

Every breaking change **must** be documented in `CHANGELOG.md` under a `## [UNRELEASED]` or version-specific section with a `### ⚠️ BREAKING CHANGES` subsection.

**Format:**
```markdown
### ⚠️ BREAKING CHANGES

- **Function Signature Change**: `mux_account.debit_spend()` now requires a third parameter `asset_type: AssetType`. Update calls to include the asset type.
  - Migration: [Link to migration guide or example]

- **Error Code Change**: Error code for `Unauthorized` changed from `3` to `10`. Update error handling code accordingly.
  - Migration: Use the new error code `10` or catch the error by name instead of numeric value
```

### Pull Request Description

Any PR that introduces a breaking change **must**:

1. Include `BREAKING CHANGE:` in the PR description or commit message body
2. Clearly describe what is breaking and why
3. Provide migration guidance or a link to migration documentation
4. Indicate the planned version for removal (if deprecation is involved)

**Example PR description:**
```
## Breaking Change Notice

**What's breaking:** The `initialize()` function now requires a `version` parameter as the third argument.

**Why:** This allows contracts to track initialization version and enables future upgrades.

**Migration:**
```typescript
// Before
muxAccount.initialize(owner, guardians);

// After
muxAccount.initialize(owner, guardians, 1);
```

**Affected versions:** Deprecated in v1.5.0, removed in v2.0.0
```

### Release Notes

When releasing a version with breaking changes, release notes must prominently feature:
- A summary of all breaking changes
- Links to migration guides
- The deprecation timeline (if applicable)

## Versioning Strategy

Mux Protocol contracts follow **Semantic Versioning** (MAJOR.MINOR.PATCH):

- **MAJOR version**: Increment when making incompatible API changes (breaking changes)
- **MINOR version**: Increment when adding functionality in a backward-compatible manner
- **PATCH version**: Increment when making backward-compatible bug fixes

### Version Bump Rules for Breaking Changes

- **No breaking changes in MINOR or PATCH releases**: These must always be backward compatible
- **Breaking changes only in MAJOR releases**: A breaking change always triggers a major version bump
- **Deprecation announced in N, removed in N+1**: Deprecate in version N.0.0, remove in (N+1).0.0

**Example timeline:**
- **v1.0.0**: Initial release with `initialize(owner, guardians)`
- **v1.5.0**: Announce deprecation of `initialize()`, introduce new `initialize_v2(owner, guardians, version)`, keep old function working
- **v2.0.0**: Remove `initialize()`, only `initialize_v2()` available

## Process for Breaking Changes

Follow this process when introducing a breaking change:

1. **Identify the Change**: Determine if the change is truly breaking (see "What Constitutes a Breaking Change")

2. **Document the Change**:
   - Add the breaking change to `CHANGELOG.md` under `### ⚠️ BREAKING CHANGES`
   - Create or update migration documentation in `docs/`
   - Include deprecation timeline

3. **Implement with Backward Compatibility** (when possible):
   - Keep the old function/behavior available alongside the new one
   - Mark old functions with a `/// # Deprecated` doc comment
   - Document the migration path clearly

4. **Update PR/Commit**:
   - Include `BREAKING CHANGE:` in commit message body
   - Provide migration guidance in PR description
   - Link to migration documentation

5. **Release**:
   - Bump MAJOR version (or minor if within deprecation period)
   - Include breaking changes prominently in release notes
   - Consider creating a migration guide as a separate document

6. **Follow-up** (if deprecation):
   - In the next major release, remove the deprecated function
   - Document the removal in `CHANGELOG.md`
   - Bump MAJOR version again

## Questions?

If you're unsure whether a change is breaking, please:
- Open an issue and tag it with `question`
- Discuss with the team before implementing
- Err on the side of caution and assume it's breaking if there's any doubt

This policy ensures a smooth upgrade path for all Mux Protocol users.
