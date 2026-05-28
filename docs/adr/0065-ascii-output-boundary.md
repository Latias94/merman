# ADR 0065: ASCII Output Boundary

Date: 2026-05-28

## Status

Accepted

## Context

`merman` is a Rust, browser-free Mermaid implementation. The existing renderer work focuses on SVG
and raster output, while downstream library users may also need terminal-friendly diagrams for
logs, documentation, CLI previews, chat systems, and environments where SVG is unavailable.

The reference repository `repo-ref/mermaid-ascii` contains a useful MIT-licensed Go implementation
of Mermaid-like ASCII and Unicode rendering. Its graph renderer includes grid placement, path
routing, junction merging, box drawing, and separate ASCII/Unicode character sets. Its sequence
renderer includes a compact participant/message layout. The repository is cloned under
`repo-ref/`, which is intentionally gitignored, so any shipped attribution, license text, and test
fixtures must live in tracked `merman` paths.

The upstream implementation also contains parser and application concerns that should not become a
second Mermaid implementation inside `merman`.

## Decision

Model ASCII output as a first-class rendering target with its own crate:

1. Add `crates/merman-ascii` for terminal/text rendering.
2. Make `merman-ascii` consume typed models from `merman-core` instead of parsing Mermaid syntax.
3. Keep ASCII layout independent from `merman-render` SVG layout. Character-cell layout is a
   separate product target, not a quantized SVG export.
4. Expose the crate through an opt-in `ascii` feature in the top-level `merman` crate after the
   renderer has a tested public API.
5. Preserve third-party attribution in tracked files before any derived code or copied fixtures ship.

The initial product target is stable, readable, deterministic text output for flowchart and
sequence diagrams. Exact byte-for-byte parity with `mermaid-ascii` is useful for algorithm port
tests, but the public product boundary is Mermaid semantic compatibility plus stable ASCII output.

## Consequences

- `merman` gains a non-SVG output surface without weakening the existing SVG parity boundary.
- ASCII snapshots become user-visible behavior and must be treated as semver-sensitive output.
- Unsupported Mermaid features need explicit degradation or structured diagnostics; silently
  misrepresenting diagram meaning is not acceptable.
- The Go reference can guide algorithm shape, but parser duplication, CLI/web code, and local
  `repo-ref` references must not enter the shipped crate.
- License and fixture provenance become part of the workstream evidence, not an afterthought.

## Non-Goals

- Do not implement a second Mermaid parser for ASCII output.
- Do not use browser, SVG, or pixel layout as the source of truth for ASCII coordinates.
- Do not make ASCII output the default `merman` rendering mode.
- Do not claim Mermaid CLI visual parity for text output.
- Do not ship copied upstream fixtures or derived source without tracked MIT license notice and
  source commit provenance.
