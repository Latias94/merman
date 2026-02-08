# ADR-0050: Release Quality Gates (Parity Contract for Publishing)

## Status

Accepted

## Context

`merman` is a 1:1 re-implementation of Mermaid with a pinned upstream baseline (see ADR-0014).
For publishing a stable crate release, we need a clear, automatable definition of “good enough”
parity that:

- is robust to unavoidable browser/layout float behavior (Mermaid renders in a browser pipeline)
- still catches real regressions in parsing, semantics, and SVG structure
- remains maintainable as coverage expands across diagrams and fixtures

In practice, byte-for-byte SVG XML parity is not a realistic contract early on, because upstream
SVGs encode:

- browser-derived `getBBox()` numbers and serialization quirks
- layout engine differences (e.g. third-party layout libraries and float rounding)
- randomly generated IDs inside embedded icon SVGs (Iconify)

## Decision

### Release gates (must pass for publishing)

For a release, we require:

- Unit/integration test suite:
  - `cargo nextest run`
- DOM parity checks (stable regression gates):
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3`
- Root viewport parity (headless `getBBox()` approximation gate):
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

Notes:

- For Flowchart, use the vendored text measurer when running these gates locally to match the
  baseline corpus assumptions:
  - add `--flowchart-text-measurer vendored`
- `parity-root` compares the root `<svg>` viewport surface (`viewBox` + `style="max-width: …px"`).
  This is the most sensitive area to float lattice drift and is treated as its own gate.

### Stress checks (not blocking, but tracked)

We also run (locally or in non-blocking CI) a higher-precision viewport stress check:

- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 6`

This is explicitly a **stress test**, not a release gate, until the headless bbox + viewport
pipeline converges further.

### Strict mode is not a release gate

`--dom-mode strict` is intentionally not a publish gate.

Rationale:

- strict mode keeps more geometry and attribute detail (e.g. path `d`, transforms, and element text),
  which makes it a useful alignment tool, but it is expected to remain noisy until our layout and
  browser-adjacent behaviors converge.
- strict mode is best treated as a “parity KPI” (trendable mismatch counts) rather than a hard gate.

### Fixture-derived root viewport overrides are acceptable for publishing

For known upstream fixture deltas where browser float/serialization behavior is the dominant
source of drift, we allow fixture-derived root viewport overrides keyed by `diagram_id` (fixture
stem). These are sourced directly from the upstream SVG baselines and applied only to the root
viewport surface.

The override footprint is tracked under:

- `crates/merman-render/src/generated/*_root_overrides_11_12_2.rs`

And summarized via:

- `cargo run -p xtask -- report-overrides`

This keeps the release gates stable while we iteratively reduce the need for overrides.

## Consequences

- Releases are gated on deterministic DOM parity modes (`structure`/`parity`) and a stable root
  viewport contract (`parity-root` at 3 decimals).
- “Strict SVG XML equality” is not promised for early releases; it remains an explicit future
  convergence goal.
- Root viewport overrides become a first-class, auditable mechanism for managing browser float
  deltas without weakening semantic/structural regression coverage.

