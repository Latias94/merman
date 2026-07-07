# ADR 0071: Editor-Facing Parser and Semantic Seam

- Status: accepted
- Date: 2026-06-24

## Context

Merman already uses a mixed parser portfolio across diagram families. Some families are parser
generator backed; others are hand-written because their syntax shape fits that better. That is a
reasonable implementation choice.

The problem now is not parser technology selection by itself. Editor-facing consumers need stable
spans, recoverable partial results, semantic identity, and visible provenance for fallback behavior.
The old fence-local structure scans in `merman-lsp` were useful as a bootstrap, but they are not a
product-grade contract.

## Decision

- Keep parser technology family-local.
- Define a span-rich semantic seam between family parsers and downstream consumers.
- Treat recoverable partial parsing as a first-class contract for editor inputs.
- Classify parser facts by projection role. Entity facts may feed completion, references, and
  outline surfaces; outline facts may feed document symbols and hover without becoming completion
  candidates; payload facts preserve source spans for lint and future semantic consumers without
  being projected into the LSP migration index.
- Route analysis, lint, editor-core, WASM editor queries, and LSP features through semantic facts
  instead of raw-text structure scans.
- Keep render-model generation as a downstream projection, not the public parsing contract.

Temporary raw-text scans may remain during migration, but only as compatibility shims. They are not
the target architecture.

As of 2026-07-01, `merman-analysis::FenceTextIndex` is the shared semantic index and
`merman-editor-core` is the protocol-neutral query boundary. LSP and WASM editor APIs project
editor-core responses into their host protocols; they do not own separate completion, outline,
navigation, rename, or semantic-token scans.

As of 2026-07-02, editor snapshots also share the active analyzer configuration instead of creating
an independent analyzer lifecycle. Diagnostic-only rule changes can refresh diagnostics without
rebuilding editor snapshots. Parse options, site config, fixed date/time, resource limits, and
source descriptors are snapshot-affecting because they can change parser facts or editor indexes.
LSP keeps mechanical protocol projection helpers local to `merman-lsp`, and its semantic-token
legend is derived from the editor-core legend instead of maintaining a second token order.

Every editor-core result that depends on semantic facts carries `FenceTextIndexSource` provenance:
`ParserComplete`, `ParserCompleteDegradedSpans`, `ParserRecovered`,
`ParserRecoveredDegradedSpans`, or `TextScan`. Parser-backed and recovered results may be
first-class editor behavior when covered by tests. The `*DegradedSpans` variants are still
parser-backed facts, but their spans were produced in parser-input coordinates that could not be
proven as exact original-source ranges; downstream payloads expose `source_mapped_spans=false` and
must not use those spans for precise edits, rename ranges, or diagnostic source positions. Text-scan
results remain bounded fallback behavior and must stay visible in capability docs and tests.

`FenceTextIndex` must respect semantic roles when it projects parser facts. It should not treat
every parser-produced span as a graph-node completion id. For example, ER attribute names can be
outline facts, while attribute types, keys, and comments can be payload facts for lint without
polluting node-id completion.

Rich-facts projection errors are explicit internal analysis failures. If a flowchart parser model
matches the flowchart family but cannot be deserialized into the published facts shape, analysis
surfaces an internal diagnostic and omits only that typed facts projection; it does not silently turn
the failure into indistinguishable absence.

## Consequences

- `merman-core` gets a stronger contract for semantic facts and source locations.
- `merman-analysis`, `merman-editor-core`, WASM editor bindings, and `merman-lsp` can stop
  rediscovering structure from raw text on the supported families.
- Parser refactors become local to each family instead of forcing a repository-wide parser-generator
  rewrite.
- Recovery and span coverage become part of the test surface, which raises the parser maintenance
  bar.

## Alternatives Considered

### Single parser-generator rewrite

Rejected. A global migration would spend a lot of effort without solving recovery or semantic
indexing by itself.

### Continue heuristic downstream scans

Rejected. Heuristic scans are brittle for incomplete buffers and weaken locality for parser bugs.

### Separate editor-only parser stack

Rejected. That would duplicate behavior and split the public parsing surface.
