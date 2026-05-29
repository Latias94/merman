# ASCII Sequence Parity - Design

Status: Active
Last updated: 2026-05-29

## Intent

Move sequence ASCII rendering from the initial tracer-bullet support into a measured, product-grade
lane. Graph parity is now closed; the remaining ASCII crate risk is mostly in sequence feature depth
and unsupported typed sequence semantics.

## Problem

`repo-ref/mermaid-ascii` has a deliberately small sequence renderer. The copied upstream sequence
fixtures already pass in `merman-ascii`, but our public crate consumes the richer
`SequenceDiagramRenderModel` from `merman-core`. That means users can parse common Mermaid sequence
syntax that the ASCII renderer still rejects, such as open-arrow sequence messages and richer
control constructs.

Without a dedicated lane, sequence work risks becoming a pile of ad hoc unsupported-feature
exceptions instead of a clear product boundary.

## Scope

- `crates/merman-ascii/src/sequence.rs`
- `crates/merman-ascii/tests/sequence_model.rs`
- `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- `crates/merman-ascii/README.md`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/SEQUENCE_FIXTURE_GAPS.md`
- `docs/workstreams/ascii-sequence-parity`

## Current Upstream Parity

Copied `mermaid-ascii` sequence fixtures are already exact after normalized whitespace comparison:

- Unicode fixtures: 12 / 12
- ASCII fixtures: 5 / 5

The upstream Go renderer supports participant boxes, lifelines, solid `->>`, dotted `-->>`, self
messages, labels, autonumber, ASCII output, and Unicode output. Upstream does not implement
activation boxes or loop/alt/opt/par blocks.

## Product Gap Direction

First target the gaps that are already represented in `SequenceDiagramRenderModel` and can be
rendered without changing parser contracts:

- Open sequence arrows: `->` and `-->` (done).
- Single-line notes: `Note left of`, `Note right of`, and `Note over` (done).
- Sequence boxes around actor groups (done).
- Activation state for `activate`/`deactivate` and `+`/`-` messages (done).
- Actor create/destroy lifecycle behavior, including cross destroy messages (done).
- Then wrapping as a separate slice.

Keep unsupported constructs explicit. Do not silently drop typed model semantics.

## Testing Plan

Focused gates:

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

Broad gates:

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Risk Plan

- Preserve all copied upstream sequence fixtures before expanding behavior.
- Add behavior through public `render_model`/`render_sequence` tests, not private helper tests.
- Keep ASCII output deterministic even when Unicode can distinguish more arrow glyphs than ASCII.
