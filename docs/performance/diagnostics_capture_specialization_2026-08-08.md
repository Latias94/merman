# Diagnostics-only capture decision — 2026-08-08 (superseded 2026-08-10)

## Decision

The deeper U12 diagnostics-only specialization was **rejected-not-admitted** on August 8, 2026.
The later architecture convergence unit completed on August 10, 2026 and deliberately removed
the public Flowchart-only facts projection together with its internal diagnostic. The current
`CaptureMode::DiagnosticsOnly` split remains, but both modes now share only generic typed warning,
source-lint, and editor-recovery candidates; no Flowchart facts projection exists in production.

No diagnostics latency or transient-memory claim is made.

## Current semantic boundary

`Analyzer::capture_from_evidence_cancellable` shares one parse/evidence snapshot for diagnostics
and rich analysis. In the parsed path, both modes run source lints, semantic warnings, and
editor-recovery candidates. Only the finishing projection differs:

- `DiagnosticsOnly` retains no syntax/index object in the returned capture.
- `RichFacts` materializes the complete editor text index and effective layout; generic node,
  reference, outline, semantic, and expected-syntax facts are derived from that index.
- Parse failures retain parser/recovery diagnostics in both modes and preserve the same
  disposition, spans, related notes, fixes, and cancellation behavior.

The former Flowchart projection was removed as part of the schema-2 wire break because no editor or
LSP operation consumed its graph. Its deletion is cross-binding API work, not a local performance
branch; diagnostics retain the generic parser-owned evidence required for recovery and policy
projection.

## Evidence and parity controls

The existing owner tests establish the safe baseline:

- `diagnostics_only_capture_does_not_materialize_syntax_indexes` proves the retained diagnostics
  object has no parser text index, semantic items, node IDs, or Flowchart facts.
- `rich_capture_materializes_parser_syntax_indexes` proves the rich oracle still retains the
  complete projections.
- facts schema tests reject schema 1 after envelope discrimination and preserve diagnostics schema 1.
- Public diagnostics and rich generation compare equal for representative parsed, recovered,
  Unicode, frontmatter, and custom-parser cases; cancellation checkpoints remain observable.

These are correctness controls, not an allocation or timing gate. No adjacent A/B, diagnostics
allocation counter, LSP/WASM/CLI public lane, or parser-policy migration was registered for this
optional candidate. The plan's required evidence for skipping projection objects is therefore not
available, and the semantic risk is material enough that a speculative branch would be worse than
retaining the current narrow split.

## Cleanup and future boundary

The rejected candidate adds no production code, switch, compatibility format, or dormant policy;
there is consequently no inverse implementation commit to apply. Existing `CaptureMode` is kept
because it is independently justified by the retained-object and parity tests above. A future U12
attempt must start from a new current-HEAD adjacent pair, instrument parser/family projection
objects separately, and prove diagnostics-only versus rich-facts byte/diagnostic identity before
changing `merman-core` capture signatures.
