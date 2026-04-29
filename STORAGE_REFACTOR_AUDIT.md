# ROSCA Storage Refactoring Audit

## Date: 2026-04-29
## Engineer: Senior Soroban/Rust Contract Engineer

## 1. CURRENT IMPLEMENTATION AUDIT

### Storage Structures Identified

#### Member Storage Pattern (BEFORE)
- **Location**: `src/lib.rs` lines 890-901
- **Structure**: Individual storage entries per member
  ```rust
  DataKey::Member(Address) -> Member {
      address: Address,
      index: u32,
      contribution_count: u32,
      last_contribution_time: u64,
      status: MemberStatus,
      tier_multiplier: u32,
      consecutive_missed_rounds: u32,
      referrer: Option<Address>,
      buddy: Option<Address>,
      shares: u32,
      guarantor: Option<Address>,
  }
  ```

#### Circle Member List (BEFORE)
- **Location**: `src/lib.rs` line 927
- **Structure**: `CircleInfo.member_addresses: Vec<Address>`
- **Purpose**: Track member addresses for iteration

### Access Patterns Analysis

#### Read Operations
1. **join_circle()** (line 2103):
   - Checks `circle.member_addresses.contains(&user)` - O(n) scan
   - Adds to `member_addresses` Vec
   - Creates individual Member storage entry

2. **deposit()** (line 2171):
   - Direct lookup: `DataKey::Member(user)` - O(1) storage read
   - Updates contribution_count
   - Writes back individual Member entry

3. **get_member()** (line 2383):
   - Direct lookup: `DataKey::Member(member)` - O(1) storage read

4. **count_active_members()** (line 1710):
   - Iterates `circle.member_addresses` - O(n)
   - Loads each Member individually - n storage reads
   - **OPTIMIZATION TARGET**: n+1 storage reads → 1 storage read

#### Write Operations
1. **join_circle()**: 3 storage writes (Circle, Member, CircleMember index)
2. **deposit()**: 3 storage writes (Member, Circle, Deposit flag)
3. **Social recovery**: Updates Member address in Vec

### Group Size Constraints
- **Configurable**: `max_members` per circle (line 2119)
- **Typical Range**: 5-20 members (ROSCA standard)
- **No Hard Cap**: Currently no global maximum enforced
- **Recommendation**: Enforce MAX_GROUP_SIZE = 20 for this optimization

### Storage Cost Analysis
**Current Pattern** (per circle with n members):
- 1 CircleInfo entry (contains Vec<Address>)
- n individual Member entries
- n CircleMember(circle_id, index) entries
- **Total**: 2n + 1 storage entries

**Proposed Pattern** (per circle with n members):
- 1 CircleInfo entry
- 1 Members Vec entry (contains all MemberRecord structs)
- **Total**: 2 storage entries

**Savings**: 2n - 1 storage entries per circle

### Files Requiring Changes
1. **src/lib.rs**: Core contract implementation
   - Lines 890-901: Member struct definition
   - Lines 906-948: CircleInfo struct
   - Lines 2103-2156: join_circle()
   - Lines 2171-2215: deposit()
   - Lines 2383-2387: get_member()
   - Lines 1710-1722: count_active_members()
   - Lines 1724-1790: apply_recovery_if_consensus()
   - Lines 2717-2810: late_contribution()
   - Lines 2812-2920: eject_member()

2. **Test Files** (all in tests/):
   - upgrade_safety_test.rs
   - grace_period_test.rs
   - bank_run_contributions_test.rs
   - anchor_deposit_test.rs (src/anchor_tests.rs)

### Gas/Instruction Cost Benchmarks
- **Not Found**: No existing benchmarks in test suite
- **Recommendation**: Add instruction counting tests post-refactor

## 2. OPTIMIZATION RATIONALE

### Why Vector Storage for Small Groups?

#### Soroban Storage Model
- **Map Storage**: Sorted structure, O(log n) lookup, separate ledger entries
- **Vec Storage**: Single ledger entry, O(n) iteration, lower constant overhead
- **Ledger Entry Fees**: Charged per entry read/write

#### Performance Analysis (n ≤ 20)
| Operation | Map (Current) | Vec (Proposed) | Winner |
|-----------|---------------|----------------|--------|
| Add Member | 3 writes | 2 writes | Vec |
| Find Member | 1 read | 1 read + O(n) scan | Tie* |
| Update Member | 1 read + 1 write | 1 read + 1 write | Tie |
| Iterate All | n reads | 1 read | Vec |
| Storage Entries | 2n+1 | 2 | Vec |

*For n ≤ 20, Vec scan is faster than Map lookup due to lower constant overhead

#### Cost Savings
- **Storage Rent**: ~90% reduction in ledger entries
- **Batch Operations**: count_active_members() goes from n+1 reads to 1 read
- **Write Amplification**: Reduced from 3 writes to 2 writes per member join

## 3. REFACTORING PLAN

### New Storage Structure
```rust
#[contracttype]
#[derive(Clone)]
pub struct MemberRecord {
    pub address: Address,
    pub contribution_count: u32,
    pub last_contribution_time: u64,
    pub status: MemberStatus,
    pub tier_multiplier: u32,
    pub consecutive_missed_rounds: u32,
    pub referrer: Option<Address>,
    pub buddy: Option<Address>,
    pub shares: u32,
    pub guarantor: Option<Address>,
    pub index: u32, // Immutable position in rotation
}

// Stored as:
DataKey::Members(u64) -> Vec<MemberRecord>
```

### Helper Functions
```rust
fn find_member(members: &Vec<MemberRecord>, addr: &Address) -> Option<u32>;
fn get_member_mut(members: &mut Vec<MemberRecord>, addr: &Address) -> Option<&mut MemberRecord>;
```

### Migration Strategy
- **Status**: Contract not yet deployed (greenfield)
- **Action**: No migration needed, direct implementation

### Acceptance Criteria
- [x] All Map<Address, Member> storage identified
- [x] Group size constraints documented (max 20)
- [x] Access patterns analyzed
- [x] Storage cost savings calculated
- [ ] MAX_GROUP_SIZE constant defined and enforced
- [ ] MemberRecord struct implemented
- [ ] find_member helper implemented
- [ ] All functions refactored
- [ ] All existing tests pass
- [ ] New tests achieve 95%+ coverage
- [ ] No unwrap()/expect() in production paths
- [ ] Inline comments explain optimization

## 4. RISK ASSESSMENT

### Low Risk
- Contract not deployed (no migration needed)
- Behavior-preserving refactor
- Small group size constraint well-established in ROSCA domain

### Medium Risk
- Test suite may rely on specific storage patterns
- Need to verify all address lookups handle "not found" gracefully

### Mitigation
- Run full test suite after each function refactor
- Add explicit error handling for all member lookups
- Document storage layout change in commit message
