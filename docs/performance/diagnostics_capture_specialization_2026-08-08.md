# Diagnostics-only capture decision — 2026-08-08

## Decision

The deeper U12 diagnostics-only specialization is **rejected-not-admitted**. The current
`CaptureMode::DiagnosticsOnly` split remains as the behavior-preserving baseline: it omits
navigation/text-index and rich Flowchart facts from the retained `AnalysisSyntaxFacts`, but it
still performs the diagnostic-bearing Flowchart projection needed for recovery and projection
diagnostics. No new parser capture policy, family registry mode, or candidate-only branch is left
in the tree.

No diagnostics latency or transient-memory claim is made.

## Current semantic boundary

`Analyzer::capture_from_evidence_cancellable` shares one parse/evidence snapshot for diagnostics
and rich analysis. In the parsed path, both modes still run source lints, semantic warnings,
editor-recovery candidates, and `flowchart_facts_projection_cancellable`. Only the finishing
projection differs:

- `DiagnosticsOnly` retains the diagram type and an unavailable `FenceTextIndex`; it does not
  materialize the editor text index or `AnalysisFlowchartFacts` in the returned syntax object.
- `RichFacts` materializes the complete editor text index, effective layout, and Flowchart facts.
- Parse failures retain parser/recovery diagnostics in both modes and preserve the same
  disposition, spans, related notes, fixes, and cancellation behavior.

The Flowchart projection cannot be removed from the diagnostics path by a local branch: its
candidate list carries schema/projection failures and diagnostics that are observable even when
the rich facts value is discarded. A parser-side capture policy would therefore have to preserve
those facts and recovery anchors across every combined-parser family before it could be evaluated
as a new candidate.

## Evidence and parity controls

The existing owner tests establish the safe baseline:

- `diagnostics_only_capture_does_not_materialize_syntax_indexes` proves the retained diagnostics
  object has no parser text index, semantic items, node IDs, or Flowchart facts.
- `rich_capture_materializes_parser_syntax_indexes` proves the rich oracle still retains the
  complete projections.
- `diagnostics_only_capture_reports_the_canonical_flowchart_projection_failure` and
  `diagnostic_reprojection_reuses_the_canonical_flowchart_projection_failure` preserve the
  malformed-Flowchart error contract.
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
