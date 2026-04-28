# SoroSusu Mobile Integrator API Guide

This document is the primary reference for developers building a SoroSusu-compatible mobile wallet or frontend. It covers the XDR structures for every mobile-relevant function, the event schemas used for push notifications, and the Sequence Number strategy for offline transactions.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Contract Address](#contract-address)
3. [XDR Structures for Mobile Functions](#xdr-structures-for-mobile-functions)
   - [init](#init)
   - [create_circle](#create_circle)
   - [join_circle](#join_circle)
   - [deposit](#deposit)
   - [claim_pot](#claim_pot)
   - [pair_with_member](#pair_with_member)
   - [set_safety_deposit](#set_safety_deposit)
   - [late_contribution](#late_contribution)
   - [get_circle](#get_circle)
   - [get_member](#get_member)
   - [get_current_recipient](#get_current_recipient)
   - [get_user_summary](#get_user_summary)
4. [Event Schemas for Push Notifications](#event-schemas-for-push-notifications)
5. [Sequence Number Handling for Offline Transactions](#sequence-number-handling-for-offline-transactions)
6. [Error Code Reference](#error-code-reference)
7. [End-to-End Mobile Flow Example](#end-to-end-mobile-flow-example)

---

## Prerequisites

| Requirement | Version |
|---|---|
| Stellar SDK (JS/Dart/Swift) | ≥ 11.x |
| Soroban RPC endpoint | Mainnet: `https://soroban-rpc.mainnet.stellar.gateway.fm` |
| Token standard | SEP-41 / Stellar Asset Contract (SAC) |
| Auth model | `require_auth()` — every mutating call must be signed by the invoking address |

---

## Contract Address

```
CAH65U2KXQ34G7AT7QMWP6WUFYWAV6RPJRSDOB4KID6TP3OORS3BQHCX
```

Network: **Stellar Mainnet**

---

## XDR Structures for Mobile Functions

All function invocations are encoded as `InvokeContractArgs`. The examples below show the argument list in order.

### `init`

Initializes the contract. Called once by the deployer — mobile clients do not call this.

```
Function : init
Arguments:
  [0] admin   : Address
  [1] global_fee : u32   // penalty fee in basis points (e.g. 500 = 5%)
Returns  : void
Auth     : admin must sign
```

---

### `create_circle`

Creates a new savings circle. The creator automatically becomes the first member and must stake a bond.

```
Function : create_circle
Arguments:
  [0] creator          : Address
  [1] amount           : i128   // fixed contribution per round, in stroops
  [2] max_members      : u32
  [3] token            : Address  // SEP-41 token contract (e.g. USDC SAC)
  [4] cycle_duration   : u64   // seconds between rounds (e.g. 604800 = 1 week)
  [5] insurance_fee_bps: u32   // per-member insurance premium in bps
  [6] nft_contract     : Address  // SBT credential contract
Returns  : u64  // circle_id
Auth     : creator must sign
```

**JavaScript (stellar-sdk) example:**

```js
import { Contract, nativeToScVal, xdr } from "@stellar/stellar-sdk";

const args = [
  nativeToScVal(creatorAddress, { type: "address" }),
  nativeToScVal(BigInt(10_000_000), { type: "i128" }), // 1 XLM
  nativeToScVal(10, { type: "u32" }),
  nativeToScVal(tokenAddress, { type: "address" }),
  nativeToScVal(BigInt(604800), { type: "u64" }),
  nativeToScVal(100, { type: "u32" }),  // 1% insurance
  nativeToScVal(nftContractAddress, { type: "address" }),
];

const operation = contract.call("create_circle", ...args);
```

---

### `join_circle`

Adds the calling user to an existing open circle.

```
Function : join_circle
Arguments:
  [0] user      : Address
  [1] circle_id : u64
  [2] shares    : u32   // 1 (standard) or 2 (double share)
  [3] guarantor : Option<Address>  // null if no guarantor
Returns  : void
Auth     : user must sign
Errors   : panic("Circle is full") | panic("Already a member") | panic("Shares must be 1 or 2")
```

**Encoding `Option<Address>` as XDR:**

```js
// With guarantor:
nativeToScVal(guarantorAddress, { type: "address" })

// Without guarantor (None):
xdr.ScVal.scvVoid()
```

---

### `deposit`

Submits the user's contribution for the current round.

```
Function : deposit
Arguments:
  [0] user      : Address
  [1] circle_id : u64
Returns  : void
Auth     : user must sign
Notes    : The token transfer is executed internally. The user must have
           approved the contract to spend `amount` tokens beforehand.
```

**Pre-approval (SEP-41 `approve`):**

```js
// Approve the SoroSusu contract to pull `amount` from the user's balance.
const approveOp = tokenContract.call(
  "approve",
  nativeToScVal(userAddress, { type: "address" }),
  nativeToScVal(sorosusuContractAddress, { type: "address" }),
  nativeToScVal(BigInt(amount), { type: "i128" }),
  nativeToScVal(ledgerExpiry, { type: "u32" }),
);
```

---

### `claim_pot`

Claims the payout for the current round's recipient.

```
Function : claim_pot
Arguments:
  [0] user      : Address
  [1] circle_id : u64
Returns  : void
Auth     : user must sign
Notes    : Reverts if the user is not the current round's recipient, or if
           a LeaseFlow default lock is active on the circle.
```

---

### `pair_with_member`

Assigns a social buddy for security and recovery.

```
Function : pair_with_member
Arguments:
  [0] user          : Address
  [1] buddy_address : Address
Returns  : void
Auth     : user must sign
```

---

### `set_safety_deposit`

Deposits collateral into the circle's safety buffer.

```
Function : set_safety_deposit
Arguments:
  [0] user      : Address
  [1] circle_id : u64
  [2] amount    : i128  // in stroops
Returns  : void
Auth     : user must sign
```

---

### `late_contribution`

Pays a contribution after the deadline but within the grace period. A late fee is applied automatically.

```
Function : late_contribution
Arguments:
  [0] user      : Address
  [1] circle_id : u64
Returns  : void
Auth     : user must sign
Errors   : panic("Payment is not late. Use deposit function for on-time payment.")
```

---

### `get_circle`

Read-only. Returns the full state of a circle.

```
Function : get_circle
Arguments:
  [0] circle_id : u64
Returns  : CircleInfo

CircleInfo fields:
  id              : u64
  creator         : Address
  amount          : i128
  max_members     : u32
  member_count    : u32
  token           : Address
  cycle_duration  : u64
  current_round   : u32
  status          : CircleStatus  // Open | Active | CollectionPhase | PayoutPhase | Completed
  payout_queue    : Vec<Address>
  insurance_fee_bps : u32
```

---

### `get_member`

Read-only. Returns the membership record for an address.

```
Function : get_member
Arguments:
  [0] member : Address
Returns  : Member

Member fields:
  address            : Address
  circle_id          : u64
  shares             : u32
  contribution_count : u32
  status             : MemberStatus  // Active | Defaulted | Exited
  buddy              : Option<Address>
  guarantor          : Option<Address>
```

---

### `get_current_recipient`

Read-only. Returns the address that will receive the next payout, or `None` if no payout is pending.

```
Function : get_current_recipient
Arguments:
  [0] circle_id : u64
Returns  : Option<Address>
```

---

### `get_user_summary`

Read-only. Returns aggregated stats for a user across all circles.

```
Function : get_user_summary
Arguments:
  [0] user : Address
Returns  : Option<UserSummary>

UserSummary fields:
  total_contributions : i128
  circles_completed   : u32
  reputation_score    : u32
  on_time_payments    : u32
  late_payments       : u32
```

---

## Event Schemas for Push Notifications

Subscribe to contract events via the Soroban RPC `getEvents` endpoint. Filter by `contractId` and the topic symbols below.

### Endpoint

```
POST https://soroban-rpc.mainnet.stellar.gateway.fm
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getEvents",
  "params": {
    "startLedger": <ledger>,
    "filters": [{
      "type": "contract",
      "contractIds": ["CAH65U2KXQ34G7AT7QMWP6WUFYWAV6RPJRSDOB4KID6TP3OORS3BQHCX"],
      "topics": [["<TOPIC_SYMBOL>"]]
    }]
  }
}
```

---

### Event: `payout_distributed`

Fired when a round's payout is sent to the recipient.

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"payout_distributed"` |
| topic[1] | u64 | `circle_id` |
| data[0] | Address | `recipient` — address that received the payout |
| data[1] | i128 | `gross_payout` — amount sent in stroops |

**Push notification trigger:** "Your circle payout of {gross_payout} has been sent to {recipient}."

---

### Event: `round_finalized`

Fired when a round is closed and the next recipient is scheduled.

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"round_finalized"` |
| topic[1] | u64 | `circle_id` |
| data[0] | Address | `next_recipient` |
| data[1] | u64 | `scheduled_time` — Unix timestamp of next payout |

**Push notification trigger:** "Round complete! Next payout scheduled for {scheduled_time}."

---

### Event: `AUDIT`

Fired on every sensitive admin action (slash, eject, purge).

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"AUDIT"` |
| topic[1] | Address | `actor` |
| topic[2] | u64 | `resource_id` (circle_id or member index) |
| data[0] | u64 | `audit_id` |
| data[1] | u64 | `timestamp` |

---

### Event: `SbtMinted`

Fired when a Soulbound Token credential is issued to a member.

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"SbtMinted"` |
| data[0] | Address | `user` |
| data[1] | u128 | `token_id` |
| data[2] | SbtStatus | `status` |

**Push notification trigger:** "Congratulations! You earned a SoroSusu credential badge."

---

### Event: `MissingTrustline`

Fired when a yield payout is held because the recipient lacks a trustline for the circle token.

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"MissingTrustline"` |
| data[0] | u64 | `circle_id` |
| data[1] | Address | `member` |
| data[2] | i128 | `held_amount` — stroops held pending trustline |

**Push notification trigger:** "Action required: Add a trustline for {token} to receive your {held_amount} yield."

---

### Event: `stale_group_purged`

Fired when a dormant circle (≥ 5 years inactive) is purged by the admin.

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"stale_group_purged"` |
| topic[1] | u64 | `circle_id` |
| data[0] | Address | `admin` |
| data[1] | i128 | `residual` — funds returned to treasury |

---

### Event: `EXEC_TX` (Sequence Buffer)

Fired when a buffered offline transaction is executed.

| Field | Type | Description |
|---|---|---|
| topic[0] | Symbol | `"EXEC_TX"` |
| topic[1] | Address | `user` |
| data | BufferedTx | `{ seq: u64, action: u32, amount: i128 }` |

---

## Sequence Number Handling for Offline Transactions

SoroSusu includes a `SequenceBuffer` module that allows mobile wallets to pre-sign transactions while offline and submit them in order when connectivity is restored.

### How It Works

1. The contract tracks a `last_executed_seq` per user in storage.
2. The mobile wallet assigns a monotonically increasing `seq` to each pending action.
3. On reconnect, the wallet calls `submit_buffered_tx` for each pending action, then `process_buffered` to execute them in order.
4. The contract rejects any `seq ≤ last_executed_seq` (replay protection).

### Action Codes

| Code | Action |
|---|---|
| `1` | Contribute (deposit) |
| `2` | Withdraw |

### Functions

#### `submit_buffered_tx`

Queues a pre-signed transaction for later execution.

```
Function : submit_buffered_tx
Arguments:
  [0] user   : Address
  [1] seq    : u64    // must be > last executed seq for this user
  [2] action : u32   // 1 = contribute, 2 = withdraw
  [3] amount : i128
Returns  : void
Auth     : user must sign
Errors   : panic("Sequence too old or already processed")
           panic("Sequence already submitted")
```

#### `process_buffered`

Executes queued transactions in sequence order, up to `max_batch` at a time.

```
Function : process_buffered
Arguments:
  [0] user      : Address
  [1] max_batch : u32  // recommended: 10
Returns  : void
Auth     : user must sign
```

#### `get_next_sequence`

Returns the next expected sequence number for a user. Call this before going offline to seed the local counter.

```
Function : get_next_sequence
Arguments:
  [0] user : Address
Returns  : u64
Auth     : none (read-only)
```

### Offline Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  ONLINE                                                     │
│  1. Call get_next_sequence(user) → store as local_seq       │
│  2. User goes offline                                       │
├─────────────────────────────────────────────────────────────┤
│  OFFLINE                                                    │
│  3. For each action, sign: submit_buffered_tx(user,         │
│       local_seq++, action, amount)                          │
│  4. Store signed XDR envelopes locally                      │
├─────────────────────────────────────────────────────────────┤
│  BACK ONLINE                                                │
│  5. Submit each stored XDR envelope to the RPC             │
│  6. Call process_buffered(user, 10) to execute in order    │
└─────────────────────────────────────────────────────────────┘
```

### Sequence Gap Handling

If a gap exists in the sequence (e.g., seq 3 was submitted but seq 2 is missing), `process_buffered` will stop at the gap and wait. The wallet should detect this and either:
- Re-submit the missing transaction, or
- Abandon the gap by calling `submit_buffered_tx` with a no-op amount of `0` to advance past it.

---

## Error Code Reference

Functions that return `Result<_, u32>` use the following numeric error codes:

| Code | Meaning |
|---|---|
| `401` | Circle not found / Unauthorized |
| `403` | Member has not missed deadline (cannot execute default) |
| `404` | Grace period has not expired / Voting period still active |
| `405` | Member has not defaulted — nothing to slash / Vote already committed |
| `406` | Timelock has not yet expired (72-hour appeals window) / Vote already revealed |
| `407` | Tally failed — voting not complete |

Functions that use `panic!` (non-recoverable errors) include descriptive string messages. Mobile clients should catch these as transaction simulation failures before broadcasting.

---

## End-to-End Mobile Flow Example

The following sequence shows a complete happy-path flow for a new member joining and completing one round.

```
1. create_circle(creator, 10_000_000, 5, USDC_SAC, 604800, 100, NFT_CONTRACT)
   → circle_id = 1

2. join_circle(alice, 1, 1, null)
3. join_circle(bob,   1, 1, null)
4. join_circle(carol, 1, 1, null)
5. join_circle(dave,  1, 1, null)
   → circle is now Active (5/5 members)

6. [Round 1 — Collection Phase]
   approve(alice, SOROSUSU_CONTRACT, 10_000_000)
   deposit(alice, 1)
   ... repeat for bob, carol, dave, creator

7. [Round 1 — Payout Phase]
   get_current_recipient(1) → carol
   claim_pot(carol, 1)
   → Event: payout_distributed { circle_id: 1, recipient: carol, gross_payout: 50_000_000 }
   → Push: "Carol received 5 XLM from circle #1"

8. finalize_round(creator, 1)
   → Event: round_finalized { circle_id: 1, next_recipient: dave, scheduled_time: ... }

9. Repeat steps 6–8 for remaining rounds until CircleStatus = Completed.
```
