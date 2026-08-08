# CLI transaction metadata decision — 2026-08-08

## Decision

The mandatory U10 target-index outcome is **accepted-structural**. The optional sealed-manifest /
compact-frontier persistence topology is **rejected-not-admitted** for this program; the existing
durable journal and dual-slot frontier remain the reference implementation.

No CLI throughput or filesystem-latency claim is made.

## Target-index boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `A` | `066bf86d4b59d748e0efb453f1a0e80efec240ad` | `917fb67e71facf5403ddda430ff9ee81d7bd6ea1` | Direct parent with per-request target lookup over the normalized entry list. |
| `B` | `764a9e01e42206fbdfdbed6834f3ed93f811f17a` | `38d123b7f491f68ef89099770810a8ab55c64d02` | Candidate that owns one immutable `HashMap<RelativeTarget, usize>` in `TransactionPlan` and transfers it into the staging transaction. |
| proof | `e5c9222d5be4bafd6bd42a3955af3a840bc4623a` | `38d123b7f491f68ef89099770810a8ab55c64d02` | Owner-local scale and lookup-count tests; no production change. |

The candidate is a direct parent/child change in `crates/merman-cli/src/transaction.rs`. The
normalization path still sorts artifacts, preserves manifest/document order, validates duplicate
and case/Unicode collisions before the map is built, and uses the same `RelativeTarget` key that
the publication and recovery code already trusts.

## Structural claim

For `N` planned entries, normalization builds the target index once in `O(N)` time and `O(N)`
owned space. Each `stage_slot` request performs one expected `HashMap` lookup, followed by the
existing ownership, generation, duplicate-issuance, and checkpoint checks. The test-only
`target_lookup_count` counter is exactly one per request at `N = 1, 16, 64, 256`; it is not a
production timing counter. Duplicate targets still fail before a usable plan is returned, so the
index cannot silently overwrite an earlier entry.

The old sequential search and its `O(N^2)` stage-slot lookup term are absent from the candidate
tree. No journal format, synchronization order, target-generation check, rollback rule, or commit
point changed in this unit.

## Verification

- `transaction_plan_indexes_every_target_across_representative_cardinalities` covers `N = 0, 1,
  16, 64, 256` and verifies every normalized target maps to its final slot.
- `stage_target_lookup_count_is_one_per_request_at_scale` covers `N = 1, 16, 64, 256` and
  verifies one indexed lookup per request.
- Duplicate targets, missing preflight generations, reserved-name collisions, Unicode/case
  collisions, out-of-plan targets, delete entries, and repeated stage issuance retain typed
  failures.
- The existing transaction integration suite continues to exercise lock ownership, target
  identity, generation checks, rollback, dual-slot persistence, recovery, tamper detection,
  synchronization checkpoints, and commit-last publication.

## Compact-persistence disposition

The current tree contains no sealed recovery manifest, manifest-bound compact frontier, or
candidate-only journal format. The only persistent state remains the existing staging journal and
dual-slot `JournalState` frontier, including its sequence, phase, next index, entry prior-state,
file/directory synchronization, and recovery authority rules.

The compact topology is rejected-not-admitted because no adjacent production candidate and no
Linux/macOS/Windows crash, tamper, mixed-generation, and foreign-file durability matrix exists.
Accepting a smaller journal without those proofs would weaken the plan's explicit durability
contract. This is a scope decision, not a claim that the baseline journal is slow; the mandatory
target index is independent and remains accepted.

## Claim boundary

This receipt admits `accepted-structural` only for target lookup cardinality. It does not claim
linear manifest/frontier bytes, reduced synchronization, lower filesystem wall time, or a compact
persistence win. The rejected optional topology leaves no production switch, dormant format, or
candidate-only helper in the final tree.
