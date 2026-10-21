---
type: Work Progress
title: Class canonical semantics and explicit full projections
description: Complete the last canonical family and migrate Class render and ASCII consumers without implicit raw-field access.
timestamp: 2026-10-21T17:27:54Z
record_id: df8421a0b2614e788f0d7d016b2c71ad
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: ca140bcb6062c51d88b7a6c513082bdaaa6c45cb
---

# Ownership And Contracts

Class now consumes one grammar/DB construction into `ClassDiagramModel`.
Canonical data preserves authored namespace relationships, style witnesses and
stable identities. Full requests create the paired namespace/DOM/identity
projection; borrowed views preserve the existing full typed and compatibility
wire values, field order and static serialization lengths. Ordinary requests
omit editor collection; combined requests capture it during the same syntax pass.

The full owner privately pairs canonical data and presentation in one allocation.
Its getters are immutable; `into_model` moves the canonical owner for editing.
Remove aggregate `Deref`: raw `.namespaces` or `.notes` would otherwise silently
bypass projected parents and collision-free IDs. Full consumers now select
borrowed views explicitly, while semantic scalar reads use `model()`.
All full iterators expose their exact length, including the note index's work
charge before allocation. No production JSON bridge or complete graph clone is
introduced. Legacy full deserialization retains its already-projected fields;
it cannot reconstruct authored data that the old wire format discarded.

Render/layout/SVG and Class ASCII consumers use projected namespace membership,
parents, relation endpoints and synthetic identities consistently. Existing
adversarial full-model fixtures use the full serde boundary where reconstruction
would repair the invalid input; ordinary edits move and rebuild the pair.
Existing assertions, inputs and small-stack limits remain intact; API reads and
fixture construction are adapted. Two additional ASCII tests cover note/interface
ID collisions and exclusion of implicit containers in flat namespace mode.

# Corrections And Evidence

- An implicit namespace cycle originally never returned. RED nextest
  `e8d336b5-3b4b-4293-8ebf-0c4765538e65` timed out in the new proof.
  Bound ancestor traversal by the namespace graph's cardinality.
- Preserve public full `ClassComplexity` projected topology, while canonical
  complexity includes raw namespace depth. Retained full text counts the canonical
  owner plus the actual sidecar once. The independent fixture sidecar is 194 bytes;
  its exact budget passes and one byte less fails.
- The unchanged depth-128 parse test on a 128 KiB thread aborted before the sidecar
  constructor. Probe RED `23c54432-8ee9-4077-aef8-e02404b36119` reached completed
  lexing but did not return from grammar construction. A single private boxed pair
  reduced the full carrier from 1,112 bytes to 8 bytes; its canonical and sidecar
  payloads remain 728 and 384 bytes. GREEN
  `81308f8d-27d6-4468-a7b4-794612a453ba` used the same test and feature recipe.
  Final renderer validation below runs it without probes or altered stack size.
- LLVM/CodeView evidence is captured in ignored
  `target/agent-patches/u5-class-stack-evidence.json`, SHA256
  `61beeb6114b4fbf0e516a34d2e1218adfcb5f8f9d73a0fa053b70380636177c0`.
  Artifact hashes and feature differences are recorded there. Older all-family
  artifacts are not an equivalent performance baseline; no timing/allocation or
  final stripped-size claim follows from these frame observations.
- Independent review exposed four missing ASCII DOM-field consumers and further
  raw/projection ambiguity. The pre-fix production check reproduced all four
  `E0609` failures. Migrate the complete projection-sensitive paths, then remove
  aggregate `Deref` to make future omissions compiler-visible. Independent
  re-review cleared both findings without running Cargo.
- The five simplification lenses found two worthwhile efficiency changes and
  three local narration comments. Avoid building the collision set when there are
  no synthetic nodes; share topology counting instead of traversing labels whose
  result is immediately discarded. Preserve the proof tests.

# Authoritative Verification

All Cargo runs were serialized, with two build jobs and two nextest threads in
the existing F-drive checkout/target. No worktree was created.

| Recipe | Result | Nextest run |
| --- | --- | --- |
| Core `--no-default-features --features all-diagrams --lib` | 478 passed | `b97149fd-c932-43ac-b0de-d096e5a9570c` |
| Core `--all-features --lib --test parse_projection_contract --test diagram_features` | 2,164 passed | `83488f35-f608-4eac-b60d-f8b73d1f88df` |
| Render Class+ER+ELK library, Class layout and Class SVG | 1,563 passed, 1 skipped | `e799f468-5722-4ef6-81ce-8b0d500f1ef1` |
| ASCII Class-only library and `class_model` | 446 passed | `2cef6c33-1104-43f2-bd87-964370ccd128` |

Production Clippy `--all-features --lib -- -D warnings` passed for both core and
ASCII. `cargo fmt --all -- --check` and whitespace checks passed. The lean union
preceded the full-only accessor retirement; canonical definitions are unchanged.
The final ASCII lint follows a behavior-neutral removal of redundant borrows.
No probe or additional validation framework remains in production.

The first complete render retry failed at MSVC PDB linking while F had zero free
bytes. Remove only 114 obsolete project test executables/PDBs under the verified
`target/debug/deps` directory (14.32 GiB), keeping source, Git, evidence and other
projects untouched. The identical recipe then linked and passed. An earlier
Class-only recipe omitted ER for a mixed ER fixture; the final recipe explicitly
includes ER without weakening the test.

# Remaining Scope

All 34 canonical selectors are now admitted by the lean union. U5 is still open:
`editor-facts` must become actual compile-time isolation and independent capture.
U6 real lean consumers, late graphics and hosts, U8 products/generated/legal/name
migration, and U9 strict parity/comparable final-byte evidence remain unfinished.

The attempted all-family ASCII library-test build also exposed pre-existing U5
consumer fixture debt in `gantt.rs` and `xychart/render{,/empty}.rs`: struct literals
still target the old full fields. It did not pass and is not represented by the
Class-only result. Migrate those fixtures next without changing their assertions,
then restore the broad ASCII check as part of U6. Ordinary ASCII is still on its
full entry point; this direct Class repair alone does not satisfy the lean-consumer
requirement.

Fresh `git fetch origin` found no new main commits. Main
`3119c614bf8c43d49cfe17ded0a53ec2e7fa983c` is already the second parent of
`6bc29b741ed1dfd9f9b0e89aaaed87a4ec03f468`; no empty merge was created.
The goal remains active. No push, PR, publication or release was performed.
