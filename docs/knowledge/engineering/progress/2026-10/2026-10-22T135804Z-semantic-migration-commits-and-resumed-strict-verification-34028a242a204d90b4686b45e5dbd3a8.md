---
type: "Work Progress"
title: "Semantic migration commits and resumed strict verification"
description: "Source and documentation migrations are committed; strict matrix continues, historical local raw evidence was removed by cargo clean."
timestamp: 2026-10-22T13:58:04Z
record_id: "34028a242a204d90b4686b45e5dbd3a8"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "018e358d4"
supersedes: "534c8bb2cfb54a7ca2f68d7914730079"
---

# Summary

The approved U1-U9 objective continues. Source, consumer, documentation and benchmark-recipe
migrations now have focused local commits. Integrated verification and final measurement remain
open; no push, PR, merge, publication or release is authorized.

# Details

Local commits:

- `6b2d15d19`: canonical semantic/full presentation routing, distinct artifacts, runtime inheritance,
  retained-source migration, canonical error handling, terminal helper capability, old-name
  retirement, consumer/test migration, and isolated-build target-directory handling.
- `87469a0e1`: current-source embedding/migration documentation, direct ASCII guidance and ADR-0098.
- `018e358d4`: benchmark features, probe fingerprints and workflow recipes. Owner thresholds,
  admission flags and semantic response digests are unchanged.

The earlier strict run passed workspace all-feature check and Clippy, then failed during an
isolated consumer build with Windows error 112, disk full. A cache deletion was rejected by tool
policy and did not execute. One debug-symbol file was moved without changing its bytes to
`E:/codex-builds/merman-semantic-capability-boundaries/preserved-debug/xtask.pdb`; its SHA-256 is
`5d5914d91d5db10af0584b3a715b8433720076cb5f71111fa38def5679915833`.
The subsequent external-target run remains live. After the maintainer ran `cargo clean`, F:
has approximately 837 GiB free. No source, fixture or unrelated edit was removed by this agent.

The cleanup also removed the ignored historical size, U3/U4 allocation, timing and parser
differential artifact directories. The dated historical reports and source commits remain in
Git, but their raw receipts and executables are no longer locally replayable. The two performance
reports have explicit retention updates. Do not recreate original samples from their tables or
reuse their measured numbers as current integration evidence.

Current verified evidence:

| Owner | Result | Evidence |
| --- | --- | --- |
| External strict workspace check | Passed | `target/semantic-integrated-strict-external.log` |
| External strict all-target/all-feature Clippy | Passed | Same log |
| External strict isolated consumers | Passed, including omitted-type rejection | Same log; curated/strict matrix follows |
| Late presentation terminal and prepared/operation regression | 76/76 passed | Surviving `target/semantic-presentation-terminal-green.log` |
| Performance infrastructure | 157/157 passed, exit 0 | `target/semantic-perf-contracts-current.log` |
| Native-memory driver contracts | 33/33 passed, exit 0 | `target/semantic-native-memory-contracts-current.log` |
| Legal generation/synchronization units | 18/18 passed, exit 0 | `target/semantic-legal-units-current.log` |
| Dart package resolution and analysis | Passed with local Dart 3.11.6 | `target/semantic-dart-pub-current.log` |
| Path budget and diff whitespace | Passed | Current command output |

The initial Dart offline resolution failed because the package cache lacked `ffigen ^23.0.0`.
Online resolution with the configured proxy then succeeded, followed by `dart analyze` with
no issues. This replaces only the dependency/static-analysis limitation of the old Flutter SDK;
the fresh native asset, ABI generation and runtime smoke still need their own verification.

Independent bounded review of observer-field elisions in seven remaining family modules returned
no findings. Its agent `01a163e4-3ebe-7430-924e-8109a51bd009` was closed. Earlier facade/core
request/docs reviewers are also closed. No delegated Cargo runs are authorized.

# Next Action

Exec session `4504` is still running strict verification. At this checkpoint it has entered
strict matrix build 166/661. Environment:

```text
CARGO_BUILD_JOBS=2
NEXTEST_TEST_THREADS=2
CARGO_INCREMENTAL=0
CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0
CARGO_TARGET_DIR=E:/codex-builds/merman-semantic-capability-boundaries/target
cargo run --locked -j 2 -p xtask -- verify --strict
```

Poll that handle and the log before another Cargo command; do not restart because it is slow.
The two-job limit is intentional. Preserve the pre-existing untracked
`crates/xtask/tests/fixtures/diagram-features-NqjeLP/`.

After strict, execute current parser differential, CLI exact process matrix and applicable
installed/native host checks sequentially. Fresh frozen size measurements must name the final
committed source and exercise the equivalent three-family meaning. Keep new measurement evidence
outside the Cargo target directory so later build-cache cleanup does not remove it.

U3 required public timing remains historically inconclusive, not admitted. Fresh adjacent
allocation/timing and final same-capability SVG controls need explicit identities and their
original acceptance rules. The goal cannot be marked complete from source migration or narrow
test passes alone.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`
- `docs/adr/0098-semantic-presentation-capabilities.md`
- `docs/migrations/render-reuse-host-integration.md`
- `docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`
- `docs/performance/semantic_parser_embedding_2026-10-22.md`
- `target/semantic-integrated-strict-external.log`
- `target/semantic-presentation-terminal-green.log`
- `target/semantic-perf-contracts-current.log`
- `target/semantic-native-memory-contracts-current.log`
- `target/semantic-legal-units-current.log`
- `target/semantic-dart-pub-current.log`
