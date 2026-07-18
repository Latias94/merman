# ADR-0073: Family-Owned Diagram Architecture

## Status

Accepted

## Date

2026-07-15

## Baseline

Mermaid `@11.16.0` (`7c0cafcf`)

## Context

Merman supports diagram families implemented with different parser technologies and different
upstream data models. That diversity is source-backed and should remain family-local. The previous
architecture nevertheless interpreted the same input through several independent paths:

- semantic JSON, typed rendering, and editor facts could run separate grammar loops;
- detector, parser, editor, render, metadata, and authoring-header registries repeated family facts;
- layout JSON and SVG could be produced through JSON-first or typed dispatch trees;
- render services and deterministic policy were selected at different pipeline stages;
- root SVG sizing and fixture-derived viewport pins were spread across family renderers; and
- parity commands rebuilt a parse-layout-render chain instead of exercising the operation used by
  public callers.

These paths made semantic drift possible even when their individual tests passed. They also made
invalid combinations, such as pairing one family's semantic model with another family's layout,
representable until runtime.

ADR-0014 remains authoritative: pinned upstream Mermaid behavior is the specification. This ADR
defines where that behavior is owned inside Merman.

## Decision

### Diagram families own meaning

Each built-in logical diagram family owns its successful semantic construction. That construction
owns grammar interpretation, parse-time mutation and ordering, validation, and source-backed spans.
Its outputs are projections, not independent interpretations:

- compatibility semantic JSON for parser and binding consumers;
- typed render semantics for layout and SVG;
- parser-backed editor facts, including recoverable facts for incomplete buffers; and
- family-specific warnings and expected-syntax facts.

Parser technology remains an implementation choice of the family. Jison-derived, Langium-derived,
LALRPOP, and hand-written parsers are not forced through a universal parser abstraction. Families
that import Mermaid's Langium `common.langium` share one span-rich implementation of its title and
accessibility syntax; grammars with different upstream rules retain their own implementation.

The built-in Diagram Family catalog is the single declaration of family ids, aliases, detector
order, tiny/full profile membership, semantic and editor adapters, typed render adapters, metadata,
configuration namespaces, and authoring headers. Public capability projections are derived from
that catalog. Custom parser registries remain explicit overlays and do not inherit built-in editor
or render capabilities.

The parse pipeline continues to own cross-family orchestration: preprocessing, detection, source
remapping, effective configuration, sanitization, timing, error suppression, and result ordering.
It does not own family grammar or family model construction.

### Editor behavior uses parser facts only

Body completion, hover, symbols, navigation, references, rename, folding, and semantic tokens use
parser-complete or parser-recovered facts. Generic raw-text body scanning is not a semantic
fallback. When parser facts are unavailable, the body semantic index is empty. Diagram headers and
templates may still be offered at legal source starts because they are static catalog facts and do
not claim to understand the document body.

`FenceTextIndexSource` records `ParserComplete`, `ParserCompleteDegradedSpans`,
`ParserRecovered`, `ParserRecoveredDegradedSpans`, or `Unavailable`. Degraded spans may support
identity and outline behavior, but precise edits must require `source_mapped_spans=true`.

The serialized diagnostics payload and richer facts payload are independent contracts with separate
version constants. The diagnostics-only `AnalysisPayload` remains version `1`. The parser-only
`AnalysisFactsPayload` is the sole version `1` facts contract: it has no `text_scan` provenance,
uses `unavailable` for absent body facts, and requires the family-owned `rename_policy` on semantic
items. The TextScan-capable alpha shape from `0.8.0-alpha.3` is removed rather than retained as a
compatibility path. Consumers of that alpha shape must update to the current schema even though its
numeric discriminator was also `1`.

This payload version is unrelated to LSP document revision numbers, Mermaid ids such as
`flowchart-v2` or `stateDiagram-v2`, and native binding ABI versions.

### Rendering is one typed operation

`merman::render::HeadlessRenderer` and the public prepared-render APIs execute the canonical
headless operation. Successful built-in rendering remains typed from family semantic construction
through layout and SVG.

At the low-level boundary, `FamilyRenderArtifact` owns the matching semantic model, layout, parse
metadata, and render session. The semantic/layout pair is opaque and cannot be independently
recombined. Compatibility layout JSON is projected from this artifact. SVG rendering consumes the
artifact, so a prepared result cannot be reused with a different family layout.

The built-in error diagram keeps its dedicated typed path. A custom semantic parser may return an
explicit custom JSON model, but it is non-renderable unless a future renderer capability is
designed and registered separately. Compatibility JSON is never the master SVG path for built-in
families.

