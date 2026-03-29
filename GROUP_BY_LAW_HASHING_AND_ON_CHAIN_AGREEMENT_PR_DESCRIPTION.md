

## Repo Avatar
- `SoroSusu-Protocol/sorosusu-contracts`

## Summary

Every Susu group has "Unwritten Rules." This pull request makes those rules "Written and Hashed." When a group is created, the organizer uploads a JSON by-laws document (late fees, dispute rules, governance process, etc.) to IPFS. The contract stores the resulting CID and requires every member to sign the by-law hash before making their first contribution.

## What This PR Implements

- `ByLaw` CID storage in the circle metadata when a group is created.
- A new member onboarding flow that requires signing the by-law hash before the first deposit.
- `SignByLaws` membership state tracking to prevent contributions until agreement is recorded.
- Audit-ready on-chain evidence linking member consent to the exact IPFS by-law content.
- A digital paper trail that supports physical court enforcement if a large-scale dispute occurs.

## Key Features

### By-Laws on IPFS
- Organizer provides a JSON by-laws document at circle creation.
- The contract stores the IPFS CID with the circle.
- By-laws are immutable once stored and referenced by hash.

### Member Signature Requirement
- Each member must sign the by-law hash before their first contribution.
- Signing is enforced on-chain and recorded in member state.
- No contributions are allowed until the member explicitly agrees.

### Legal and Security Benefits
- Creates a verifiable paper trail for group rules.
- Supports dispute resolution with time-stamped member consent.
- Aligns smart contract behavior with real-world legal expectations.

## Labels
- `legal`
- `security`
- `backend`

## Test Coverage

- `test_group_creation_stores_bylaw_cid`
  - Verifies the circle stores the IPFS CID for the by-law document.
- `test_member_must_sign_bylaws_before_deposit`
  - Confirms a new member cannot deposit until the by-law hash is signed.
- `test_bylaw_signature_persists_across_sessions`
  - Ensures signed status remains recorded and prevents re-consent bypass.
- `test_invalid_signature_rejected`
  - Validates the contract rejects contributions from members who have not signed.
