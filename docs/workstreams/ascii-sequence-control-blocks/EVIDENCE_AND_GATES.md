# ASCII Sequence Control Blocks - Evidence And Gates

Status: Closed
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
- 2026-05-29 ASCB-030: Implemented the first block-aware sequence render-plan slice for
  single-section `loop`, `opt`, and `break`. `merman-ascii` now converts those endpoint-less core
  control markers into internal control events, records rendered row spans, and applies labeled
  ASCII/Unicode text frames around contained message and note rows. Sectioned blocks remain
  unsupported as `control messages`; `rect` and `par_over` remain deferred. Fresh gates passed:
  `cargo nextest run -p merman-ascii sequence_single_section_control_blocks`,
  `cargo nextest run -p merman-ascii sequence_control_blocks`, `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii sequence_golden`,
  `cargo nextest run -p merman-ascii`, `git diff --check`, and
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`. CLI visual sanity checks for
  Unicode `loop`, `opt`, and `break` examples also rendered successfully.
- 2026-05-29 ASCB-030 review: No blocking workstream-compliance or code-quality findings. The diff
  stays inside the ASCB-030 scope, keeps sectioned blocks unsupported, introduces a dedicated
  `sequence/control.rs` frame renderer above low-level row painting, and leaves existing sequence
  golden fixtures unchanged.
- 2026-05-29 ASCB-040: Extended the control-block render plan to sectioned `alt`/`else`,
  `par`/`and`, and `critical`/`option` blocks. Internal control events now include section
  separators, `sequence/control.rs` renders labeled separator rows, and `rect`/`par_over` remain
  explicit deferred `control messages`. Tests cover ASCII/Unicode sectioned frames, repeated
  sections, and notes inside a section. Fresh gates passed:
  `cargo nextest run -p merman-ascii sequence_sectioned_control_blocks`,
  `cargo nextest run -p merman-ascii sequence_control_blocks sequence_deferred_control_blocks`,
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii sequence_golden`,
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`,
  `cargo nextest run -p merman-ascii`, and `git diff --check`. CLI visual sanity check for
  Unicode `alt`, `par`, and `critical` examples rendered successfully.
- 2026-05-29 ASCB-040 review: No blocking workstream-compliance or code-quality findings. The diff
  stays inside ASCB-040 scope, keeps `rect` and `par_over` deferred, reuses the dedicated control
  frame renderer for separator rows, and covers multi-section plus note-in-section behavior.
- 2026-05-29 ASCB-050: Settled the first edge-case policy for control blocks. Nested blocks return
  `nested control blocks`; empty sections return `empty control block sections`; `rect` and
  `par_over` remain deferred as `control messages`; activations, create/destroy lifecycle rows,
  notes, and participant boxes are covered as supported combinations. Fresh gates passed:
  `cargo nextest run -p merman-ascii sequence_nested_control_blocks sequence_empty_control_block sequence_control_blocks_support sequence_control_blocks_render_inside`,
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii sequence_golden`,
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`,
  `cargo nextest run -p merman-ascii`, and `git diff --check`.
- 2026-05-29 ASCB-050 review: No blocking workstream-compliance or code-quality findings. The
  change is test/docs focused, records explicit diagnostics for deferred edge cases, and covers the
  supported lifecycle and participant-box combinations without expanding scope beyond ASCB-050.
- 2026-05-29 ASCB-060: Generated manual inspection files:
  `D:\Frankorz\Downloads\merman-ascii-control-blocks-input.mmd`,
  `D:\Frankorz\Downloads\merman-ascii-control-blocks-unicode.txt`, and
  `D:\Frankorz\Downloads\merman-ascii-control-blocks-ascii.txt`. The examples cover `loop`, `opt`,
  `break`, `alt`/`else`, `par`/`and`, and `critical`/`option` in both Unicode and plain ASCII text
  output. README now names this sequence control-block subset in the ASCII/Unicode section.
- 2026-05-29 ASCB-060 verify: Fresh closeout gates passed: `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii` (70 passed),
  `cargo nextest run -p merman --features ascii` (4 passed),
  `cargo nextest run -p merman-cli --features ascii` (10 passed),
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check`.
- 2026-05-29 ASCB-060 review: No blocking workstream-compliance, code-quality, or missing-gate
  findings. The lane target is met, support docs and README state the shipped boundary, and the
  remaining parity debt is explicitly deferred to follow-ons rather than hidden inside this lane.
