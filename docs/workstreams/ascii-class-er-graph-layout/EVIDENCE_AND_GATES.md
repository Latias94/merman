# ASCII Class ER Graph Layout - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

The closed reference-expansion lane intentionally left class and ER graph layouts bounded to a
single relationship. Current executable diagnostics:

- `classDiagram` rejects multiple relationships with `multiple class relationships`.
- `classDiagram` rejects unrelated-class relationship layouts with
  `class relationship layouts with unrelated classes`.
- `erDiagram` rejects multiple relationships with `multiple ER relationships`.
- `erDiagram` rejects unrelated-entity relationship layouts with
  `ER relationship layouts with unrelated entities`.

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
cargo nextest run -p merman-ascii class er
```

Use the smallest focused gate for the active task. The combined filter is useful when touching the
shared placement boundary.

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

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACEG-010 | `git diff --check -- docs/workstreams/ascii-class-er-graph-layout` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
