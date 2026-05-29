# ASCII Sequence Control Blocks - Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Smallest Current Repro

```bash
cargo nextest run -p merman-ascii sequence
```

This gate covers the current sequence renderer, including unsupported-feature diagnostics and the
copied upstream sequence golden fixtures through the `sequence` filter.

## Gate Set

### Targeted Iteration Gate

```bash
cargo fmt --all --check
cargo nextest run -p merman-ascii sequence
cargo nextest run -p merman-ascii sequence_golden
git diff --check
```

Use this gate for focused control-block model, collection, and rendering changes.

### Package Gate

```bash
cargo nextest run -p merman-ascii
```

Use this gate after each completed implementation task.

### Broader Closeout Gate

```bash
cargo nextest run -p merman --features ascii
cargo nextest run -p merman-cli --features ascii
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
git diff --check
```

Use the broader gate before lane closeout or when touching package integration, public API,
examples, or CLI-visible behavior.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Review should check:

- control signals are not silently dropped,
- unsupported edge cases stay explicit,
- non-control sequence output remains stable unless the task intentionally changes it,
- and block collection remains separate from low-level row painting.

## Evidence Anchors

- `docs/workstreams/ascii-sequence-control-blocks/DESIGN.md`
- `docs/workstreams/ascii-sequence-control-blocks/TODO.md`
- `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- `crates/merman-ascii/src/sequence/model.rs`
- `crates/merman-ascii/src/sequence/render.rs`
- `crates/merman-ascii/tests/sequence_model.rs`
- `crates/merman-render/src/svg/parity/sequence/block_collection.rs`

## Evidence Log

- 2026-05-29 ASCB-010: Opened the sequence control-block lane after
  `ascii-sequence-renderer-modularization` closeout. Current inventory: `merman-core` represents
  control blocks as endpoint-less `SequenceMessage` control signals; ASCII currently rejects those
  as `control messages`; SVG parity already has a typed stack collector for the primary block
  forms. First executable task is ASCB-020 boundary/inventory tests.
- 2026-05-29 ASCB-020: Added executable inventory coverage for `loop`, `opt`, `break`, `alt`,
  `par`, and `critical`. The test proves `merman-core` emits endpoint-less control messages with
  the expected line type numbers and labels, and that `merman-ascii` still returns
  `UnsupportedFeature { feature: "control messages" }` for the block subset before rendering work
  begins. Updated `SEQUENCE_SUPPORT.md` to name this unsupported boundary. Fresh gates passed:
  `cargo nextest run -p merman-ascii sequence_control_blocks`, `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`, `git diff --check`, and
  `cargo nextest run -p merman-ascii`.
- 2026-05-29 ASCB-020 review: No blocking workstream-compliance or code-quality findings. The diff
  stays inside the task boundary, freezes the unsupported control-block boundary through public
  parse/render APIs, and does not introduce rendering behavior.
