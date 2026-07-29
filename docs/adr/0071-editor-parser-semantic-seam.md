# ADR-0071: Editor-Facing Parser and Semantic Seam

## Status

Accepted

## Dates

- Accepted: 2026-06-24
- Updated: 2026-07-28

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
- The Diagram Family catalog declares the semantic parser, closed combined semantic/editor
  construction, aliases, and authoring headers for every built-in id. The pinned catalog is
  complete rather than selected by Cargo features. Analysis does not maintain another family
  match or invoke a second family parser after failure.
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

Lexical facts are a separate family-owned projection over the same grammar evidence. Global
preprocessing owns fences, frontmatter, directives, and global comments; each family owns its body
keywords, operators, delimiters, identifiers, literals, comments, and family-specific values.
Lexemes are not inferred by LSP, Monaco, a Monarch grammar, or a raw-text fallback.

### Raw-text body semantics are removed

Analysis, editor-core, WASM editor queries, and LSP features use parser facts. Generic raw-text body
scans are not a fallback. If no parser-backed complete or recovered facts exist, the semantic index
is `Unavailable` and contains no body semantic items.

Source-start diagram headers and templates remain available from static Diagram Family catalog
facts. This is authoring metadata, not evidence that an unknown or malformed body was parsed.

Rich-facts projection errors are explicit internal analysis failures. If a typed family model
cannot be projected into its published fact shape, analysis reports that failure and omits only the
failed projection. It does not turn the error into indistinguishable absence or invoke a text scan.

One core parse snapshot owns preprocessing metadata, either the compatibility semantic model or the
original family error, and editor facts retained by that same construction. The known-type editor
facts API only projects this snapshot. Error suppression remains a compatibility behavior of the
JSON and render facades; it never rewrites a failed editor snapshot into a successful error family.

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
- current writers include `rename_policy` on every semantic item;
- parser-backed, recovered, and source-mapped-span flags retain their explicit meanings; and
- no TextScan-shape decoder, legacy executor, deprecated alias, or dual projection path remains.

The TextScan-capable prerelease shape is deleted. This decision deliberately resets the contract during
the coordinated refactor: consumers must regenerate against the current package, and every discriminator
other than version `1` is rejected before its body is decoded.

`AnalysisFactsPayload` is a binding wire projection, not the internal exchange format between
analysis, editor-core, and LSP. Those modules share typed `AnalysisResult`,
`merman_editor_core::DocumentAnalysisContext` / `DocumentSnapshot`, the LSP-owned
`DocumentAnalysisContext` / request-scoped `SnapshotContext`, and `FenceTextIndex` data without a
JSON round trip.

`AnalysisResult` and `AnalyzedDiagram` are sealed canonical outputs. Public callers may inspect
their source map, payload, ranges, syntax, and parser disposition through read-only accessors, but
only the analyzer and document-analysis pipeline can construct a generation or attach parser
evidence.

The facts schema version is unrelated to:

- LSP `textDocument.version`, which is a client document revision;
- Mermaid syntax and internal ids such as `flowchart-v2`, `stateDiagram-v2`, and
  `classDiagram-v2`; and
- C, WASM, UniFFI, or platform ABI/package versions.

### Ownership above analysis stays layered

- `merman-analysis::FenceTextIndex` is the shared semantic index and owns diagnostic/fact payload
  construction and source mapping.
- `merman-editor-core` owns protocol-neutral completion, hover, symbols, navigation, references,
  rename, selection, folding, code-action metadata, `DocumentAnalysisContext`, and the sole
  semantic-token planner.
- One generated token descriptor owns token codes, modifiers, precedence, LSP legend indices, and
  the five-word LSP-relative UTF-16 packed representation. Editor-core validates, sorts, resolves
  overlaps, splits multiline spans, converts UTF-16 positions, and packs the sequence once.
- `merman-lsp` owns its transport `DocumentAnalysisContext`, request-scoped `SnapshotContext`,
  request lifecycle, URI/range projection, full/delta result state, capability advertising, and
  stale-result suppression. It does not sort tokens, assign legend indices, or define language
  semantics.
- WASM validates the same descriptor and returns the same packed words. Monaco and VS Code consume
  that descriptor without a second enum, lookup table, sort, or regex grammar.

One cached LSP `DocumentAnalysisContext` pairs an editor-core `DocumentSnapshot` with its canonical
`AnalysisResult` and current diagnostic `AnalysisPayload` for a source version and analyzer
configuration. Each request borrows a `SnapshotContext`, which captures the document epoch and
snapshot generation together with the diagnostic generation. The request kind determines whether
currentness requires only the snapshot or both the snapshot and diagnostic payload. Completion,
hover/structure, rename, code actions, diagnostics, detection, and tokens therefore use one coherent
parse generation.

A rule-only analyzer change advances the diagnostic generation and reprojects `AnalysisPayload`
from the cached canonical `AnalysisResult` through `Analyzer::reproject_payload`; it retains the
same snapshot, parser evidence, and semantic-token state and does not invoke another parse.
Snapshot-affecting changes instead advance both generations, clear cached analysis contexts and
semantic-token state, and require a new analysis build. Site configuration, runtime policy
including fixed date/time, source limits, and source descriptors are snapshot-affecting because
they can alter parser facts, source availability, or source mapping.

## User-Visible Behavior

- Completion, hover, symbols, definition, references, rename, folding, and semantic tokens keep
  working for parser-complete and tested parser-recovered families.
- Syntax highlighting uses the same grammar facts and recovery identity in LSP, Playground Monaco,
  and the unpublished VS Code extension. An unavailable language Worker leaves the editor usable as
  plain text and exposes an explicit retry state; it does not install heuristic coloring.
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

### Reset the replacement facts shape as version 1

Adopted by amendment. The TextScan-capable shape and its decoder are deleted. The final parser-only
facts contract begins at version 1, and every other discriminator is rejected before nested fields
are decoded.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0070: Diagnostics-First Analysis Contract
- ADR-0072: Lint Rule Governance
- ADR-0073: Family-Owned Diagram Architecture
