# ASCII Sequence Renderer Modularization - Evidence And Gates

Status: Closed
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
- 2026-05-29 ASRM-030: Extracted participant layout calculation, lifecycle visibility planning,
  lifecycle edge lookup, and participant-left geometry into `sequence/layout.rs`. Row rendering,
  message rendering, note rendering, and control-block behavior remain out of scope. Passed
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii sequence_golden`, `cargo nextest run -p merman-ascii`, and
  `git diff --check`.
- 2026-05-29 ASRM-030 review: No blocking workstream-compliance or code-quality findings. The
  extraction stays inside the task boundary and leaves rendering behavior under the existing facade.
- 2026-05-29 ASRM-040: Extracted top-level render orchestration, row rendering, message/self-message
  rendering, note rendering, group-box overlays, and sequence-local text helpers into
  `sequence/render.rs`, `sequence/events.rs`, `sequence/notes.rs`, `sequence/boxes.rs`, and
  `sequence/text.rs`. `sequence.rs` is now a facade plus shared constants. No control-block
  behavior was added. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii sequence_golden`, `cargo nextest run -p merman-ascii`, and
  `git diff --check`.
- 2026-05-29 ASRM-040 review: No blocking workstream-compliance or code-quality findings. The diff
  completes the intended module-boundary extraction and keeps public API/output behavior stable.
- 2026-05-29 ASRM-050: Documented the final module boundary in `DESIGN.md` and confirmed that
  Mermaid sequence control blocks remain a separate follow-on lane. Passed
  `cargo nextest run -p merman-ascii` and `git diff --check`.
- 2026-05-29 ASRM-060: Closeout review closed the modularization lane. Target state is met,
  review findings are non-blocking, and remaining control-block work is split into
  `sequence-control-blocks` follow-on scope. Fresh closeout verification passed:
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii`, and `git diff --check`.
