# Admin Centralization Audit — Issue #342

> **Purpose:** Pre-Zealynx audit self-review of every function that requires the
> `Admin` role. For each function we document (1) why admin privileges are
> required, (2) what happens if the admin key is lost, and (3) a concrete plan
> to migrate the function to a DAO-based multi-sig.

---

## Summary

| Function | Why Admin? | Key-Loss Risk | DAO Migration Path |
|---|---|---|---|
| `init` | Bootstrap — sets the initial admin | Low (one-time) | N/A |
| `set_lending_pool` | Trusted external contract address | Medium | Governance proposal |
| `set_protocol_fee` | Fee rate affects all payouts | High | Time-locked governance vote |
| `init_liquidity_buffer` | One-time buffer setup | Low | N/A |
| `trigger_payout` | Override payout trigger | Medium | Member supermajority vote |
| `purge_stale_group` | Irreversible storage deletion | High | DAO proposal + time-lock |
| `eject_member` | Removes a member permanently | High | Member vote (≥ 2/3) |
| `trigger_insurance_coverage` | Transfers insurance funds | High | Automated oracle + DAO |
| `set_leaseflow_contract` | Cross-protocol trust anchor | High | Governance proposal |
| `set_grant_stream_contract` | Cross-protocol trust anchor | High | Governance proposal |
| `set_hard_asset_basket` | Affects all basket circles | High | Governance vote |
| `update_price_oracle` | Price data integrity | High | Decentralised oracle network |

---

## Detailed Analysis

### `init(admin: Address)`

**Why admin-only:**  
Bootstraps the contract by recording the initial admin address and zeroing the
circle counter. Must be called exactly once; subsequent calls overwrite the
admin, which is intentional for emergency key rotation during the early
deployment phase.

**If admin key is lost:**  
The contract remains fully operational. All member-facing functions (`create_circle`,
`join_circle`, `deposit`, `finalize_round`) do not require admin auth. The only
consequence is that admin-gated functions (listed below) become permanently
inaccessible until a social-recovery mechanism is added.

**DAO migration path:**  
Replace the single `admin` address with a `MultiSigAdmin` struct holding N
addresses and a threshold M. Require M-of-N signatures for any admin action.
Emit an `ADMIN_ROTATED` audit event on every key change.

---

### `set_lending_pool(admin, pool: Address)`

**Why admin-only:**  
The lending pool is a trusted external contract. Allowing arbitrary callers to
point the protocol at a malicious pool would drain all deposited funds.

**If admin key is lost:**  
The lending pool address is frozen at its last-set value. Existing lending
integrations continue to work; new pools cannot be registered.

**DAO migration path:**  
Introduce a `propose_lending_pool_change` governance action with a 7-day
time-lock and a ≥ 60 % member vote. Emit a `LENDING_POOL_PROPOSED` event so
members can review before execution.

---

### `set_protocol_fee(admin, fee_basis_points: u32, treasury: Address)`

**Why admin-only:**  
The fee rate is deducted from every payout. An unconstrained caller could set
the fee to 100 % (10 000 bps), stealing all member funds.

**If admin key is lost:**  
The fee is frozen at its last-set value. The treasury address is also frozen.
Payouts continue at the existing fee rate; the protocol cannot be monetised
differently until key recovery.

**DAO migration path:**  
Cap fee changes to ±50 bps per governance cycle. Require a ≥ 51 % member vote
with a 48-hour voting window and a 24-hour time-lock before the new fee takes
effect. Emit a `FEE_CHANGE_PROPOSED` audit event.

---

### `init_liquidity_buffer(admin)`

**Why admin-only:**  
One-time initialisation of the liquidity buffer reserve. Prevents duplicate
initialisation that could reset the buffer balance.

**If admin key is lost:**  
The buffer is already initialised; this function is a no-op after first call.
No operational impact.

**DAO migration path:**  
Guard with `has(&DataKey::LiquidityBufferConfig)` and remove the admin check
entirely — the function becomes idempotent and safe for any caller.

---

### `trigger_payout(admin, circle_id: u64)`

**Why admin-only:**  
Payouts transfer the entire pot to a single recipient. Allowing arbitrary
callers to trigger payouts could enable griefing (forcing premature payouts)
or front-running attacks where an attacker triggers a payout before all members
have contributed.

**If admin key is lost:**  
Members can still call `distribute_payout` directly (which enforces the same
contribution-completeness check), so funds are **not** permanently locked. The
admin trigger is a convenience/override path only.

**DAO migration path:**  
Expose a time-locked `propose_trigger_payout` governance action that requires
a ≥ 2/3 member vote. This removes the single point of failure while preserving
the override capability.

