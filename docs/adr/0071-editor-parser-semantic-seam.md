# ADR-0071: Editor-Facing Parser and Semantic Seam

## Status

Accepted

## Dates

- Accepted: 2026-06-24
- Updated: 2026-07-15

## Context

Merman uses a mixed parser portfolio because Mermaid's diagram families use different grammar and
DB strategies. Editor consumers nevertheless need one trustworthy contract for spans, recoverable
partial input, semantic identity, expected syntax, and provenance.

The original fence-local structure scans in `merman-lsp` and the later generic analysis body scan
were useful bootstrap mechanisms, but they could guess node ids, symbols, and references that no
family parser had accepted. They also created a second grammar in downstream code and could disagree
with render semantics.

## Decision

### Parser technology is family-local; semantic facts are shared

- Each built-in family owns its successful semantic construction and its recoverable editor facts.
- The Diagram Family catalog declares the semantic parser, editor parser, aliases, profile gates,
  and authoring headers for every built-in id. Analysis does not maintain another family match.
- Recoverable partial parsing is a first-class family contract for incomplete editor buffers. It
  shares family tokens and grammar facts; it is not a separate editor-only successful grammar.
- Families that use Mermaid's Langium `common.langium` share the same span-rich title and
  accessibility parser. Families with different upstream syntax remain family-local.

Semantic facts are classified by projection role:

- **Entity** facts may feed completion, definitions, references, rename, hover, and outline.
- **Outline** facts may feed document symbols and hover without becoming completion identifiers.
- **Payload** facts retain source spans for diagnostics and semantic tokens without being promoted
  to entities.

Every renameable entity carries a family-owned `FenceRenamePolicy`. LSP does not impose a second
identifier grammar.

### Raw-text body semantics are removed

Analysis, editor-core, WASM editor queries, and LSP features use parser facts. Generic raw-text body
scans are not a fallback. If no parser-backed complete or recovered facts exist, the semantic index
is `Unavailable` and contains no body semantic items.

Source-start diagram headers and templates remain available from static Diagram Family catalog
facts. This is authoring metadata, not evidence that an unknown or malformed body was parsed.

Rich-facts projection errors are explicit internal analysis failures. If a typed family model
cannot be projected into its published fact shape, analysis reports that failure and omits only the
failed projection. It does not turn the error into indistinguishable absence or invoke a text scan.

### Provenance and span safety are explicit

Every editor result that depends on semantic facts carries `FenceTextIndexSource`:

- `ParserComplete`
- `ParserRecovered`
- `Unavailable`

Preprocessing produces a composable edit map for every deletion, replacement, normalization, and
entity placeholder. Facts are mapped independently to exact original-source coordinates. If a fact
crosses a span that cannot be represented exactly, only that fact is omitted and a recovery
diagnostic is attached; unrelated facts remain available. There is no parser-input coordinate mode
or whole-document degraded fallback. Serialized facts keep `source_mapped_spans` for wire
compatibility: it is true for parser-backed facts and false for `Unavailable`. `Unavailable`
produces no body completion, hover, symbols, navigation, rename, or semantic tokens.

### Analysis facts v1 is the sole parser-only wire contract

The diagnostics-only `AnalysisPayload` and richer `AnalysisFactsPayload` are independently
versioned. `AnalysisPayload` remains version `1`. `AnalysisFactsPayload` version `1` is the sole
parser-only semantic contract produced after this migration:

- `fact_source: "text_scan"` is removed and `"unavailable"` represents honest absence;
- every semantic item includes a required `rename_policy`;
- parser-backed, recovered, and source-mapped-span flags retain their explicit meanings; and
- no legacy decoder, executor, deprecated alias, or dual projection path remains.

Merman `0.8.0-alpha.3` exposed a TextScan-capable alpha shape with the same numeric discriminator.
This decision deliberately resets the contract before the first stable release: consumers of that
alpha shape must update to the current v1 schema, and version `1` alone is not evidence that an
obsolete alpha payload is compatible.

`AnalysisFactsPayload` is a binding wire projection, not the internal exchange format between
analysis, editor-core, and LSP. Those modules share typed `AnalysisResult`, `DocumentSnapshot`, and
`FenceTextIndex` data without a JSON round trip.

The facts schema version is unrelated to:

- LSP `textDocument.version`, which is a client document revision;
- Mermaid syntax and internal ids such as `flowchart-v2`, `stateDiagram-v2`, and
  `classDiagram-v2`; and
- C, WASM, UniFFI, or platform ABI/package versions.

### Ownership above analysis stays layered

- `merman-analysis::FenceTextIndex` is the shared semantic index and owns diagnostic/fact payload
  construction and source mapping.
- `merman-editor-core` owns protocol-neutral completion, hover, symbols, navigation, references,
  rename, selection, folding, code-action metadata, and semantic-token selection.
- `merman-lsp` owns request lifecycle, URI/range conversion, token delta encoding, capability
  advertising, and stale-result suppression. It does not own language semantics.
- WASM and other bindings project the same editor-core and analysis contracts into host types.

Editor snapshots share the active analyzer configuration. Diagnostic-only rule changes refresh
diagnostics without rebuilding semantic snapshots. Parse options, site configuration, fixed
date/time, resource limits, and source descriptors invalidate snapshots because they can change
parser facts or source mapping.

## User-Visible Behavior

- Completion, hover, symbols, definition, references, rename, folding, and semantic tokens keep
  working for parser-complete and tested parser-recovered families.
- Refactoring becomes stricter and safer because candidate identity, references, and rename policy
  come from the same family semantics used by parsing and rendering.
- Unknown, unsupported, or unrecoverable body text no longer receives guessed identifiers or
  navigation targets. Legal source-start header completion remains available.
- Consumers that require exact edits must check span provenance rather than assuming every
  parser-backed result has original-source coordinates.

## Consequences

- Parser bugs and recovery gaps stay local to the owning family.
- Editor behavior cannot silently diverge into a protocol-specific grammar.
- Removing guessed semantics may reduce apparent feature output on unsupported input, but that
  reduction is a correctness improvement rather than a capability regression.
- Recovery and exact span coverage are part of the family admission and test surface.
- Facts consumers have one current contract and no permanent runtime compatibility split.

## Rejected Alternatives

### Rewrite every family with one parser generator

Rejected. It would erase useful source-backed parser differences without solving recovery or
semantic projection by itself.

### Continue heuristic downstream scans

Rejected. They invent semantics, weaken locality, and are unsafe for navigation and refactoring.

### Maintain a separate editor-only parser stack

Rejected. It duplicates successful grammar behavior and splits the public meaning of a document.

### Number the replacement facts shape as version 2

Rejected. The facts API is still explicitly alpha, and the chosen pre-stable reset leaves one clean
v1 contract instead of carrying an obsolete schema generation into the stable version history.
Strict schema validation rejects the removed TextScan shape and all wire version values other than
`1`; consumers of the earlier alpha must update with the breaking package release.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0070: Diagnostics-First Analysis Contract
- ADR-0072: Lint Rule Governance
- ADR-0073: Family-Owned Diagram Architecture
