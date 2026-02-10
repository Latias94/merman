# ADR 0052: Normalized Upstream Fixtures for CLI Baselines

Date: 2026-01-25

## Context

`merman` targets 1:1 parity with Mermaid `@11.12.2`. Our authoritative end-to-end baselines are
generated via the official Mermaid CLI pinned under `tools/mermaid-cli/`.

Some upstream inputs (notably Cypress rendering specs) include shorthand syntax that is
accepted by the browser bundle but rejected by the Mermaid CLI `@11.12.2` Langium parser. When the
CLI rejects an input, it produces an "error SVG", which cannot be used for SVG DOM parity with a
successful local render.

At the same time, we want to preserve upstream strings exactly as written for traceability.

## Decision

We will support **two fixture representations** when an upstream input is not CLI-renderable:

1. Keep the upstream string as a `*_parser_only_` fixture.
   - This fixture is semantic-only (`*.golden.json`) and is excluded from layout snapshots and
     upstream SVG baseline generation.
2. Add a `*_normalized` fixture variant that rewrites the input into Mermaid `@11.12.2`'s Langium
   grammar, making it **CLI-compatible**.
   - This fixture is eligible for:
     - layout golden snapshots (`*.layout.golden.json`)
     - upstream SVG baselines (`fixtures/upstream-svgs/**`)
     - DOM parity compares (`xtask compare-*-svgs --check-dom`)

## Scope

This approach is intended for:

- Syntax differences between Mermaid browser rendering and Mermaid CLI parsing at the pinned
  baseline version.

It is not intended to paper over:

- non-deterministic upstream layout (those should remain parser-only until we have a stable baseline
  source), or
- semantic differences where there is no clear grammar-preserving rewrite.

## Consequences

Pros:

- Preserves upstream fixtures exactly as written for traceability.
- Enables CLI-generated upstream SVG baselines + DOM parity for equivalent inputs.
- Keeps the parity target consistent (the pinned Mermaid CLI output).

Cons:

- Duplicates some fixtures (raw + normalized), increasing fixture count.
- Requires explicit documentation of the rewrite rules per diagram where this occurs.

## Example (Architecture)

The Architecture Cypress spec contains lines like:

- `db L--R server`
- `servC (L--R) servL`
- `servC L-[Label]-R servL`

These are rejected by Mermaid CLI `@11.12.2`, so we keep the raw strings as `*_parser_only_` and add
normalized variants using the Langium grammar, e.g.:

- `db:L -- R:server`

See `docs/alignment/ARCHITECTURE_UPSTREAM_TEST_COVERAGE.md`.
