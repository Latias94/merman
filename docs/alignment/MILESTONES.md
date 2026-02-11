# Alignment Milestones (Mermaid@11.12.2)

This document tracks high-level alignment milestones for the pinned Mermaid baseline.

It is intentionally release-oriented (what “done” means) and should stay stable even as the
fixture corpus grows. For the detailed post-parity hardening phases, see:
`docs/alignment/PARITY_HARDENING_PLAN.md`.

## Baseline

- Mermaid baseline: `repo-ref/mermaid` at `mermaid@11.12.2` (see `repo-ref/REPOS.lock.json`).
- DOM gate: `parity-root` (root `<svg>` viewport + DOM structure, decimals = 3).

## Milestones

### M0: Baseline parity for current corpus (done)

Exit criteria:

- `cargo nextest run` is green.
- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3` is green.
- Upstream SVG baselines are stored under `fixtures/upstream-svgs/**` and are traceable to the
  pinned Mermaid CLI toolchain.

### M1: Fixture expansion with zero-regression parity gates

Goal:

- Increase confidence by importing more upstream tests/docs fixtures, while keeping M0 gates green
  after each batch.

Batch policy:

- Prefer small batches (10–30 fixtures) that share a single upstream source file.
- Every imported fixture must be traceable to an upstream path (and pinned commit via
  `repo-ref/REPOS.lock.json`).

Exit criteria:

- `parity-root` remains green for the expanded corpus.
- No “silent drift”: new fixtures must include semantic + layout snapshots, and (when applicable)
  upstream SVG baselines.

### M2: Reduce fixture-scoped root viewport overrides

Goal:

- Replace fixture-id keyed viewport overrides with deterministic, topology/semantics-driven logic
  where feasible.

Exit criteria:

- Override count decreases while M0 gates remain green.
- Each override removal is backed by either:
  - a reusable algorithmic change, or
  - an ADR explaining why the override remains necessary.

### M3: ZenUML compatibility (headless)

Goal:

- Expand practical ZenUML support for headless consumers while keeping Mermaid parity gates green.

Constraints:

- ZenUML is an external diagram upstream and is rendered via browser-only `@zenuml/core`.
- `merman` does not maintain upstream SVG baselines for ZenUML; it is snapshot-gated only.

Planned steps:

1. Import a small batch of examples from `repo-ref/mermaid/docs/syntax/zenuml.md` into
   `fixtures/zenuml/`.
2. Extend the translator in `crates/merman-core/src/diagrams/zenuml.rs` incrementally.
3. Gate changes on:
   - semantic snapshots (`fixtures/zenuml/*.golden.json`)
   - layout snapshots (`fixtures/zenuml/*.layout.golden.json`)

Exit criteria:

- ZenUML fixtures cover at least:
  - basic messages (`A->B: msg`, `A-->B: msg`)
  - titles and accessibility directives
  - at least one control-flow feature (e.g. loop/alt) *or* an explicit ADR explaining why it is
    deferred.

## Release notes

- Release/publishing gates are defined in `docs/releasing/PUBLISHING.md`.

