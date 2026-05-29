# ASCII Sequence Renderer Modularization - Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Smallest Current Repro

```bash
cargo nextest run -p merman-ascii sequence
```

This gate covers the current sequence behavior surface, including unsupported-feature diagnostics,
typed model rendering, and copied upstream sequence golden tests through the `sequence` filter.

## Gate Set

### Targeted Iteration Gate

```bash
cargo fmt --all --check
cargo nextest run -p merman-ascii sequence
cargo nextest run -p merman-ascii sequence_golden
```

### Package Gate

```bash
cargo nextest run -p merman-ascii
```

### Broader Closeout Gate

```bash
cargo nextest run -p merman --features ascii
cargo nextest run -p merman-cli --features ascii
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
git diff --check
```

Use the broader gate when a task changes public API, feature wiring, or package integration. For
pure internal module movement, targeted and package gates are the default proof.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Review must check that the task
is behavior-preserving and that control-block behavior was not silently folded into this lane.

## Evidence Anchors

- `docs/workstreams/ascii-sequence-renderer-modularization/DESIGN.md`
- `docs/workstreams/ascii-sequence-renderer-modularization/TODO.md`
- `docs/workstreams/ascii-sequence-renderer-modularization/MILESTONES.md`
- `crates/merman-ascii/src/sequence.rs`
- `crates/merman-ascii/tests/sequence_model.rs`

## Evidence Log

- 2026-05-29 ASRM-010: Opened the sequence renderer modularization lane after
  `ascii-sequence-parity` closeout. First executable task is a no-behavior extraction of internal
  sequence model and validation responsibilities from `sequence.rs`.
- 2026-05-29 ASRM-020: Extracted the internal ASCII sequence render model, typed-model adapter,
  autonumber handling, lifecycle model validation helpers, and unsupported-feature validation into
  `sequence/model.rs` and `sequence/validate.rs`. No public API or output behavior change is
  intended. Passed `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`. Follow-up package verification also passed:
  `cargo nextest run -p merman-ascii` and `git diff --check`.
- 2026-05-29 ASRM-020 review: No blocking workstream-compliance or code-quality findings. The
  diff stays inside the task boundary, keeps `sequence.rs` as the facade, moves typed-model
  semantics and validation out of the facade, and does not introduce control-block behavior.
