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
ASCII remains an independent model-to-grid adapter rather than a projection of SVG geometry. It
receives the same operation control, runtime context, and operation-owned resource policy as the
SVG target, while keeping its own graph, routing, canvas, character-width, and output rules.
`max_grid_cells` is a structured ASCII resource quota; cancellation/deadline is a separate
operation outcome. The public seam is one model-level backend entry, not per-family source
render helpers.

## 2026-08 viewport/report addendum

The ASCII request boundary now also accepts a provider-neutral terminal-cell viewport through
`AsciiViewportPolicy`. The policy is intentionally separate from `AsciiRenderOptions`: callers own
the requested width and choose `Allow`, `Fallback`, or `Error`, while the renderer owns display-cell
measurement, complete typed fallback selection, and terminal-safe diagnostics. `AsciiOutput` is the
canonical renderer/facade result; the named string helpers are projections of its `text` field.

`Fallback` is not clipping, ellipsis, source disclosure, or a resource recovery path. It makes at
most one typed-model fallback attempt and reports the stable `primary_overflow` reason when that
attempt is selected. It returns `FallbackUnavailable` when the family cannot
preserve the required fields within the bound. Resource and cancellation errors retain their
existing precedence and never become fallback reports. `AsciiLayoutProfile::Compact` is an explicit
opt-in candidate; canonical geometry and the Issue #53 pre-layout label wrapping remain the default.

## 2026-08-25 contract-hardening addendum

Internal output seams may change during the alpha stabilization window. At this stage the merged
report vocabulary remained schema 1; the later encoding addendum below supersedes that transport
version. `AsciiOutput` owns one measured-candidate path for extent, display-cell,
grapheme, encoded-byte, and output admission observations. Fallback writers perform bounded local
checks while constructing a complete candidate, then commit its document admission once against the
render-wide resource ledger; failed candidates never become partial output.

`AsciiOutput::metadata()` is the canonical transport adapter input and `AsciiOutput::report()` is the
CLI report projection. Bindings, generated fixtures, Web/Playground metadata, and platform DTOs must
consume those fields rather than hand-copying report vocabulary. Projection and fallback capability
come from typed family capability records; output-string prefixes are not semantic classifiers.
Semantic fallback applies typed-model complexity preflight before family compatibility projection;
Flowchart's typed projection additionally uses a bounded, cancellation-aware JSON writer before
flattening, keeping intermediate materialization within the active ASCII output policy for bounded
profiles.

## 2026-08-26 layout, theme, and encoding addendum

The alpha contract now resolves one host request into narrower host, family-layout, visual-role, and
output policies. `AsciiRenderOptions` remains the public compatibility façade, but family renderers
consume resolved policy rather than interpreting a global density switch. Flowchart and Sequence
separately admit `AsciiLayoutProfile::Compact`; State, Class, ER, XYChart, and structured-text
families remain canonical-only until family-local evidence admits another profile. Flowchart Compact
uses a 24-cell ordinary-node wrap default and Sequence Compact uses three-cell participant spacing,
unless the caller explicitly overrides the corresponding family option. State does not inherit
Flowchart graph policy.

`AsciiColorMode::Ansi16` is the terminal-native styled profile. It keeps unstyled primary text at
terminal Reset and applies sparse named ANSI accents to semantic roles without detecting terminal
background polarity. Plain remains the deterministic default. Host environment signals such as TTY,
`NO_COLOR`, `TERM`, and `COLORTERM` are resolved by the CLI or another adapter before the renderer is
called.

`AsciiOutput` transport schema 2 adds explicit `encoding` identity for Plain, ANSI16, ANSI256,
TrueColor, and HTML results. Logical extent counts content rows; a final line terminator is encoded
output but not another logical row. CLI report mode is a machine-safe Plain channel and rejects an
explicit styled request. Viewport Fallback is likewise admitted only for Plain; styled fallback
requests fail capability preflight rather than returning ambiguous or partially styled structured
text. `Allow` and `Error` remain valid for every admitted primary encoding.

Family capability records are the request-preflight authority. In addition to semantic coverage and
projection, they publish admitted layout profiles, width profiles, primary encodings, and fallback
encodings. CLI, bindings-core, UniFFI, Web, Flutter, Python, Apple, and generated contract snapshots
must project those same source descriptors rather than maintaining transport-local admission tables.
