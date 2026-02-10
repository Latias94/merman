# ADR 0060: Benchmarking Strategy

- Status: accepted
- Date: 2026-02-10

## Context

`merman` targets upstream Mermaid parity. This requires quality gates (DOM parity and golden
snapshots), but it is also important to track performance regressions as the implementation grows.

## Decision

- Use Criterion for local performance benchmarking of the headless pipeline.
- Keep the initial bench scope small and stable:
  - parse-only
  - layout-only
  - full render (parse + layout + SVG emission)
- Document how to run benchmarks and interpret results in `docs/performance/BENCHMARKING.md`.

## Consequences

- Benchmarks are not treated as strict CI gates (machine variance), but they provide a reliable
  regression signal for developers.
- The benchmark inputs should remain deterministic and representative; larger stress inputs can be
  added incrementally.

