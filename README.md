# SoroSusu: Decentralized Savings Circle
A trustless Rotating Savings and Credit Association (ROSCA) built on Stellar Soroban.

## Table of Contents
1. [Overview & Deployment](#overview--deployment)
2. [Protocol Overview](#protocol-overview)
3. [Safety & Formal Verification](#safety--formal-verification)
4. [Core Features](#core-features)
    - [Flexible Shares](#flexible-shares)
    - [Path Payments](#path-payments)
    - [Buddy System & Trust Networks](#buddy-system--trust-networks)
    - [Audit Logging](#audit-logging)
5. [Risk Management & Security](#risk-management--security)
    - [Collateral & Slashing](#collateral--slashing)
    - [Guarantors & Social Vouching](#guarantors--social-vouching)
    - [Nuclear Option & Emergency Recovery](#nuclear-option--emergency-recovery)
    - [Rate Limiting](#rate-limiting)
    - [Graceful Exit Plan](#graceful-exit-plan)
6. [Governance & Voting](#governance--voting)
    - [Quadratic Voting](#quadratic-voting)
    - [Leniency Voting](#leniency-voting)
    - [Governance Token Mining](#governance-token-mining)
7. [Advanced Modules](#advanced-modules)
    - [Milestone System & Reputation](#milestone-system--reputation)
    - [Yield Delegation & Vesting](#yield-delegation--vesting)
    - [Gas Buffers](#gas-buffers)
8. [Analytics & Reputation Engines](#analytics--reputation-engines)
    - [Credit Score Oracle](#credit-score-oracle)
    - [Group Resilience Rating](#group-resilience-rating)
    - [Source of Funds Verification](#source-of-funds-verification)
9. [Developer & Maintainer Guide](#developer--maintainer-guide)
10. [Testing & Verification](#testing--verification)

---

## Overview & Deployment
### Deployed Contract
- **Network:** Stellar Mainnet
- **Contract ID:** CAH65U2KXQ34G7AT7QMWP6WUFYWAV6RPJRSDOB4KID6TP3OORS3BQHCX

### Features
- Create savings circles with fixed contribution amounts
- Join existing circles with flexible shares (1x or 2x contributions)
- Deposit USDC/XLM securely
- Automated payouts with double rewards for 2nd-share members
- Immutable audit log for sensitive actions
- Family and small business friendly participation options

---

## Protocol Overview
### Randomized Payout Order
SoroSusu supports randomized payout queues to ensure fairness and prevent circular collusion.
- **Random Queue**: Uses Soroban's Pseudo-Random Number Generator (`env.prng().shuffle()`) to reorder the members vector.
- **Manual Finalization**: Admin triggers `finalize_circle()` to lock the payout order after members have joined.

### Group Rollover (Multi-Cycle Savings)
The protocol allows circles to persist across multiple cycles without redeployment.
- **Rollover**: Resets payout flags and increments the `cycle_number` while preserving the existing member list.

---

## Safety & Formal Verification
### Safety Invariants
1. **Vault Balance Integrity**: `vault_balance = total_deposits - total_payouts`.
2. **Fee Consistency**: Protocol fees must never exceed the payout amount (Capped at 100%).
3. **Contribution Completeness**: Payouts only occur when `contributions_count = member_count`.
4. **Member Uniqueness**: One membership per circle.
5. **Non-Negative Balances**: All arithmetic operations are checked for overflow/underflow.

### Verification Tools
- **Halmos**: Symbolic testing for `deposit()` and `payout()` logic.
- **Certora**: Formal verification of vault integrity and authorization checks.

---

## Core Features

### Flexible Shares
Members can participate at different contribution levels:
- **Standard Shares (1 Share)**: Contribute 1x, receive 1x payout.
- **Double Shares (2 Shares)**: Contribute 2x, receive 2x payout (Fixed pot size remains consistent).

### Path Payments
Enables members to contribute using any Stellar asset (XLM, USDC, EURC) via path payments, automatically swapping to the circle's base currency during the deposit.

### Buddy System & Trust Networks
Enhances security via decentralized cross-referencing:
- **Buddy Assignment**: Every member is paired with a "Buddy" responsible for tracking their health/activity.
- **Leniency Proposals**: Buddies can propose grace periods for missed payments based on real-world trust.

---

## Risk Management & Security

### Collateral & Slashing
To prevent defaults, the protocol supports mandatory or voluntary collateral.
- **Bond Staking**: Initial security deposit required for circle creation.
- **Slashing**: Defaulting members can have their bond or previous contributions slashed and redistributed to the circle's reserve.

### Guarantors & Social Vouching
- **Vouching**: High-reputation members can vouch for new entrants.
- **Guarantor Pool**: A separate staking pool that covers defaults in exchange for a portion of the circle's interest/fees.

### Nuclear Option & Emergency Recovery
- **Nuclear Option**: A majority-voted "Pause and Exit" mechanism for extreme market volatility.
- **Social Recovery**: Allows members to recover access to their participation NFT via consensus from their designated trust buddies.

### Rate Limiting
Prevents "DDoS" or "Spam Joining" through a tiered cooldown system for circle creation and membership joins within a single ledger window.

---

## Governance & Voting

### Quadratic Voting
Reduces the "plutocracy" effect by making subsequent votes from the same actor exponentially more expensive.
`Cost = Votes²`

### Leniency Voting
A specialized voting mechanism to grant extensions to members in temporary financial distress, preventing immediate slashing and maintaining group cohesion.

---

## Advanced Modules

### Milestone System & Reputation
Members earn rewards for consistent behavior:
- **Consecutive Payment Bonus**: 5, 10, or 12 cycles of perfect attendance.
- **Trust Score**: A dynamic reputation metric used to lower collateral requirements.

### Gas Buffers
A "Pay-Forward" pool where members contribute tiny amounts of XLM to cover the network fees of the circle's payout transactions, ensuring automated execution never stalls due to lack of gas.

---

## Analytics & Reputation Engines

### Credit Score Oracle
Integrates on-chain transaction data and off-chain reputation (via signed Attestations) to calculate a "DeFi Credit Score" for circle prioritization.

### Source of Funds Verification
Produces cryptographic proof of participation and contribution history, enabling users to prove the legitimacy of their accumulated savings to banking institutions.

---

## Developer & Maintainer Guide
### Build Instructions
```bash
cargo build --target wasm32-unknown-unknown --release
```

### Git Workflow (Maintainers)
- **Feature Branches**: `feature/[module-name]`
- **PR Labels**: Requires `audit-required` for logic changes.
- **Main Branch**: Protected; requires 1 pass of local integration tests.

---

## Testing & Verification
### Hyper-Inflationary Scenario Testing
Validates the protocol against extreme volumes and high-decimal tokens (18-decimal support).
- **Checks**: 1e27 stroop overflows, bitmap limits (64 members), and fee calculation precision.

### Graceful Exit Testing
Ensures that members can exit a circle midway if a replacement is found, preserving the circle's total capital while returning the exiting member's principal.