The old public `layout_parsed*`, `render_layouted_svg`, raw semantic/layout SVG helpers, and
per-family pass-through SVG wrappers are not compatibility layers. They are removed. Applications
should use `HeadlessRenderer`, `layout_json_sync`, `render_svg_sync`, or the consuming
`prepare_render_sync` stages. Direct `merman-render` users use `family::prepare` and retain the
operation's `RenderSession`.

### The operation owns its render environment

`RenderEnvironment` selects immutable services and policy before an operation begins:

- text measurement routes for `Layout`, `Wrap`, `SvgBBox`, and `ComputedLength`;
- math rendering and icon lookup;
- clock and random-seed policy;
- and render resource limits.

`begin_session()` freezes those choices once and produces an opaque `RenderSession`. Family code
receives only the narrow session projection it needs. `SvgRenderOptions` contains request values,
while `SvgDebugOptions` contains diagnostics. Family renderers do not construct production
measurers or read process-global render policy.

The parity environment uses named vendored measurement and deterministic time/seed policy. Host
environments may supply host services explicitly. The operation report records the resolved routes,
measurement provenance, time, and seed. Successful host measurements bypass vendored fallback
facts for their routed operation.

### Root Viewport owns every root SVG

Every built-in family supplies content bounds and a source-backed root algorithm to the internal
Root Viewport module. That module owns:

- finite viewport normalization and max-width formatting;
- fixed, responsive, and Mermaid `useMaxWidth` sizing;
- root SVG attributes, accessibility chrome, escaping, and DOM-compatible attribute order; and
- deferred root finalization for families whose bounds are known only after SVG emission.

Family renderers use the typed `RootViewportContext`, `RootViewportSpec`, `RootViewportPlan`,
`RootChrome`, and opaque `RootDocument` protocol. They do not emit root attributes directly or look
up generated tables. Root viewports always come from family-computed or emitted-content bounds.
There is no generated-or-computed policy split and no mutable or request-local override path.

Browser-dependent root differences remain verification residuals under ADR-0062. They do not
authorize production fixture pins, model distortion, comparator broadening, or fixture-specific
tuning.

### Verification exercises the same operation

Parity comparison commands render through the canonical typed `HeadlessRenderer` operation and
report the actual render path and environment policy. The compare harness owns fixture locking,
upstream provenance, DOM modes, normalization, accepted residuals, and root coverage. Family hooks
may add narrow diagnostics, but they may not rebuild parsing, layout, or SVG rendering.

Compatibility JSON checks remain explicit projection tests. They supplement typed operation tests;
they are not the SVG oracle for built-in families.

Architecture guards and repository audits enforce ownership invariants rather than private naming.
They reject reintroduction of generic body scans, JSON-first built-in rendering, independent typed
semantic/layout pairing, hidden production measurers, direct family root override mutation, and
compare adapters that rebuild the operation.

## Consequences

- A family grammar or model change has one semantic owner and several explicit projections.
- LSP completion and refactoring remain available for parser-backed families. Unsupported or
  unknown body text now returns honest absence instead of plausible but invented symbols.
- Compatibility JSON remains supported, but callers must not use it as evidence that a separate
  render grammar exists.
- Invalid cross-family render pairings are unrepresentable on the canonical path.
- Host-specific rendering behavior is explicit, reproducible, and observable in operation reports.
- Root viewport algorithms can evolve without reopening every family renderer, while family-specific
  content-bounds algorithms remain local.
- Adding a family or alias requires one catalog declaration, family-owned semantic projections,
  a typed render adapter when renderable, parser-backed editor facts when admitted, and parity
  evidence through the canonical operation.
- Consumers of the superseded alpha facts shape must migrate directly to the current facts v1
  contract; no deprecated alias, legacy executor, or dual decoding path is kept.

## Rejected Alternatives

### Keep JSON and typed rendering as equal master paths

Rejected because it preserves duplicated dispatch and allows semantic/layout drift.

### Add a permanent compatibility or strangler layer

Rejected because the project is still alpha and the layer would make the transition architecture
permanent. Migration is documented instead.

### Introduce one parser framework for every family

Rejected because upstream Mermaid itself uses different grammar and DB strategies. Ownership and
projection convergence solve the architecture problem without erasing source-backed differences.

### Preserve a generic editor text scan for unsupported inputs

Rejected because guessed node ids and references are indistinguishable from parser facts to users.
Static source-start authoring does not require body scanning.

### Keep version-pinned root or complete-text tables

Rejected because a fixture-keyed answer is a second renderer path and cannot generalize to unseen
diagrams. Browser-only differences belong in verification evidence, while production consumes
computed bounds and general measurement facts.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0014: Upstream Parity Policy
- ADR-0050: Release Quality Gates
- ADR-0057: Headless SVG Text `getBBox()` Approximation
- ADR-0062: No Production Fixture Overrides
- ADR-0071: Editor-Facing Parser and Semantic Seam
