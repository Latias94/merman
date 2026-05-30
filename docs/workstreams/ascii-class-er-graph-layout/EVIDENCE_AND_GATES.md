# ASCII Class ER Graph Layout - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

The closed reference-expansion lane intentionally left class and ER graph layouts bounded to a
single relationship. Current executable capabilities and diagnostics:

- `classDiagram` renders layered extension chains and simple extension stars.
- `classDiagram` rejects unrelated-class relationship layouts with
  `class relationship layouts with unrelated classes`.
- `classDiagram` rejects crossing class relationship layouts with
  `crossing class relationship layouts`.
- `erDiagram` renders layered relationship chains and simple relationship stars.
- `erDiagram` rejects unrelated-entity relationship layouts with
  `ER relationship layouts with unrelated entities`.
- `erDiagram` rejects crossing relationship layouts with `crossing ER relationship layouts`.

Relevant files:

```text
crates/merman-ascii/src/class/render.rs
crates/merman-ascii/src/er/render.rs
crates/merman-ascii/tests/class_model.rs
crates/merman-ascii/tests/er_model.rs
```

## Gate Set

### Documentation And Hygiene

```bash
git diff --check
cargo fmt --all --check
```

### Focused Iteration

```bash
cargo nextest run -p merman-ascii class
cargo nextest run -p merman-ascii er
```

Use the smallest focused gate for the active task. The combined filter is useful when touching the
shared placement boundary; run both class and ER focused commands when a single change affects both
diagram families.

### Package And Public Gates

```bash
cargo nextest run -p merman-ascii
cargo nextest run -p merman --features ascii
cargo nextest run -p merman-cli --features ascii
```

Run public gates when public behavior or docs change.

### Lint

```bash
cargo clippy -p merman-ascii --all-targets -- -D warnings
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings
```

Use package clippy for implementation tasks; use broader clippy before closeout.

## Evidence Anchors

- `docs/workstreams/ascii-class-er-graph-layout/DESIGN.md`
- `docs/workstreams/ascii-class-er-graph-layout/TODO.md`
- `docs/workstreams/ascii-class-er-graph-layout/MILESTONES.md`
- `docs/workstreams/ascii-reference-implementation-expansion/`
- `crates/merman-ascii/README.md`
- `crates/merman-ascii/tests/class_model.rs`
- `crates/merman-ascii/tests/er_model.rs`

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | ACEG-010 | Opened follow-on lane from `ascii-reference-implementation-expansion` closeout. | Lane scope is limited to class/ER multi-relationship terminal graph layout. |
| 2026-05-30 | ACEG-020 | Added parser-backed tracer tests in `crates/merman-ascii/tests/class_model.rs` and `crates/merman-ascii/tests/er_model.rs`. | Current unsupported diagnostics are locked for class multiple relationships and ER unrelated-entity relationship layouts before layout refactoring starts. |
| 2026-05-30 | ACEG-030 | Added `crates/merman-ascii/src/relation_graph.rs` and routed class/ER single-relationship rendering through it. | Terminal placement is shared while class/ER adapters still own relationship semantics; focused snapshots stayed stable. |
| 2026-05-30 | ACEG-040 | Added layered class relationship layout for parser-backed extension stars and chains, with explicit crossing-layout diagnostics and support-doc updates. | Class multi-relationship output now shows every supported relation without silently dropping edges. |
| 2026-05-30 | ACEG-050 | Added layered ER relationship layout for parser-backed chains and stars, with explicit crossing-layout diagnostics and support-doc updates. | ER multi-relationship output now preserves cardinality, line style, and labels for supported layouts without silently dropping relationships. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACEG-010 | `git diff --check -- docs/workstreams/ascii-class-er-graph-layout` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACEG-020 | `cargo nextest run -p merman-ascii class` | Focused class ASCII tests | PASS, 12 tests | Class multiple-relationship behavior is explicitly unsupported through the public parser-backed render path. |
| 2026-05-30 | ACEG-020 | `cargo nextest run -p merman-ascii er` | Focused ER/filter gate | PASS, 77 tests | ER unrelated-entity relationship layout remains an explicit diagnostic through the public parser-backed render path; existing ER and substring-matched tests stay green. |
| 2026-05-30 | ACEG-030 | `cargo nextest run -p merman-ascii class` | Focused class ASCII tests | PASS, 12 tests | Class rendering uses the shared placement seam without changing existing class behavior. |
| 2026-05-30 | ACEG-030 | `cargo nextest run -p merman-ascii er` | Focused ER/filter gate | PASS, 77 tests | ER rendering uses the shared placement seam without changing existing ER or substring-matched behavior. |
| 2026-05-30 | ACEG-030 | `cargo fmt --all --check` | Workspace formatting check | PASS | Refactor and docs are formatted. |
| 2026-05-30 | ACEG-030 | `git diff --check` | Whitespace hygiene | PASS | Refactor and ledger updates have no whitespace errors. |
| 2026-05-30 | ACEG-040 | `cargo nextest run -p merman-ascii class` | Focused class ASCII tests | PASS, 14 tests | Class extension star and chain layouts render through the public parser-backed path; crossing layouts stay explicit diagnostics. |
| 2026-05-30 | ACEG-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | merman-ascii lint gate | PASS | New layered class layout code is warning-free under the package lint gate. |
| 2026-05-30 | ACEG-040 | `cargo fmt --all --check` | Workspace formatting check | PASS | Implementation and support docs are formatted. |
| 2026-05-30 | ACEG-040 | `git diff --check` | Whitespace hygiene | PASS | Implementation and ledger updates have no whitespace errors. |
| 2026-05-30 | ACEG-040 | `cargo nextest run -p merman-ascii er` | Shared seam regression check | PASS, 79 tests | ER output still passes after extending shared `relation_graph` box geometry helpers for class layout. |
| 2026-05-30 | ACEG-050 | `cargo nextest run -p merman-ascii er` | Focused ER/filter gate | PASS, 81 tests | ER chain and star layouts render through the public parser-backed path; crossing layouts stay explicit diagnostics. |
| 2026-05-30 | ACEG-050 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | merman-ascii lint gate | PASS | New layered ER layout code is warning-free under the package lint gate. |
| 2026-05-30 | ACEG-050 | `cargo fmt --all --check` | Workspace formatting check | PASS | Implementation and support docs are formatted. |
| 2026-05-30 | ACEG-050 | `git diff --check` | Whitespace hygiene | PASS | Implementation and ledger updates have no whitespace errors. |
