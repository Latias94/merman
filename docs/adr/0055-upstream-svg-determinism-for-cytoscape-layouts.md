# ADR-0055: Upstream SVG Determinism for Cytoscape Layouts (Architecture/FCoSE)

## Status

Accepted

## Context

`merman` uses upstream Mermaid SVG outputs as authoritative baselines (`fixtures/upstream-svgs/**`)
for 1:1 parity work.

For most Mermaid diagrams, the Mermaid CLI output is stable enough to treat as a byte-level golden
(or, at worst, a DOM-signature golden with minor numeric normalization).

However, Architecture diagrams in Mermaid `@11.12.2` rely on Cytoscape `fcose`, whose spectral
initialization uses `Math.random()`. This means:

- Re-generating upstream Architecture SVG baselines can produce different root viewports
  (`viewBox`, `style="max-width: ..."`), even with identical input text and environment.
- A fully deterministic Rust port (`manatee`) cannot reliably match an upstream baseline that is
  inherently stochastic unless we pick a deterministic upstream baseline strategy.

The project goal is “official Mermaid as the source of truth” *and* deterministic, headless Rust
outputs suitable for integration into other UIs. We therefore need deterministic upstream baselines
without weakening the feature set.

## Decision

- For Architecture upstream SVG generation, `xtask gen-upstream-svgs --diagram architecture` uses a
  Puppeteer wrapper that:
  - loads the official Mermaid CLI HTML bundle and Mermaid runtime
  - **seeds browser-side randomness deterministically** by overriding `Math.random()` (and
    `crypto.getRandomValues` when present) before Mermaid code executes
- Other diagrams continue to use the pinned `mmdc` binary directly.

## Rationale

- We keep Mermaid’s own renderer and layout logic as the baseline (no re-implementation on the JS
  side; only a deterministic RNG prelude).
- We make baseline regeneration reproducible across machines and over time.
- This aligns with `merman`’s determinism policy (explicit seeds, stable iteration order).

## Consequences

- Baseline generation for Architecture becomes deterministic and therefore “matchable” by a
  deterministic Rust port.
- The seeded Architecture baseline may differ from an arbitrary unseeded Mermaid CLI run, but this
  is the only practical way to define stable 1:1 baselines in the presence of upstream stochastic
  layout.