---

### `purge_stale_group(admin, circle_id: u64)`

**Why admin-only:**  
Permanently deletes a circle's storage entry and transfers any residual
insurance balance to the treasury. This is irreversible; a non-admin caller
could maliciously purge active circles.

**If admin key is lost:**  
Stale groups accumulate indefinitely, increasing ledger rent costs. No funds
are at risk (residual balances remain in the contract), but storage efficiency
degrades over time.

**DAO migration path:**  
1. Add a `propose_purge` function callable by any member of the stale circle.
2. Require a ≥ 75 % vote from the circle's last-known members (or a 30-day
   no-objection window if members are unreachable).
3. Enforce the 5-year staleness check on-chain regardless of who calls it.
4. Emit a `PURGE_PROPOSED` audit event and a `PURGE_EXECUTED` event.

---

### `eject_member(caller, circle_id, member: Address)`

**Why admin-only (or creator-only):**  
Removing a member affects the payout queue and contribution bitmap. An
unconstrained caller could eject members to manipulate who receives the pot.

**If admin key is lost:**  
Members cannot be ejected for non-payment or misconduct. Defaulting members
remain in the queue, potentially blocking payouts indefinitely.

**DAO migration path:**  
Replace with a `propose_eject_member` vote requiring ≥ 2/3 of active circle
members. Add a 48-hour objection window. The ejected member's collateral is
slashed automatically on execution. Emit an `EJECT_PROPOSED` audit event.

---

### `trigger_insurance_coverage(caller, circle_id, member: Address)`

**Why admin-only:**  
Transfers funds from the group insurance pool to cover a defaulting member's
contribution. An unconstrained caller could drain the insurance fund.

**If admin key is lost:**  
Insurance claims cannot be processed. Members who default will not have their
contributions covered, potentially breaking the payout cycle.

**DAO migration path:**  
Automate via an on-chain oracle that monitors contribution deadlines. If a
member misses a deadline by > 48 hours, any circle member can call
`trigger_insurance_coverage` after providing a signed proof-of-default. Emit
an `INSURANCE_TRIGGERED` audit event.

---

### `set_leaseflow_contract(admin, leaseflow: Address)` / `set_grant_stream_contract(admin, grant_stream: Address)`

**Why admin-only:**  
These set trusted cross-protocol contract addresses. A malicious address could
intercept payout flows or falsely report defaults.

**If admin key is lost:**  
Cross-protocol integrations are frozen at their last-set addresses. Existing
integrations continue; new integrations cannot be added.

**DAO migration path:**  
Require a ≥ 60 % governance vote with a 7-day time-lock and a mandatory
security review period. Emit a `CROSS_PROTOCOL_PROPOSED` audit event.

---

### `set_hard_asset_basket(admin, gold_weight_bps, btc_weight_bps, silver_weight_bps)`

**Why admin-only:**  
The hard asset basket defines the reserve currency composition for all basket
circles. Changing it affects the value of every member's contributions.

**If admin key is lost:**  
The basket composition is frozen. Basket circles continue to operate at the
last-set weights; rebalancing is impossible.

**DAO migration path:**  
Require a ≥ 66 % supermajority vote across all active basket circles with a
14-day voting window and a 7-day time-lock. Cap individual asset weight changes
to ±10 % per governance cycle to prevent sudden composition shifts.

---

### `update_price_oracle(oracle_provider, asset, price)`

**Why admin-controlled:**  
Price data is used for collateral valuation and asset swap triggers. A
manipulated price feed could enable under-collateralised borrowing or
premature asset swaps.

**If admin key is lost:**  
Price data becomes stale. Collateral checks and asset swap triggers may use
outdated prices, potentially under- or over-valuing positions.

**DAO migration path:**  
Integrate with a decentralised oracle network (e.g., Reflector on Stellar).
Remove the admin-only gate and instead validate that the caller is a
whitelisted oracle provider registered via governance vote. Emit a
`PRICE_UPDATED` audit event with the provider's address for accountability.

---

## Trust Assumption Footprint

After the DAO migrations above, the remaining trust assumptions are:

1. **Multi-sig admin** (M-of-N) for emergency key rotation — unavoidable
   during the protocol's early life.
2. **Oracle providers** for price data — mitigated by using multiple
   independent providers and a median aggregation strategy.
3. **Time-locks** on all governance actions — ensures members have time to
   react to malicious proposals.

The goal is to reduce the single-admin trust assumption to zero within 12
months of mainnet launch, replacing it entirely with on-chain governance.

---

*Generated for Zealynx pre-audit review — Issue #342*
