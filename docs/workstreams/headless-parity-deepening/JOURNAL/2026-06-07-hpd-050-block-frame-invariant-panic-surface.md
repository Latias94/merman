# HPD-050 - Block Frame Invariant Panic Surface

Date: 2026-06-07

## Context

Block's deep composite panic-surface work had already converted the expensive tree clone and
projection paths to explicit heap-backed traversal. The remaining target here was narrower:
explicit-stack frame `expect(...)` calls in parent-child population and document parsing.

Those frame stacks are internally controlled and should remain non-empty for normal input. They do
not need to panic the process if future parser or model drift violates that invariant.

## Change

- `BlockDb::populate_block_database(...)` now guards both explicit-stack frame lookups with
  `Option` branches and exits the loop if the stack is unexpectedly empty.
- Block document parsing now uses a small frame helper that returns a block `DiagramParse` error if
  the document-frame stack is unexpectedly empty.
- `finish_document_frame(...)` now returns `Result<()>`, so callers keep normal parse-error
  propagation instead of relying on `expect(...)`.

## Verification

- `cargo +1.95 fmt -p merman-core`
- `cargo +1.95 nextest run -p merman-core block`
- `rg -n 'populate frame should exist|document frame should exist|root document frame should exist|parent document frame should exist' crates/merman-core/src/diagrams/block.rs`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check
- `git diff --check`

## Notes

- No Block semantic JSON schema, typed render-model shape, renderer SVG output, fixture baseline, or
  root-bounds formula changed.
- Existing Block tests remain the normal-path guard for columns, spaces, class/style statements,
  named/anonymous composites, edge parsing, and the deep-chain regression.
