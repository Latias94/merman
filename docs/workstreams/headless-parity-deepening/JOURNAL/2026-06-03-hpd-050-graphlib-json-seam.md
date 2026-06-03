# HPD-050 - Graphlib JSON Seam

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## What Changed

- Added [crates/dugong-graphlib/src/json.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/json.rs)
  and exposed `dugong_graphlib::json::{write, read}` as a public Graphlib-shaped serialization
  seam. The primary seam uses `Graph<Option<N>, Option<E>, Option<G>>` so upstream `undefined`
  maps to `None` and explicit JSON `null` remains `Some(Value::Null)`.
- Added [crates/dugong-graphlib/tests/json_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/json_test.rs)
  to port all six upstream cases from `repo-ref/graphlib/test/json-test.js`, plus a focused
  regression that distinguishes omitted labels from explicit `null`.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  so `json-test.js` is no longer an undecided gap.

## Findings

- The seam is justified by a real local need, not coverage vanity. HPD-050 already has Rust-side
  Graph/Dagre debug snapshots in `xtask`; landing a public Graphlib JSON seam prevents more
  Graphlib-shaped serializers from appearing ad hoc elsewhere.
- Rust default labels are intentionally kept on a separate fallback path:
  `write_with_defaults` / `read_with_defaults`. The main `write/read` seam is the source-backed
  one and preserves Graphlib's omitted-value shape through `Option<T>`.
- No renderer, solver, or layout behavior changed. This is a source-backed container seam plus
  direct test coverage, not an Architecture residual tune.

## Verification

- `cargo fmt --check --package dugong-graphlib` - passed.
- `cargo nextest run -p dugong-graphlib --test json_test` - passed, `8` tests.
- `cargo nextest run -p dugong-graphlib --tests` - passed, `95` tests.
- `cargo nextest run -p dugong --tests` - passed, `267` tests.
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl` - passed.
- `git diff --check` - passed with only existing LF/CRLF working-copy warnings for the JSONL files.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; implemented-matrix structural parity stayed green.
