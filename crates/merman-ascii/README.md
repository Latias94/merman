# merman-ascii

[![Crates.io](https://img.shields.io/crates/v/merman-ascii.svg)](https://crates.io/crates/merman-ascii) [![Documentation](https://docs.rs/merman-ascii/badge.svg)](https://docs.rs/merman-ascii) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

`merman-ascii` is the terminal/text rendering crate for [merman](https://github.com/Latias94/merman). It renders Mermaid typed models as stable ASCII or Unicode text output for terminals, logs, documentation pipelines, and environments where SVG is not the right output format.

This crate consumes canonical and full data from `merman-model`. Its production dependencies do not include Mermaid source parsing. Applications can construct models directly or obtain them from `merman-core`.

The host supplies width, charset, color mode, palette, graph presentation, and overflow policy.
This crate does not probe terminal state or own a pager, cache, viewer, clipboard, or source-display policy. The
[host integration recipes](../merman/examples/README.md#host-integration-recipes) show facade requests
for terminal text and plain agent/log output without adding a host-policy abstraction.

Display-cell width differs from byte length and Unicode character count. Keep the same width
profile through wrapping, layout, extent reporting, and encoding. Color supplements glyphs,
spacing, and semantic disclosure; it must not be the only carrier of a relationship or status.

Terminal geometry uses `TerminalWidthProfile::Unicode` by default. Select
`TerminalWidthProfile::Cjk` when the target terminal renders East Asian ambiguous characters as
wide. Measurement, wrapping, truncation, placement, and output use the selected profile together;
the character set only selects structural glyphs and does not change authored-text width policy.
Because Unicode box-drawing and marker glyphs are East Asian Ambiguous, the CJK profile uses
single-cell ASCII structure even when `AsciiCharset::Unicode` was requested. Authored Unicode text
is preserved. This deterministic fallback prevents borders and routes from occupying two grid
cells per structural token.

Browser previews must also preserve terminal cell geometry. A CSS `monospace` family can use a
fallback font whose CJK glyphs are narrower than two Latin cells, shifting every border after a
Chinese label even when the text grid is correct. Use a compatible terminal font or normalize
font advances (for example, `font-size-adjust: ch-width 0.5` in supporting browsers), disable
ligatures, and verify mixed-script positions in the rendered preview. Line height must also match
the structural font when continuous vertical strokes matter. Selecting the CJK width profile
does not correct a browser font's pixel advances.

`merman-ascii` has an empty default feature set. Select the positive `diagram-*` features for
required families, or `all-diagrams` for the complete data and terminal-adapter surface.
Each selector enables model data and this crate's local handler when one exists. It does not enable
a source parser or system adapters. Model availability alone does not promise a terminal projection.

> **Implementation crate:** applications that ingest source can select the `ascii` feature on the [`merman`](https://crates.io/crates/merman) facade. Direct model consumers pass a `merman_model::ParsedSemanticDiagram` to `AsciiRenderer::render_parsed` or `render_parsed_report`, or supply a borrowed `AsciiDiagram` with `AsciiSourceContext`. A full `merman_model::presentation::RenderSemanticModel` requires explicit projection into the borrowed inputs.

This model renderer does not own a runtime-policy constructor, so it does not forward `system-*`
features. Direct users construct `AsciiRenderer`, capture an `OperationContext` from their facade or
host runtime policy, and pass that context together with the caller-owned `OperationControl` and
`AsciiResourcePolicy` to `AsciiRenderer::render_model`. Deterministic and sandboxed applications
should provide explicit operation values instead of enabling system adapters.

This guide covers `merman-ascii 0.11.0`. When upgrading from `0.9.x`, migrate core model imports to `merman-model` using the [model/source guide](../../docs/migrations/model-source-boundaries.md). See the [host guide](../../docs/release/PREBUILT_PRODUCTS.md) for available versions and installation channels.

## Quick Start

Most applications should use the `merman` facade so parsing and text rendering stay in one operation. The facade owns source parsing, operation control, and target selection; this crate exposes only the typed-model backend seam.

```toml
[dependencies]
merman = { version = "0.11.0", default-features = false, features = ["all-diagrams", "ascii"] }
```

For a current-source checkout beside the application, select the required families explicitly:

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = ["ascii", "diagram-flowchart", "diagram-gantt"] }
```

Depend on `merman-ascii` directly when the application owns a `merman_model::ParsedSemanticDiagram` for `AsciiRenderer::render_parsed` or `render_parsed_report`, or supplies a borrowed `AsciiDiagram` with `AsciiSourceContext`. Explicitly project a full `merman_model::presentation::RenderSemanticModel` into those borrowed inputs before rendering.

With a current-source `merman` executable on PATH, this PowerShell command requests adaptive Plain
output at 80 columns and returns the canonical report JSON, including the complete `text`:

```powershell
echo 'flowchart LR; A --> B & C; B --> C & D; D --> C' | merman render - --format unicode --ascii-color plain --ascii-graph-presentation adaptive --ascii-layout-profile auto --ascii-max-width 80 --ascii-overflow fallback --ascii-report --output -
```

Adaptive keeps a valid fitting diagram. It permits a complete graph document after an admitted
planning miss; the independent width policy controls whether a wide candidate is allowed, rejected,
or reflowed. See [Graph Presentation Boundary](#graph-presentation-boundary).

## Supported Families

ASCII support describes the quality of the terminal projection, not whether Merman can parse the Mermaid family. Query the runtime capability records when an application needs to populate an output picker.

| Semantic coverage | Primary projection | Families | Output contract |
| --- | --- | --- | --- |
| Partial | Diagrammatic | Flowchart, Swimlane, Sequence, State, Class, ER, Gantt, GitGraph, Packet, Pie, XYChart, Usecase, Block, C4, Architecture, Agentflow, Requirement, ZenUML | Core semantics render with documented limits; every admitted family can opt into one complete typed structured fallback for a bounded viewport. |
| Partial | Structured text | Journey, Kanban, Mindmap, Timeline, TreeView | Ordered, readable reports or outlines instead of browser-oriented chart geometry; the same projection can be reflowed under an explicit viewport fallback policy. |

Agentflow uses role-labelled spatial nodes and expanded flow frames for its admitted subset.
Rich metadata, collapsed views, and ambiguous containment use complete keyed reading records as
an explicitly reported StructuredText primary. Sequence, headless reference, and failure relationships
remain distinct. See [Agentflow support](../../docs/rendering/ascii/AGENTFLOW_SUPPORT.md) for the semantic boundary.

Requirement combines a shared-endpoint traceability graph with complete keyed node and relation
records. Contains marks the source; other relation kinds retain dotted target arrows. Ambiguous
names and unresolved references use explicit StructuredText primaries. See
[Requirement support](../../docs/rendering/ascii/REQUIREMENT_SUPPORT.md) for the field and presentation boundary.

C4 renders all five views with declaration-order rows, role-labelled rectangles, nested frames,
and short relationship keys linked to complete records. The overview shows shape roles/names and
boundary keys/names; technology, environment type, descriptions, and browser styles remain in
records. Invalid detached models use honest StructuredText primaries. Canonical and Strict are
the admitted profiles; routing failures remain errors, and width fallback is Plain-only. See
[C4 support](../../docs/rendering/ascii/C4_SUPPORT.md) for identity, arrows, and resource boundaries.

Architecture borrows its canonical services, junctions, groups, edges, and layout hints directly.
Spatial output preserves declared endpoint sides, arrow ownership, service-anchored group
contacts, and Row/Column member order. Invalid or conflicting detached geometry uses complete
StructuredText primaries. Canonical and Strict are admitted; routing and operation failures remain
errors, and width fallback is Plain-only. See
[Architecture support](../../docs/rendering/ascii/ARCHITECTURE_SUPPORT.md) for the semantic boundary.

Pie presents proportions as independent bars on one shared 0–100% scale, with complete keyed
labels and exact values. Tiny positive categories retain an explicit marker; undefined
distributions use StructuredText primaries. See [Pie support](../../docs/rendering/ascii/PIE_SUPPORT.md)
for the rounding, numeric, and presentation boundary.

Gantt uses one continuous elapsed-time axis over resolved task facts, with visible-end bars,
scheduled-midpoint milestones, and independent done/active/critical badges. Source-index T keys
link rows to complete UTC dates, raw endpoints, literal typed dependencies, and metadata records.
The timeline uses a 60–120-cell envelope (naturally 80) with a 26-cell key/status/label column;
records reflow to the viewport. Non-spatial domains, including compact/unknown display modes,
have StructuredText primaries. Canonical/Strict and Plain-only complete width fallback retain
existing limits and errors. See [Gantt support](../../docs/rendering/ascii/GANTT_SUPPORT.md) for
admission, exact midpoint rounding, and the SVG styling/tick and dependency-arrow limits.

Mindmap puts labels and hierarchy first. Distinct authored IDs, shapes, icons, and custom sections
appear in a separate `Details:` block, referenced from the corresponding node. The standard
branch palette is already expressed by the tree and is omitted when the complete graph follows
that rule. Labels that could forge an annotation or lose whitespace are quoted and escaped.

```text
root
|-- left
|   \-- leaf
\-- right
```

GitGraph has fixed branch tracks in LR/TB/BT with child-pointing arrows and explicit independent
crossings. Source-index C and B keys link the overview to complete commit, branch, and metadata
records. Cherry-picks disclose their picked source separately from ancestry. Parallel commits,
ambiguous/non-tip histories, and unsafe overlaps retain StructuredText primaries; Plain width
fallback uses the same complete records. See
[GitGraph support](../../docs/rendering/ascii/GITGRAPH_SUPPORT.md) for admission and tradeoffs.

Every selected logical model family has one capability record, including unsupported terminal
projections. These records describe this crate's local adapters even when another dependency selects
additional model data. The source facade intersects these capabilities with its actual parsers. Flowchart and Swimlane have
separate rows and share their terminal implementation. `semantic_coverage`,
`primary_projection`, and `structured_text_fallback` are independent; legacy `support_level` is
derived from them. The same record exposes `layout_profiles`, `graph_presentations`, `width_profiles`,
`encodings`, and `fallback_encodings`, so a host can reject an unsupported combination before rendering. Compact and Auto
layout selection are currently admitted only for Flowchart, Swimlane, and Sequence; every other supported family reports
canonical layout only. All supported families admit Plain, ANSI16, ANSI256, TrueColor, and HTML
primary output with Strict graph presentation, while structured viewport fallback is Plain-only.
`AsciiGraphPresentation::Strict` is the default. Adaptive is Plain-only and admitted only for
Agentflow, Flowchart, Requirement, Swimlane, and State; other supported families expose Strict only. Styled Adaptive requests
are rejected before routing, even if the input would fit. Other Mermaid families return
`AsciiError::UnsupportedDiagram` through the typed model path. The tracked [ASCII/Unicode support matrix](https://github.com/Latias94/merman/blob/main/docs/rendering/ASCII_SUPPORT_MATRIX.md) is the user-facing source of truth for exact limits. The [ASCII documentation index](../../docs/rendering/ascii/README.md) links family support notes, design comparisons, and maintained evidence.

Packet uses two cells per bit for integer `packet.bitsPerRow` in `1..=64`, with full field records
after the grid. Source rendering honors `bitsPerRow`, `bitOrder`, and `showBits`; detached models
use 32 bits per row with ascending annotations and visible bit numbers. Labels never widen a field:
`*` points to the exact record when a label or bit annotation cannot fit. Non-spatial numeric or
overlapping models retain complete records and report `StructuredText` for that result.

Swimlane reuses the Flowchart model kind. Use `render_parsed` or `render_parsed_report` to retain
the parser-owned logical language identity, or provide the matching `AsciiSourceContext` when
rendering a borrowed model. Report `family` preserves that source identity, such as `swimlane`,
including through viewport fallback. Shared internal graph diagnostics can still name `flowchart`.
Enabling one graph language does not enable the other; query `ascii_capabilities()` for logical
language availability.

The canonical report API is `AsciiRenderer::render_model_report` or
`AsciiRenderer::render_parsed_report`. It returns the text together with optional display-cell
`primary_extent`, measured `emitted_extent`, projection, width profile, layout profile, requested bound,
overflow outcome, fallback capability/attempt/reason state, trim state, and lossiness. The convenience
`render_model`/`render_parsed` helpers project only the text. `OverflowPolicy::Fallback` never
clips or ellipsizes authored values; it either returns a complete typed compatibility projection or
`AsciiError::FallbackUnavailable`.

`AsciiOutput::metadata()` is the canonical schema-4 transport payload for binding operation plans;
`AsciiOutput::report()` adds the exact text projection for CLI report JSON. Schema 4 records
`requested_graph_presentation`, the text `encoding`, requested/effective layout profiles, and whether
Compact was attempted. `projection` identifies the effective presentation. When planning yields no
valid primary scene, `primary_width`, `primary_height`, `layout_profile`, and `primary_overflowed`
are all explicitly null; Rust exposes `primary_extent: None`. A real empty primary remains measured
as `0×0`, with a non-null layout profile and `primary_overflowed: false`. `emitted_overflowed` always
compares the actual emitted width with the requested bound, so `outcome: fallback` alone does not
mean width overflow. This replaces schema 3's `overflowed` field; Options JSON remains version 3 and
outer binding operation metadata remains version 1. The [migration guide](../../docs/migrations/render-reuse-host-integration.md#ascii-terminal-graph-planning)
describes the source and report changes. CLI report mode is a machine channel: `auto` resolves to Plain there and
an explicitly styled report request is rejected. Styled Rust and binding results remain supported
and carry their exact encoding identity. Both projections are derived from the same measured
candidate, so CLI, Rust, and bindings do not remeasure text or reconstruct enum and field names
independently. Extent height counts logical content rows; a final line terminator does not create an
extra row. The measured seam accounts for display cells, encoded bytes, grapheme limits, and the
render-wide layout ledger once per emitted candidate. A Plain structured candidate is checked
incrementally in a detached candidate scope and admitted to the render-wide ledger only after it is
complete and permitted by the requested width policy. Styled fallback requests are rejected during capability
preflight. Semantic fallback projection first applies typed-model complexity admission; Flowchart's
typed JSON projection is serialized through a bounded, cancellation-aware writer before flattening,
so a rejected candidate does not allocate an unbounded intermediate under a bounded resource
profile.

Flowchart node labels wrap before layout at a default width of 40 terminal display cells. Use
`AsciiRenderOptions::with_flowchart_node_label_wrap_width` to tune that family-owned terminal
policy. The value is not a CSS-pixel conversion of Mermaid's SVG `wrappingWidth`; the same
grapheme-safe plan drives node measurement and final text materialization. The opt-in Compact
profile resolves the Flowchart node-wrap default to 24 cells and horizontal rank gap from 5 to 3,
and Sequence participant spacing from 5 to 3,
unless the caller explicitly overrides the corresponding family option. These adjustments are
resolved into family-local policies and do not affect State, Class, ER, XYChart, or structured-text
families.

Flowchart and Swimlane also consume the model's sparse sanitized label sources. Text entities
are decoded once after authored line breaks are resolved, preserving literal text such as
`returns &lt;T&gt;` and preventing `&lt;br&gt;` from acquiring newline semantics. Markdown emphasis
and strong markers do not occupy terminal cells; plain-label punctuation, code, and link notation
remain authored text. The same projection feeds node sizing, edge routing, and group titles.
Complete source spellings remain available in bounded Plain fallback records. Compatibility JSON
keeps its pinned Mermaid projection; its decoded label strings are not a replacement for these
source facts.

Graph routing compares deterministic candidates using target-port congestion, marked-source port
congestion, and combined route length, shared-cell cost and scene growth. Compound Flowchart routes
with a known common-scope direction prefer forward departure/arrival contacts after endpoint
congestion, while retaining alternate sides for obstructed routes. Open starts can retain shared outgoing stems;
visible source markers prefer independent contacts. Clear direct routes stay direct. Ordinary
candidates retain all cardinal port combinations instead of discarding mixed horizontal/vertical
choices before scene admission.
Actual endpoint markers and primary terminal positions remain protected. Unused alternate marker
positions admit transverse route crossings; later marker allocation rejects any position occupied
by such a crossing. This keeps alternate choices from sealing an entire branch corridor, including
covered two-ended-marker fans through fourteen edges at zero preferred graph padding.
When marked endpoint demand exceeds the shape-valid contacts reachable at the top or left canvas
boundary, layout reserves an exterior reading margin and a turn cell. It translates the complete
scene and coarse-grid origin together, preserving node dimensions, ranks, and group containment.
Endpoints with multiple markers also account for the escape corridors of their unmarked ends.
Different branches at one endpoint cannot stack marker heads along their pointing axis; parallel
edges with the same directed node pair retain their independently owned contiguous marker chain.
Colliding or congested node-to-node edges can additionally use terminal-cell search over every
non-corner contact accepted by the endpoint shape, including across nested compound frames. This
separates branch/merge heads and skip-edge lanes without changing Dagre ranks. Coarse-grid and
terminal-cell searches share frame constraints: routes stay inside common containers, avoid
unrelated containers, and cross owned frame sides transversely without turning on a border.
Aligned edges obstructed by another node reserve a bypass lane in their nearest common container;
group titles retain a horizontal reading gap where space permits. A missing primary route family
does not prevent alternate candidates or bounded recovery from running. Parent local-direction
layouts reserve the full span of child contents and move each child as a unit before frame spacing
is resolved. Optional improvement of an already valid route has a shared expansion cap across
port pairs. Pairs are generated in usage/distance order with a frontier linear in the source
perimeter rather than retaining their Cartesian product. Where a globally empty separator exists,
visible branches reserve a transit cell beyond both endpoint reading margins on the applied local
rank axis, including at zero preferred padding. Node content bands in other scopes stay intact.
Required collision recovery remains bounded by the scene extent and operation work
budget. Container endpoints also use terminal-cell recovery: external edges contact the outside
of the frame, while ancestor/member edges contact its inside. Only selected frame contacts permit
terminal crossings; corner contacts are excluded. Container self-loops can recover through two
distinct exterior contacts, keeping their transit and labels inside a shared parent when nested.
The immediate parent reserves room to leave a berth before turning and returning to the frame.
Center-out contour enumeration provides alternatives around titles and member nodes.
State concurrency separators remain open to transverse routes through intermediate regions;
their generated Divider identities retain their dedicated endpoint-routing boundary. Empty regions
remain ordered structural sections rather than visible nodes. Node self-loops can also recover
through alternate terminal ports when fixed lanes are blocked. Before such a search, earlier
loop labels are protected and the incumbent route is checked against those reservations again.
All attempts charge the same work ledger and observe cancellation.
Recovery rejects sealed straight approaches before opening the path frontier. Label candidates
retain first-insertion order with operation-local hash-set deduplication, avoiding a repeated
linear scan for each generated position as the contour search grows.
The search frontier stores indices into the scene's critical-coordinate axes. Each admitted
port-pair search resolves its starting coordinate once; adjacent-channel lookup then takes
constant work without a dense grid or an additional coordinate map. Collision checks and path
costs still inspect every traversed terminal cell, and only the winning path expands to cell
coordinates. The [resource audit](../../docs/rendering/ASCII_RESOURCE_AND_PANIC_AUDIT.md#indexed-channel-adjacency)
defines this adjacency-work bound separately from complete routing cost.

Opposite authored edges retain separate transit paths. Mixed normal/dotted/thick edges can cross
transversely. Unicode uses `╪` for an independent crossing and `┼` for a joined four-way channel;
ASCII and CJK structure use `+` for both. Both continuing axes remain visible. The crossing cell
is neutral to stroke weight; adjacent cells retain each route's solid, dotted, or thick stroke,
and the crossing keeps the horizontal route's color. Incompatible overlapping segments try
another lane. Committed routes reserve their label footprints before subsequent
edges are planned. Parallel node self-loops reserve nested lane space before node projection and
assign their labels as one route batch before unrelated edges or a searched loop are planned.
Searched-loop labels are committed immediately. Independent vertical stems and markers keep one
column of reading clearance from neighboring labels, preventing an ASCII `v` from merging with
label text or a caption from appearing attached to another relationship.
A graph with no safe route, label placement, or independent marker berth still returns a structured
diagnostic rather than a partial drawing. Boundary routes with no vertical transit segment
retain their labels on an available inline segment. Label candidates respect container ownership
and the route's planned marker berths: external group labels stay outside the endpoint frame, and internal labels stay
inside their common container. Coarse placement reserves a separator band and moves complete
sibling branches together. Axis projection uses a valid separating axis for measured frame
clearance, keeping external nodes outside their endpoint containers even at zero preferred padding.

Auto requires an explicit `AsciiViewportPolicy::with_max_width` and is admitted only for Flowchart,
Swimlane, and Sequence. It retains a fitting Canonical diagram; if Canonical is wide, it tries
Compact once and selects the narrower valid diagram, keeping Canonical on ties. Strict preserves
an initial planning error. Adaptive Auto for Flowchart and Swimlane can also try Compact once after
a Canonical planning miss, before selecting a graph document. Explicit Canonical and Compact never
switch profiles, and State remains Canonical-only. An optional Compact search/capacity miss retains
a valid Canonical candidate. Strict also retains Canonical on an optional Compact `UnsupportedFeature`;
Adaptive propagates that error. Resource, allocation, cancellation, and deadline errors propagate
under both policies, including with an incumbent. The existing `Allow`, `Error`, or
`Fallback` policy applies after selection. A narrower diagram may be taller; Auto does not change
direction or truncate labels. String-only renderer methods without a viewport reject Auto.

`AsciiOutput::layout_profile` identifies the selected primary geometry when one exists, while
`requested_layout_profile` and `compact_attempted` explain automatic selection. All attempts share
one work budget and cancellation control. Retained candidate text counts toward storage admission.
Use the spacing builders for explicit overrides, including a value equal to the default; direct
assignment of a default-valued public field cannot record that intent.

## Graph Presentation Boundary

Flowchart, Swimlane, and State use one private [scene planner](src/graph/planning.rs) to join measured
demand, layout generations, compatible channel allocation, routing, and final validation. Required
feasibility follows the finite channel states under the caller's public work/deadline policy.
Adaptive adds an early search-work stop to leave room for complete document recovery; replacement
and optional quality search have separate finite allowances. Quality work starts only after a
complete validated incumbent exists. Rejected route, port, marker,
and label reservations identify actual blockers for repair. A trial includes the affected routes'
shared-channel membership and replaces their contacts, paths, markers, and labels atomically. A
failed trial restores the incumbent without refunding spent work; a fatal error propagates.

Within each attempted layout profile, the planner permits at most two measured rebuilds
after the initial geometry: first for endpoint/corridor demand, then for scoped feedback-lane
demand. Only when neither capacity request grows may the final recovery phase select a cyclic
rank candidate. An unchanged request does not justify a rebuild. Each new generation rebuilds
coordinate-dependent allocations, contacts, protection, and search axes while retaining semantic
identities, authored directions, and group ownership. These private allowances bound a best-effort
search; they are neither new public resource limits nor a proof of complete or optimal routing.

After a routing miss, a flat Flowchart with visible edges and no authored side constraints may
use strongly connected components to generate another rank assignment. The condensation DAG
determines the main-axis levels, including cross-component relationship lengths; every member
remains a separate node in a transverse lane and every original relationship is routed normally.
This avoids stretching a chain of small cycles into one long column. It is an internal geometric
candidate within the same profile and operation budget, not a collapsed semantic summary or a new
layout option. Complete initial and capacity-recovered scenes retain their existing geometry.
Compound graphs and invisible rank constraints keep their existing planning rules, and arbitrary
cyclic graphs can still exhaust bounded search.

Static node, frame, and title protection is compiled into one spatial hierarchy per geometry
generation and shared by fixed-layout congestion trials. Cell, marker, and label queries prune
unrelated bounds before applying the same exact protection rules. The hierarchy retains linear
storage; overlapping bounds can still require visiting every protected region. Work accounting
includes index preparation and visited hierarchy nodes; the [resource audit](../../docs/rendering/ASCII_RESOURCE_AND_PANIC_AUDIT.md#static-graph-protection)
owns those structural bounds and historical measurements.
The [presentation corpus](tests/testdata/local-semantic/flowchart/presentation/README.md), generated
small-graph oracle, compound tests, and family tests define finite acceptance cases. Current-source
recovery checks live in [adaptive_graph.rs](tests/adaptive_graph.rs) and the fixed dense-input owner
[scene_planning_corpus.rs](tests/scene_planning_corpus.rs); these are separate from the historical
[reference comparison](../../docs/rendering/ascii/ASCII_REFERENCE_COMPARISON.md).
The public parsed-report matrix in [graph_family_viewports.rs](tests/graph_family_viewports.rs)
also checks State/Swimlane selection, logical identity, complete fields and measured extents across
60/80/100/120 columns in both character sets and width profiles.

| Request or result | Behavior |
| --- | --- |
| Valid diagram fits the viewport, or no width bound is supplied | Strict and Adaptive retain the complete diagram, preserving markers, conditions and actual node/group endpoints. |
| Selected valid diagram remains too wide | Apply Allow, Error, or Plain Fallback. Successful width fallback reports `structured_text`, `fallback`, `presentation_only`, and reason `primary_overflow`, retaining the measured primary extent. |
| State width request | Use Canonical and the same overflow policies; Compact and Auto are not admitted. |
| Required planning ends with `RoutingSearchExhausted` | Strict returns the error. Adaptive may emit a complete graph document with reason `routing_search_exhausted`; the miss does not prove that no layout exists. |
| Required planning ends with `PortCapacityExceeded` | Strict returns the error. Adaptive may emit a complete graph document with reason `port_capacity_exceeded`; the certificate applies only to the current fixed-terminal presentation. |
| Adaptive recovery document exceeds an explicit width | Allow emits the complete natural records with `emitted_overflowed: true`; Error returns `WidthOverflow`; Fallback reflows complete records or returns `FallbackUnavailable` if they cannot fit. Successful recovery retains its planning reason and `outcome: fallback`. |
| Parse/model error, unsupported semantics, resource/allocation failure, cancellation, or deadline | Return the corresponding error without partial output. Adaptive never catches arbitrary `UnsupportedFeature` errors. |

Graph presentation and width overflow are independent policies. Adaptive requires neither a width
bound nor `OverflowPolicy::Fallback`: without a bound, all three width policies accept the complete
natural recovery document. With a bound, Allow and Error measure its natural records, while Fallback
reflows them. Strict width fallback requires a valid primary candidate and cannot bypass a required
planning failure. All candidate, repair, rebuild, and document work shares the operation's cumulative
ledger, cancellation control, and deadline; choosing Adaptive does not raise resource ceilings.

Structured graph records preserve full labels, direct endpoints, relationship direction and
multiplicity, typed connectors, membership, and per-edge conditions within the family's existing
semantic contract. Flowchart uses first-use declarations and individually keyed relationships.
Records do not reproduce spatial direction and can be taller than a valid diagram. State retains
Canonical-only layout admission and full authored-detail disclosure, and the renderer has no
maximum-height selection policy. Hosts that
need a pager or alternate view own that presentation choice.

Graph detail records use word-aware wrapping with indented continuation rows. A word that fits
the viewport can use its full width when the usual two-cell indent would split it; longer atoms
still wrap at grapheme boundaries. Authored spaces, escapes and independently owned records
remain complete, including repeated State descriptions and relation records.
Complete semantic fallback records, including non-graph serialized fields, preserve data spaces
even under `TrimTrailingSpaces` and report `trimmed: false`; the requested trim policy still
applies to diagram canvas padding.

## Resource Policy

`AsciiResourcePolicy` owns six typed limits: checked grid extent, deterministic layout work,
aggregate logical document cells, actual encoded output bytes, bytes in one grapheme cluster, and
semantic nesting depth. Facade users set `AsciiRequest::resources`; direct typed-model users pass
the policy as the fourth argument to `AsciiRenderer::render_model`. Select a shared profile with
`AsciiResourcePolicy::for_profile`, or tighten one limit with
`AsciiResourcePolicy::with_limit(AsciiResourceLimitId, value)`. `UnboundedForTrustedInput` uses an
explicit unbounded value rather than a numeric sentinel; arithmetic overflow and allocation
failure remain fallible. `AsciiRenderOptions` contains presentation and family layout choices only,
so copying options cannot implicitly copy or replace an operation's resource budget.

Resource limits are hard errors. They never select a relation summary or another lower-fidelity
projection. Plain, ANSI16, ANSI256, TrueColor, and HTML use the same logical document accounting,
while `max_ascii_output_bytes` counts the actual bytes produced by each encoder.

Layout work counts deterministic text, geometry, and search operations, including conservative
batch bounds. It is not elapsed time, memory, token count, or a diagram-size limit. The shared
Interactive profile permits 2,000,000 units; the CLI's Trusted Native profile permits 8,000,000.
An operation deadline is an independent host control. See the
[CLI work/deadline guide](../../docs/security/CLI_RESOURCE_POLICY.md#ascii-planning-work-and-operation-deadlines)
for scoped budget overrides and trusted unbounded commands. The unbounded profile removes all
policy budgets, while private finite search allowances and hard guards remain.

## Terminal Visual Style

Merman's terminal diagrams use compact, readable structure before decoration. The graph renderer
keeps the author's node shapes, uses rounded `╭╮╰╯` turns for ordinary Unicode wires, and retains
square branch junctions and frame contacts. A rectangular node therefore remains `┌─┐`, while a
wire turning around it reads as a continuous path. Rounded nodes still mean the rounded shape
selected in the source; thick and dotted relationships retain their own strokes. ASCII and the
CJK structural fallback use their existing single-cell glyphs.

| Element | Visual rule | Reading purpose |
| --- | --- | --- |
| Node | Label-sized contour with the authored shape and explicit markers | Identify the operation or state without stretching unrelated nodes |
| Ordinary wire | Light stroke, rounded turns, square junctions, solid arrowheads | Follow a path and distinguish a bend from a branch |
| Group | Solid closed frame, separate complete title block, internal reading clearance | Show membership and the exact node or frame touched by an external edge |
| Label | Complete text associated with its own route and protected reading space | Recover the condition and endpoint pair without using color |
| Optional color | Host text, quieter structure, sparse heading/arrow emphasis | Reinforce the same hierarchy in ANSI or HTML without changing geometry |

Group titles center their complete block inside the frame and align its lines to one left edge.
This gives wrapped headings a stable reading start while retaining the transit space beside
single-line headings. RL output keeps that alignment; BT output keeps the existing lower title
band and upright line order. Routing protects the actual projected text and one reading cell on
each side, using the same title geometry as drawing. Minimum-width empty groups retain their
complete titles within the existing frame. Group borders remain solid so they are distinct from
dotted relationships and State concurrency separators.

For `A → B`, `A → C`, `B → C`, `B → D`, and `D → C`, the Unicode output keeps each direct
relationship visible in a 35-column, six-row drawing:

```text
┌───┐     ┌───┐     ┌───┐     ┌───┐
│ A ├────►│ B ├────►│ D ├────►│ C │
└──┬┘     └──┬┘     └───┘     └───┘
   │         │                 ▲▲
   │         ╰─────────────────╯│
   ╰────────────────────────────╯
```

The visual style is a renderer default, independent of Canonical/Compact layout and host color
encoding. It does not introduce a second layout backend or a separate theme preset registry.
Plain output must remain understandable on its own. Width-constrained structured output preserves
the same complete names and relationships instead of adding decoration that consumes the viewport.

## Terminal Theme API

`AsciiColorTheme::from_terminal_palette` derives terminal roles from a compact `AsciiTerminalPalette`: required `foreground` and `background`, plus optional `line`, `accent`, `muted`, `surface`, and `border` colors. The derived theme maps only terminal-meaningful roles such as text, borders, edge lines/arrows, sequence lifelines, and chart series colors. It does not import SVG CSS-variable semantics into text output. Explicit `AsciiColorTheme::with_role` calls still take precedence after derivation.

`AsciiColorMode::Ansi16` is the terminal-native profile. Unstyled primary text remains at terminal
Reset and sparse named ANSI accents mark renderer-owned structural roles, so the renderer does not
guess whether the terminal background is light or dark. ANSI16 does not resolve semantic roles
through the RGB theme; direct authored RGB colors are quantized separately. Use TrueColor when the
host's RGB palette should control terminal role colors, or ANSI256 for an approximation. ANSI16
changes encoding only: its stripped text and logical extent are identical to Plain for the same
layout request.

| Host intent | ASCII color mode | Palette behavior |
| --- | --- | --- |
| Plain logs or a machine text channel | `Plain` | No color escapes; viewport fallback is available |
| Use the terminal's own palette | `Ansi16` | Primary text and most structural roles use Reset; emphasis uses named ANSI colors |
| Apply a known host RGB palette | `TrueColor` | Semantic roles resolve through `AsciiColorTheme` |
| Approximate a host RGB palette | `Ansi256` | Resolved RGB colors are mapped to the 256-color encoding |

Named terminal colors and Reset do not guarantee contrast for an arbitrary user-customized
terminal palette. Styled output accepts viewport `Allow` or `Error`; `Fallback` is Plain-only
and an invalid styled/fallback combination is rejected even when the diagram would fit.

SVG compiled diagram themes and terminal `AsciiTerminalPalette` are independent inputs. The host
can map its own palette into both; selecting an SVG theme does not set terminal colors. SVG
typography, CSS-oriented family variables, and terminal role encoding remain separate. See the
[diagram theme guide](../../docs/rendering/custom-diagram-themes.md) and the executable
[terminal palette example](../merman/examples/terminal_palette.rs).

Bindings expose the same shape as `ascii.theme` in [Options JSON](../../docs/bindings/OPTIONS_JSON.md#ascii-options). Color values use the existing CSS color parser for opaque terminal colors; transparent colors are rejected rather than silently falling back.

## XYChart ASCII Contract

The XYChart renderer builds one typed `TerminalChartPlan` instead of rebuilding coordinates independently in each orientation. Every series consumes the model's `data` tuples as the source of truth for x/y coordinates; `values` is only a compatibility fallback for manually constructed legacy models that omit `data`. Parser-produced tuples derive their x coordinate from the typed band categories or from the linear axis domain and sample order, because Mermaid plot statements author an ordered value list rather than independent x/y pairs. Band axes resolve authored categories, linear axes map each model-owned numeric x coordinate through its explicit or inferred range, and negative, reversed, and degenerate value ranges remain deterministic. Data values use shortest-roundtrip formatting, while axis ticks use a separate scale-aware formatter so tiny ranges such as `0.001 --> 0.005` do not collapse into repeated labels.

By default, vertical charts use a five-row value area and three-character category bands. `AsciiRenderOptions::with_xychart_vertical_plot_height` and `AsciiRenderOptions::with_xychart_category_band_width` adjust that compact policy. Multiple bar series divide each category into stable lanes instead of overwriting one another. Line series use their model-owned x coordinates and share a stair-step topology layer that is painted after bars so mixed plots remain visible; missing samples split paths instead of creating false connecting segments.

Horizontal charts use a ten-character value axis by default; `AsciiRenderOptions::with_xychart_horizontal_plot_width` adjusts it. Multiple bar series receive independent rows per category, and horizontal line samples are connected rather than emitted as isolated points. Category labels come from the typed band axis when present; linear axes retain their typed numeric domain. The complete expanded plot extent is checked before row allocation.

Charts with more than one series render a compact legend row before the plot. When a Mermaid plot statement includes a user-authored series title, the legend uses that typed model title; otherwise it falls back to stable terminal labels such as `Bar 1` and `Line 1`. Line series cycle through six distinct point markers, matched by the legend in Plain and styled output. Their connections and sample ownership are combined before painting, so series order cannot erase an earlier point or connection. A terminal cell containing samples from multiple line series uses `X`; its exact owners and values remain in the disclosure. Shared line segments use the axis role rather than claiming one series owns them. Quantization and marker reuse beyond six line series still require the exact records for disambiguation.

The renderer consumes the typed XYChart display policy from `merman-core`. `xyChart.showTitle`, `xyChart.showDataLabel`, `xyChart.showDataLabelOutsideBar`, and `xyChart.xAxis/yAxis.showLabel/showTitle/showTick/showAxisLine` affect terminal output. Tick marks can render independently from axis lines. For a single bar series, data labels stay close to the bars and respect `showDataLabelOutsideBar`. For line and multi-series charts, `showDataLabel` emits injective `values:` rows with explicit series/sample indices, plot kind, title ownership, typed x coordinates, point labels, clipping state, and orphan labels. Every authored string is UTF-8 byte-length framed and quoted with terminal-safe escaping, so authored delimiters cannot forge renderer-owned fields. Multi-series identity, clipped samples, missing values, duplicate, normalized, alignment-changing, or over-wide categories, quantized point collisions, overlapping dense bars, same-row coordinate collisions, and insufficient grouped-bar lanes also trigger deterministic disclosure so semantic facts are not silently lost when terminal geometry is approximate.

Exact disclosure follows the plot. A borrowed record plan emits separate rows for series headers, samples, and orphan point labels; measurement and painting replay those same records. Adding samples therefore increases the record count instead of producing one ever-wider series row. Long individual authored fields and large chart geometry can still exceed a viewport and use the host's existing overflow policy. Record rows retain the `values:` prefix, byte framing, and complete authored values; no truncation or resource-error fallback is introduced.

The public typed-model path accepts an empty orientation as the legacy vertical default and accepts `vertical` or `horizontal` case-insensitively. Other orientation strings and a Band y-axis return a precise `UnsupportedFeature` error instead of silently changing meaning. `accTitle` and `accDescr` remain browser/accessibility metadata and are intentionally omitted from terminal output.

Header-only charts and typed plots with no terminal slots use an injective `xychart: empty` report instead of returning an empty document. The report preserves orientation, title, typed axes and categories, display policy, and any empty-series metadata through the same byte-length framing and resource limits as other StructuredText disclosures.

## Node padding and graph spacing

Flowchart, Swimlane, and State resolve node padding independently on each axis. Their defaults
prefer one cell of horizontal padding, no blank padding rows inside ordinary node frames, and
one row between graph ranks. Required endpoint and label demand can enlarge a node or gap.
Fixed State symbols retain their own dimensions. Independent contours in an ungrouped rank
share its center line, so a short state and taller side notes stay aligned without stretching
the state frame.

The existing `box_border_padding` field remains the scalar compatibility default. Explicit
`node_padding_x` and `node_padding_y` overrides take precedence, including zero. Bindings accept
both snake_case and camelCase spellings. Use `with_node_padding(1)` and
`with_graph_padding_y(5)` to select the previous State spacing explicitly; the builders preserve
these choices even when they equal the legacy public scalar defaults. Use
`with_node_padding(0)` to remove preferred padding on both axes, or combine
`with_node_padding_x` and `with_node_padding_y`. Graph rank gaps are separate preferences:
`with_graph_padding_x` and `with_graph_padding_y` preserve explicit values under Compact.
Required frame gutters, label clearance, and valid arrow berths can increase these gaps.
Assigning a legacy default directly to a public numeric field carries no explicit-override
provenance; use the corresponding builder when that value must override the family default.

Sequence spacing is measured from participant names and messages before rows are painted.
A message label occupies one interval between adjacent lifelines, including when its arrow
crosses intermediate participants. Explicit `wrap:` messages retain their wrapping policy.

Complete unwrapped labels can increase the required width and grid cell count. Resource ceilings
remain unchanged: large offline diagrams can use the bounded `TrustedNative` resource profile;
interactive callers can author explicit wrapping or select their own resource limits.

## Relation Summary Diagnostics

Class and ER share a directed topology plan: iterative strongly connected components produce a
condensation DAG, whose ranks preserve the direction of acyclic relationships. Cyclic components
unfold in a stable order without changing semantic endpoints. Bounded private channels separate
box contacts from exterior return paths and reserve label reading areas. This replaces the former
four-node topology exception; three-, four-, and five-node rings and DAG diamonds use the same
planning rules in all four directions. It remains a best-effort terminal layout rather than a
complete graph-routing algorithm.

Once the routes pass validation, the whole scene compacts redundant straight-channel rows and
columns. Boxes and text keep their dimensions; terminals and bends remain distinct; separated
features retain at least one cell of clearance. The scene is checked again before painting.
This applies to canonical Class/ER geometry and does not add Compact or Auto profile support.

Private Unicode route bends use rounded corners and the edge-line color role. A bend does not
claim a branch or crossing; ASCII and CJK structure keep single-cell `+` turns. Endpoint markers,
cardinalities, and labels retain their family-owned meanings.

Private-channel endpoint markers touch their class/entity frames. Cardinality text sits beside
vertical stems, with multiline values sharing one left edge, so text does not interrupt the
connection or separate its marker from the owner. The same label anchor is used for collision
checks, scene compaction, and drawing. Measured endpoint text filters unusable contacts before
path search, including wide multiline values beside horizontal chains. Unsafe endpoint text
uses the existing complete summary.

Class and ER diagrams fall back to readable `relations:` summary sections when bounded planning cannot produce a complete terminal grid, contacts do not fit, or route/overlay checks would damage another object. Simple relationships between sibling namespaces, a namespace and a root class, or nested sibling namespaces route through the nearest namespace facades; byte-length-framed fields disclose the authored member identity. Dense or colliding namespace scenes retain the complete summary. Default output hides the planning reason. Enable `AsciiRenderOptions::with_relation_summary_diagnostics(true)` to add `reason: route_collision` or `reason: overlay_collision` under `relations:`; contact and exhausted-channel failures use `route_collision`. Resource limits return `AsciiError::ResourceLimitExceeded` instead of selecting a summary.

When the complete primary result exceeds an explicit viewport with Plain `Fallback`, Class and
ER emit a family-owned reading document. Short keys identify classes/entities, namespaces/groups,
interfaces, notes, and individual relationships. Class endpoints retain independent marker types
and endpoint labels. Qualified class aliases identify their resolved class and preserve the
authored reference; interface references reuse the primary topology's identities. ER endpoints
use `1`, `0..1`, `1..*`, `0..*`, or `parent`, with an explicit
identifying/non-identifying stroke legend. Member/method and attribute records remain with their
owner, including keys and comments. Quoted fields wrap without truncating or rebinding identities.
Known empty/default properties, duplicate labels, native generated ER IDs, and Class's derived
topology cache are omitted. Nondefault styles, metadata, custom constant tables, source title, and
source configuration remain disclosed. These records reuse the admitted bounded serialization and
the same output/resource/cancellation rules as other viewport fallbacks.

## Direct Model API

```rust,no_run
use merman_ascii::{
    AsciiGraphPresentation, AsciiLayoutProfile, AsciiOutputOutcome,
    AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy,
    AsciiViewportPolicy, OverflowPolicy,
};
use merman_core::{OperationControl, ParseOptions, SemanticParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = SemanticParser::new();
    let control = OperationControl::new();
    let context = parser.begin_operation()?;
    let parsed = parser
        .parse_diagram_with_options_controlled_in_context_sync(
            "flowchart TD\nsubgraph one\nA((Start)) -- go --> B[(DB)]\nend",
            ParseOptions::strict(),
            &control,
            &context,
        )??
        .expect("diagram detected");

    let options = AsciiRenderOptions::unicode()
        .with_graph_presentation(AsciiGraphPresentation::Adaptive)
        .with_layout_profile(AsciiLayoutProfile::Auto);
    let renderer = AsciiRenderer::new(options)?;
    let resources = AsciiResourcePolicy::default();
    let report = renderer.render_parsed_report(
        &parsed,
        AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
        &control,
        &context,
        resources,
    )?;

    assert!(matches!(
        report.outcome,
        AsciiOutputOutcome::Primary
            | AsciiOutputOutcome::WideAllowed
            | AsciiOutputOutcome::Fallback
    ));
    println!("{} ({}×{})", report.text, report.emitted_extent.width, report.emitted_extent.height);
    Ok(())
}
```

Enable matching `diagram-*` selectors on both crates. Canonical terminal rendering needs neither
`presentation` nor `compat-json`; authored style precedence and complete fallback data come from
the same family model. Parsing and output above share one control and runtime context.

## Upstream Provenance

The ASCII renderer work is based on and informed by MIT-licensed reference implementations:

- [`AlexanderGrooff/mermaid-ascii`](https://github.com/AlexanderGrooff/mermaid-ascii)
  - Source commit used for the initial port plan and copied fixtures: `6fffb8e2714acab2c4cb41c78894fabbc62cee56`
  - Upstream license: MIT
  - License copy: `LICENSES/mermaid-ascii-MIT.txt`
  - Fixture source inventory: `tests/testdata/mermaid-ascii/README.md`
- [`lukilabs/beautiful-mermaid`](https://github.com/lukilabs/beautiful-mermaid)
  - Source commit used for reference planning: `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`
  - Upstream license: MIT
  - License copy: `LICENSES/beautiful-mermaid-MIT.txt`
  - Intended use: reference algorithms, output ideas, and tests for class, ER, xychart, color, and multiline ASCII work.
  - Promoted ideas are re-expressed as local semantic probes, including ampersand flowchart fan-in/fan-out, Class annotations and methods, ER attributes with identifying relationships, Sequence multi-message ordering, and XYChart multi-series value disclosure.

The local `repo-ref/` directory is gitignored and is only a research reference. Any derived source, fixtures, or notices required for builds and releases must live in tracked paths in this crate. `merman-ascii` remains model-driven: reference parsers are not copied into this crate.

## License

`merman-ascii` follows the workspace license: `MIT OR Apache-2.0`.

Ported algorithm work and copied fixtures derived from reference implementations preserve upstream MIT license notices in `LICENSES/`.
