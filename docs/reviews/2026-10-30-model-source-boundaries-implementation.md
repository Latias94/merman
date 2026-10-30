# Model/source boundaries implementation record

Status: M0 in progress, 2026-10-30. This records implementation of the [approved plan](../plans/2026-10-30-model-and-source-boundaries.md), [family inventory](../plans/2026-10-30-family-data-inventory.md) and [ADR-0099](../adr/0099-model-and-source-boundaries.md). Open plan checkboxes remain requirements, not completed claims.

## Frozen source and observations

- Production base: `a5c73612fb2288e5effdbb07b3e1c3e683968603`, including the tested dependency-review prerequisite `8888358b1873c89049c0e430615c91d406dac224` and documentation only after it.
- Remote main: `4e9fa48139361bb30f81cb86d413597643978a4f`, fetched and inspected on 2026-10-30; no newer source commits needed integration at M0.
- Worktree: `merman-deps-review`, branch `plan/fearless-model-boundaries-20261030`. The primary checkout's unrelated untracked fixture is outside this work.
- Local experiment registration: shared ignored `target/bench/experiments/model-source-boundaries/experiment.yaml`. Receipts retain actual source, fixture, lock, profile, target and toolchain identities.
- Independent consumer fixtures and exact baseline commands are being prepared before production edits. There are no new dependency, size or compile-time results yet.

## Execution record

| Slice | Status | Evidence |
| --- | --- | --- |
| M0 | In progress | Source identities and experiment admission registered; fixed consumer recipes pending compilation. |
| M1–M8 | Open | No implementation or acceptance result yet. |

Existing correctness results at the prerequisite retain their original scope in the [dependency review](../research/2026-10-30-dependency-review.md). New consumer baselines and implementation gates will be recorded here as they complete. Do not infer a runtime improvement from the package boundary or rewrite historical measurement hashes.

## M1 extraction decisions

Shared operation/control, runtime policy/context, civil time/time-zone values and `SourceSpan` have no required source-parser dependency. Core keeps its ambient parse-context TLS and source-span scanning helpers; moving shared values does not require publishing those private orchestration functions. The operation's observed-cancellation path stays distinct from a new checkpoint, preserving writer I/O error ordering.

Pure lifecycle/runtime/time tests follow their types. Parse terminal/deadline and TLS restoration tests stay core-owned, with only the necessary feature-gated fake-clock/runtime test entrypoints across the package boundary. Adapter implementations and their capability query move together; core feature selection forwards to the actual shared owner.

Mindmap's compatibility projector currently derives its diagram ID from core's ambient context. Its M5 projection migration must receive an explicit operation context or frozen ID fact; the source-only TLS must not migrate into the model package to preserve this incidental call path.
