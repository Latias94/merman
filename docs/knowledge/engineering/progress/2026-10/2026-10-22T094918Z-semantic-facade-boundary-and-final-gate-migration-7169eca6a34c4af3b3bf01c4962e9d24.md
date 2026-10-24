---
type: "Work Progress"
title: "Semantic facade boundary and final gate migration"
description: "Work Progress for Semantic facade boundary and final gate migration."
timestamp: 2026-10-22T09:49:18Z
record_id: "7169eca6a34c4af3b3bf01c4962e9d24"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "43deb2860e787d0290dc31bcfc6b1f07316ec278"
---

# Summary

The approved U1-U9 goal remains active. This receiving session continues the recovered
facade/API migration, preserving the inherited dirty worktree. The current HEAD is unchanged;
this checkpoint is not a commit or whole-plan completion receipt. No push, PR, merge,
publication or release is authorized.

# Details

Canonical facade semantic and ordinary ASCII operations use `SemanticParser` and a lean
`SemanticArtifact`. Explicit presentation requests, retained full sources and graphical
outputs use `MermaidEngine` and `PresentationArtifact`. The old `Engine`, `ParseMetadata`,
`with_engine` and format-neutral `prepare_semantic` source APIs are retired. CLI compatibility
JSON and binding-owned wire APIs select `compat-json` explicitly. Rustdoc rendering does not
need that facade feature.

A correctness review found that warming the inherited presentation engine before replacing
the semantic parser retained old config/runtime values. The corrected red run
`d3a367cd-9320-4afe-8f96-227f23f95776` reproduces both failures while the explicit-engine
independence control passes. `PresentationEngineSource` now stores either one inherited lazy
engine or one explicit engine. Replacing the parser invalidates only inherited defaults.
The green run `2796a7d7-3de3-44ae-b4a9-673cc9243821` passes all seven focused semantic,
ASCII, cache-config and cache-runtime tests. The first exploratory runtime test incorrectly
assumed `i64::MAX` was outside the project's expanded civil domain; the corrected tests observe
the actual frozen runtime instant instead. That exploratory failure is not the behavioral
red proof.

Core terminal normalization and its Unicode helpers now require the positive `terminal-text`
leaf. ASCII and facade diagnostic owners select it explicitly. Direct semantic core consumers
omit it. New lean rustdoc contracts reject presentation, theme-color, editor and terminal
imports when their capabilities are absent. These compile-fail checks still need their
dedicated lean execution.

Current recipes and documentation migrate explicit presentation/compatibility features,
full carrier ownership and the actual canonical ASCII API. `inspect_semantics` now inspects
canonical Flowchart meaning without JSON or SVG. Benchmark contracts retain their metric
bounds and semantic output digests while updating source fingerprints and required features.
The benchmark worker reports 189 pure Python tests passing; no new performance measurement
is claimed. Subsequent changes to the facade manifest require refreshing its three pinned
memory-contract input hashes.

Verified in this session:

- The recovered facade default tests/examples/benches check ended successfully.
- Bindings-core all-feature test targets compile successfully.
- CLI `ascii,diagram-flowchart,diagram-info` with defaults disabled compiles successfully.
- Web prepared-source, mount, descriptor and legal unit tests: 29 passed.
- Legal generator/synchronization unit tests: 18 passed.
- Node package unit run: 114 passed and one real locked-metadata test failed. Offline metadata
  refreshed only the standalone lock's `euclid` edge from core to renderer; the failed test
  then passed for both candidate transport recipes.
- Repository path-length and `git diff --check` checks pass.

The feature-matrix run passed structural validation and all sixteen isolated consumer
scenarios, then entered its 45 curated builds. It was intentionally stopped during build five
before the final capability edits, so it is not a complete current matrix pass.

The historical U3/U4 source, allocation, red/green, timing and SVG evidence was audited in
`docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`. Allocation observations remain
attached to their historical revisions. U3 public timing still has 22 inconclusive rows and
two confirmed JSON non-regression rows; complete performance admission remains unresolved.
The shared historical SVG candidate executable was overwritten, while its stored SVG outputs,
allocation probes, timing executables and baseline SVG executable still verify.

# Next Action

The only root-owned active Cargo build at this checkpoint is exec session `84553`:

```text
cargo nextest run --locked --workspace --all-features --no-fail-fast --test-threads 2 --cargo-quiet
```

Output is in `target/semantic-workspace-nextest.log`. Poll that session before another Cargo
build. It is still compiling; no all-workspace test outcome exists yet. Reuse `target` with
two build jobs.

Subagent `01a162ff-c117-7ad1-bfe9-9e214158d251` owns narrowly scoped core cfg-warning repairs,
excluding `lib.rs`, `semantic.rs`, `resources.rs`, `parse_options.rs`, manifests and tests.
It must not run Cargo. Review its changed paths and preserve all existing changes. Other
delegated document, evidence and benchmark workers have completed and were closed.

The core `diagram_features` target now retains canonical catalog/cancellation tests without
compatibility JSON, instead of making the no-feature CI command run zero tests. These two new
tests still need execution, including the no-family and selected-family configurations.

After compilation/tests, complete scoped lint, lean rustdoc/feature checks, current matrix,
legal/ABI/generated/package checks, strict parity, isolated committed size evidence and
applicable prepared/Web/host gates. Refresh the standalone fuzz lock (offline metadata already
did so) and verify its explicit full comparison feature. Keep inconclusive/platform-deferred
gates explicit; do not mark the goal complete based only on old receipts.

The pre-existing untracked `crates/xtask/tests/fixtures/diagram-features-NqjeLP/` remains
untouched and must not be staged merely because it appears in a broad search. Focused local
commits remain required after verification.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`
- `docs/migrations/render-reuse-host-integration.md`
- `docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`
- `docs/performance/semantic_parser_embedding_2026-10-22.md`
- `target/semantic-cache-red-corrected.log`
- `target/semantic-cache-green.log`
- `target/semantic-feature-matrix.log`
- `target/semantic-workspace-nextest.log`
- `target/semantic-web-unit.log`
- `target/semantic-legal-unit.log`
- `target/semantic-node-unit.log`
- `target/semantic-node-lock-green.log`
