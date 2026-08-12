# ADR-0081: Release Quality Gates (Parity Contract for Publishing)

## Status

Accepted; updated 2026-08-11 for Mermaid `@11.16.1`, ADR-0050, and ADR-0062.

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
- Root viewport invariants and descendant parity:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

Notes:

- For Flowchart, use the vendored text measurer when running these gates locally to match the
  baseline corpus assumptions:
  - add `--flowchart-text-measurer vendored`
- `parity-root` keeps descendant parity blocking and invokes the root viewport contract for every
  fixture. The contract rejects an invalid root SVG, malformed or non-finite viewport geometry,
  non-positive dimensions, changed width/height strategy, changed non-numeric root style, and a
  changed `max-width`/`viewBox` relationship.
- A small exact set in `fixtures/_verification/deterministic-root-contracts.json` remains bound to
  the pinned input and upstream SVG hashes. These deterministic roots are exact release evidence,
  not production overrides or family tolerances.

### Browser diagnostics and cropping

Browser-owned exact bbox values are diagnostic rather than routine acceptance policy. Scheduled and
release evidence records the fixed browser identity plus exact root and painted-content rectangles,
so browser or font movement stays attributable without becoming a committed tolerance catalog.

Cropping remains blocking through an independent browser-mounted oracle. It mounts the final SVG,
derives painted descendant rectangles from browser layout rather than production bounds, and
requires SVG, HTML, MathML, and `foreignObject` content to remain inside the final viewport. Its
single epsilon covers coordinate quantization only; it is never fixture- or family-specific.

### Strict mode is not a release gate

`--dom-mode strict` is intentionally not a publish gate.

Rationale:

- strict mode keeps more geometry and attribute detail (e.g. path `d`, transforms, and element text),
  which makes it a useful alignment tool, but it is expected to remain noisy until our layout and
  browser-adjacent behaviors converge.
- strict mode is best treated as a “parity KPI” (trendable mismatch counts) rather than a hard gate.

### Production fixture overrides are forbidden

Root viewports are computed from family-owned or emitted-content bounds through the shared Root
Viewport module. Fixture ids and complete label strings are verification inputs, never production
lookup keys. Browser-owned exact numeric movement remains diagnostic; it cannot become a production
lookup, fixture-specific tolerance, or family acceptance envelope. Deterministic root fixtures and
root policy remain exact under ADR-0050, while ADR-0062 continues to forbid fixture-derived
production behavior.

## Consequences

- Releases are gated on deterministic DOM parity modes (`structure`/`parity`), blocking root
  invariants and deterministic root fixtures, and independent browser-mounted cropping containment.
- “Strict SVG XML equality” is not promised for early releases; it remains an explicit future
  convergence goal.
- Exact browser bbox movement remains attributable diagnostic evidence and cannot alter production
  output or weaken the blocking root contract.
