# ASCII Sequence Parity - Evidence And Gates

Status: Closed
Last updated: 2026-05-29

## Focused Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

## Broad Gates

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-29 ASP-010: Opened sequence parity lane after graph fixture parity closed. Copied
  upstream sequence parity starts at 17 exact fixtures: 12 Unicode and 5 ASCII. First executable
  product slice is open-arrow sequence messages from typed `SequenceDiagramRenderModel` values.
- 2026-05-29 ASP-020: Added open-arrow sequence message support for typed message types `5`
  (`A->B`) and `6` (`A-->B`). Unicode output now keeps open arrows visually distinct from filled
  `->>`/`-->>` messages. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-040: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check` all passed.
- 2026-05-29 ASP-030: Split rich sequence work into independently executable tasks: single-line
  notes, sequence boxes, activations/create-destroy, and wrapping.
- 2026-05-29 ASP-050: Added single-line typed note rendering for `Note right of`, `Note left of`,
  and `Note over` while preserving copied upstream sequence fixtures. Wrapped and multiline notes
  remain explicitly unsupported. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-090: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check` all passed.
- 2026-05-29 ASP-060: Added typed sequence box rendering as a final text overlay around actor
  groups. Box labels render in the border; fill colors remain intentionally non-rendered in plain
  text. Wrapped, empty, and unknown-actor boxes are explicit unsupported features. Passed
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-100: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check` all passed.
- 2026-05-29 ASP-070: Added activation renderer state for typed `activate`/`deactivate` events and
  `+`/`-` message activation syntax. Active participants render with active lifeline bars. Split
  create/destroy actor lifecycle rendering into ASP-075. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-110: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check` all passed. Re-ran `git diff --check` after evidence updates; passed.
- 2026-05-29 ASP-075: Added actor create/destroy lifecycle rendering from typed
  `createdActors`/`destroyedActors` indices. Created participants are hidden until their creating
  message, render a participant box at creation time, and then keep a lifeline. Destroyed
  participants render an `x`/`×` termination marker and stop their lifeline afterward. Added
  solid/dotted cross message rendering for destroy-oriented sequence syntax. Passed
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-120: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`, and
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings` all
  passed. `git diff --check` passed after evidence updates.
- 2026-05-29 ASP-080: Added wrapped sequence message and note rendering. Wrapped messages render as
  multiple label rows bounded by terminal display width; wrapped notes render as taller note boxes.
  CJK text without spaces wraps by display width. Wrapped actor labels and wrapped boxes remain
  explicit unsupported features because they require a multi-line participant/group layout model.
  Passed `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-130: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`, and
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings` all
  passed. `git diff --check` passed after evidence updates.
- 2026-05-29 ASP-140: Closeout review closed the sequence parity lane and split richer Mermaid
  control blocks into follow-on scope. Fresh closeout verification passed:
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii sequence`,
  `cargo nextest run -p merman-ascii sequence_golden`, and `git diff --check`.
