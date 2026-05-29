# ASCII Sequence Rect And ParOver Blocks - Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Smallest Current Repro

```bash
cargo nextest run -p merman-ascii sequence_rect_par_over
```

This gate will cover the focused `rect` / `par_over` tests once ASRP-020 lands.

## Gate Set

### Targeted Iteration Gate

```bash
cargo fmt --all --check
cargo nextest run -p merman-ascii sequence_rect_par_over
git diff --check
```

Use this gate for inventory and focused render behavior changes.

### Sequence Regression Gate

```bash
cargo nextest run -p merman-ascii sequence
cargo nextest run -p merman-ascii sequence_golden
```

Use this when changing control frame collection or text output.

### Package Gate

```bash
cargo nextest run -p merman-ascii
```

Use this after each completed implementation task.

### Broader Closeout Gate

```bash
cargo nextest run -p merman --features ascii
cargo nextest run -p merman-cli --features ascii
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
git diff --check
```

Use the broader gate before lane closeout or when touching package integration, public API, examples,
or CLI-visible behavior.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Review should check:

- endpoint-less `rect` and `par_over` signals are not silently dropped,
- `par_over` asymmetric start/end matching is explicit,
- unsupported nested and empty cases stay explicit,
- and terminal rendering preserves semantics without promising color parity.

## Evidence Anchors

- `docs/workstreams/ascii-sequence-rect-par-over/DESIGN.md`
- `docs/workstreams/ascii-sequence-rect-par-over/TODO.md`
- `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- `crates/merman-ascii/src/sequence/model.rs`
- `crates/merman-ascii/src/sequence/control.rs`
- `crates/merman-ascii/src/sequence/render.rs`
- `crates/merman-ascii/tests/sequence_model.rs`
- `crates/merman-core/src/diagrams/sequence_grammar.lalrpop`
- `crates/merman-render/src/svg/parity/sequence/block_collection.rs`
- `crates/merman-render/src/svg/parity/sequence/frames.rs`

## Evidence Log

- 2026-05-29 ASRP-010: Opened the `rect` / `par_over` follow-on lane after
  `ascii-sequence-control-blocks` closeout. Current inventory: core represents `rect` as
  endpoint-less line types 22/23, and `par_over` as line type 32 followed by normal `par` end line
  type 21. ASCII currently rejects both as `control messages`. First executable task is ASRP-020
  boundary/inventory tests.
