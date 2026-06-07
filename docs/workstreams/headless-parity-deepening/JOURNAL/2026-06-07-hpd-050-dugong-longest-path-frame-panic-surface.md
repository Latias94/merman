# HPD-050 - Dugong Longest-Path Frame Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

The earlier Class namespace / Dugong deep traversal slice converted
`dugong::rank::util::longest_path(...)` from recursive DFS to an explicit frame stack. A follow-up
production panic scan still found one invariant `expect("longest-path frame should exist")` on the
final stack pop. The pop is guarded by `while let Some(frame) = stack.last_mut()` in normal control
flow, but keeping the `expect` would expose a library panic if that invariant drifted.

## Changes

- Replaced `expect("longest-path frame should exist")` with a defensive `let Some(frame) =
  stack.pop() else { break; }` branch.
- Preserved normal longest-path rank propagation, `minlen` handling, visited-node behavior, and
  missing-edge fallback behavior.
- Left Dugong ordering, Graphlib API shape, renderer SVG output, and residual classifications
  unchanged.

## Verification

- `cargo +1.95 fmt -p dugong` - passed.
- `cargo +1.95 nextest run -p dugong --test rank_util_test` - passed, `6` tests run.
- `rg -n 'longest-path frame should exist' crates/dugong/src/rank/util.rs` - no matches.
- `rg -n 'unwrap\(|expect\(|panic!\(' crates/dugong/src -g '*.rs'` - no matches.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `864`
  lines parsed.

## Boundary

This is a Dugong ranker panic-surface guard in an already iterative traversal. It does not claim
Class or Architecture residual closure and does not change normal rank output.
