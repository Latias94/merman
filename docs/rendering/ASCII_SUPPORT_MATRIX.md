# ASCII/Unicode Support Matrix

This document describes the user-facing `merman-ascii` support boundary. It is intentionally
stricter than "the parser accepts this Mermaid family": ASCII output is terminal text, so browser-
only styling, rich geometry, and some SVG container semantics may be approximated, summarized, or
omitted.

## Capability Dimensions

| Field | Values | Meaning |
| --- | --- | --- |
| `semantic_coverage` | `full`, `partial`, or `null` | How much of the family's typed semantics the terminal output preserves. `null` means no output is available. |
| `primary_projection` | `diagrammatic`, `structured_text`, or `none` | Whether the primary output is box-and-line terminal geometry, a readable report/outline, or unavailable. |
| `structured_text_fallback` | Boolean | Whether the typed family has an admitted complete structured projection that can be selected when its primary result exceeds a bounded viewport. This applies to both diagrammatic and already-structured families. |
| `layout_profiles` / `layoutProfiles` | Array | Layout profiles admitted for this family. The Flowchart projection, including Swimlane, and Sequence expose `canonical`, `compact`, and `auto`; every other supported family currently exposes `canonical` only. |
| `graph_presentations` / `graphPresentations` | Array | Graph presentation policies admitted for this family. Agentflow, Flowchart, Requirement, Swimlane, and State expose `strict` and `adaptive`; other supported families expose `strict` only. Adaptive additionally requires Plain encoding. Unsupported families expose an empty array. |
| `width_profiles` / `widthProfiles` | Array | Terminal display-width conventions admitted for this family: `unicode` and `cjk` for every supported output. |
| `encodings` | Array | Primary result encodings admitted for this family: `plain`, `ansi16`, `ansi256`, `truecolor`, and `html` for every supported output. |
| `fallback_encodings` / `fallbackEncodings` | Array | Encodings admitted with viewport `fallback`. This is currently `plain` only; styled fallback requests are rejected before rendering. |

The legacy `support_level` / `supportLevel` field remains a derived compatibility view:
`none` maps to `unsupported`, `structured_text` maps to `summary`, and `diagrammatic` maps to its
`full` or `partial` semantic coverage. Structured text is supported output, but it is excluded from
diagrammatic-family counts.

## Runtime Capability Metadata

The same support story is exposed to bindings through the family capability records in
[`crates/merman-ascii/src/capability.rs`](../../crates/merman-ascii/src/capability.rs) and the
`ascii_capabilities_json` binding helper. The
legacy `ascii_supported_diagrams` list is derived from records with available output. Product counts
and diagram-only filters use the separate diagrammatic projection.

Binding migration: consumers must replace `summary_fallback` with `structured_text_fallback`, read
`semantic_coverage` and `primary_projection` as the source fields, treat `support_level` only as a
compatibility view, and preflight layout/graph-presentation/width/encoding combinations from the admission arrays
instead of assuming that a renderer-wide option is valid for every family. Package availability is
also independent: first check the compiled artifact's operations, then the family's admitted
combinations. The [host integration recipes](../../crates/merman/examples/README.md#host-integration-recipes)
show how hosts supply policy and consume results without terminal detection inside the renderer.

## Viewport and report contract

`AsciiOutput` is the canonical schema-4 result for a render request. `AsciiOutput::metadata()` is
used by operation metadata and platform bindings; `AsciiOutput::report()` is the CLI JSON projection
with the same metadata plus `text`. Both use the same field names, enum strings, display-cell
extents, and width/layout profiles. Schema 4 deliberately replaces schema 3's `overflowed` field
and permits unavailable primary geometry; the [migration guide](../migrations/render-reuse-host-integration.md#ascii-terminal-graph-planning)
owns the upgrade procedure. Options JSON remains version 3, outer binding operation metadata
remains version 1, and SVG and semantic JSON retain their existing schemas.

| Result fields | Schema-4 meaning |
| --- | --- |
| `primary_width`, `primary_height`, `layout_profile`, `primary_overflowed` | Required nullable group: all are non-null for a valid measured primary, including width fallback, or all are null when planning produced no valid primary scene. Rust represents the extent as `Option<AsciiExtent>`. |
| Real empty primary | Numeric `primary_width: 0`, `primary_height: 0`, its selected `layout_profile`, and `primary_overflowed: false`, with `outcome: empty`. Zero is not a substitute for unavailable geometry. |
| `emitted_width`, `emitted_height`, `emitted_overflowed` | Always measured from the complete emitted candidate. Overflow compares its width with the requested bound and is false without a bound. |
| `requested_layout_profile`, `compact_attempted` | Preserve the request and the actual Compact-attempt state even when `layout_profile` is null. |
| `requested_graph_presentation`, `projection` | Record requested `strict`/`adaptive` and effective `diagrammatic`/`structured_text` independently. |

`encoding` identifies the emitted text representation. Logical height counts content rows; a final
line terminator does not create an extra row. `primary_overflowed` describes the selected untrimmed
primary when one exists; `emitted_overflowed` describes the actual emitted width after permitted
trimming. Neither `outcome: fallback` nor `fallback_attempted: true` alone implies width overflow.
CLI report mode is Plain-only: host `auto` resolves to Plain and an explicit ANSI/HTML report request
is rejected. Rust and binding result APIs retain styled Strict output and its exact encoding.
Viewport Fallback and Adaptive graph presentation are both Plain-only; styled combinations are
rejected by capability preflight before routing, even when that particular input would fit.

`AsciiGraphPresentation::Strict` is the default. Adaptive permits complete graph-document recovery
only for Agentflow, Flowchart, Requirement, Swimlane, and State after successful semantic construction and an admitted
planning miss. It is independent of the width overflow policy:

| Planning result | Graph presentation | Width policy | Result |
| --- | --- | --- | --- |
| Valid primary fits, or no width bound | Strict or Adaptive | Any | Retain the complete primary. |
| Valid primary exceeds the bound | Strict or Adaptive | Allow / Error / Fallback | Emit the wide primary / return `WidthOverflow` / select a complete Plain width fallback with reason `primary_overflow`, or return `FallbackUnavailable`. |
| `RoutingSearchExhausted` or `PortCapacityExceeded` | Strict | Any | Return the typed planning error; width fallback cannot bypass it. |
| Same admitted planning miss | Adaptive | Allow | Emit complete natural graph records, even when wide. Report `outcome: fallback`, the planning reason, and truthful `emitted_overflowed`. |
| Same admitted planning miss | Adaptive | Error | Measure the natural graph document; reject it with `WidthOverflow` only if it exceeds an explicit bound. |
| Same admitted planning miss | Adaptive | Fallback | Reflow complete graph records to the explicit bound, or return `FallbackUnavailable` if they cannot fit. Without a bound, emit the natural records. |
| Parse/model error, unsupported semantics, resource/allocation failure, cancellation, or deadline | Any | Any | Preserve the error with no partial output; no recovery attempt swallows it. |

Successful graph recovery reports `structured_text`, `fallback`, and `presentation_only`, with
reason `routing_search_exhausted` or `port_capacity_exceeded`. Width reflow of that document retains
the planning reason. A routing-search miss is a bounded heuristic result; port capacity certifies
only the current fixed-terminal presentation's marker bound. Neither proves that no other layout
exists, and Adaptive does not catch arbitrary `UnsupportedFeature` errors.

Auto requires an explicit width and is available only for Flowchart, Swimlane, and Sequence. It
keeps a fitting Canonical primary; for a wide Canonical primary it tries Compact once, chooses the
narrower valid candidate, and keeps Canonical on ties. Strict preserves an initial planning error.
Adaptive Auto for Flowchart and Swimlane can additionally try Compact once after a Canonical
planning miss before graph-document recovery. Explicit Canonical/Compact never switches profiles;
State remains Canonical-only. An optional Compact search/capacity miss retains a valid Canonical
candidate. Strict also retains Canonical on an optional Compact `UnsupportedFeature`; Adaptive
propagates that error. Resource, allocation, cancellation, and deadline errors still propagate with
an incumbent. Narrower output can be taller; Auto does not choose a graph direction or truncate long labels.

The shared graph planner uses actual reservation blockers and atomic shared-channel repair, with
at most two measured rebuilds after the initial geometry in each attempted profile. Flat visible-edge
Flowcharts without authored side constraints can use component ranks after a routing miss when
existing endpoint, corridor, and return-lane feedback cannot grow: the condensation DAG sets levels,
while every original node and relationship remains independently rendered. This candidate shares
the existing work allowance and preserves valid initial and capacity-recovered scenes. Planning
remains best effort. The [graph presentation boundary](../../crates/merman-ascii/README.md#graph-presentation-boundary)
describes this private planner and the remaining density/height limits. Class/ER relation planning,
Sequence ordering, quantitative diagrams, and StructuredText families retain their family owners.

A measured candidate owns its metrics once; fallback construction checks width and output ceilings
incrementally, then commits one render-wide resource admission. Overflow remains a presentation
result (`allow`, `fallback`, or `error`), never a resource success or resource failure. Cancellation
and resource errors retain precedence over fallback selection. Semantic fallback applies typed-model
complexity preflight; Flowchart's typed projection uses a bounded cancellation-aware JSON writer
under bounded profiles. Flowchart, State, and Swimlane width fallback and adaptive recovery present keyed nodes/groups
and individual relationships with complete labels, markers, strokes, membership, and authored
details. Flowchart first-use declarations avoid a separate repeated node table. Class and ER
width fallback uses family-owned keyed declarations, members/attributes, and individual
relationships with independent markers or cardinalities. Known defaults and derived layout
caches are omitted; nondefault metadata and source configuration stay with their owner. Packet
and Gantt use the existing bounded record reflow for Plain width fallback. Other families retain
their existing fallback projections.

### Admitted profile combinations

| Family set | Layout profiles | Graph presentations | Width profiles | Primary encodings | Fallback encodings |
| --- | --- | --- | --- | --- | --- |
| Flowchart, Swimlane | `canonical`, `compact`, `auto` | `strict`, `adaptive` | `unicode`, `cjk` | `plain`, `ansi16`, `ansi256`, `truecolor`, `html` | `plain` |
| Sequence | `canonical`, `compact`, `auto` | `strict` | `unicode`, `cjk` | `plain`, `ansi16`, `ansi256`, `truecolor`, `html` | `plain` |
| Agentflow, Requirement, State | `canonical` | `strict`, `adaptive` | `unicode`, `cjk` | `plain`, `ansi16`, `ansi256`, `truecolor`, `html` | `plain` |
| Class, ER, Gantt, Packet, Pie, XYChart, Usecase, Block, C4, Architecture, ZenUML, structured-text families | `canonical` | `strict` | `unicode`, `cjk` | `plain`, `ansi16`, `ansi256`, `truecolor`, `html` | `plain` |
| Unsupported families | none | none | none | none | none |

The primary encoding list applies to Strict requests. Adaptive is Plain-only even with Allow or
Error; admitting both `adaptive` and a styled primary encoding separately does not admit their
combination.

## Diagrammatic Families

These families are counted as ASCII diagrams because the primary projection preserves a spatial
relationship: nodes/edges, participants/messages, states/transitions, entities/relations, time
intervals, plot coordinates, or bit positions. A family remains `Partial` until its common semantic
fields are both retained and readable at ordinary terminal widths.

| Mermaid family | Semantic coverage | Primary projection | Structured-text fallback | What renders well | Important limits |
| --- | --- | --- | --- | --- | --- |
| Flowchart / graph | Partial | Diagrammatic | Yes | Root directions, Dagre-compatible ranking, explicit pinned-shape dispositions, common diagrammatic node shapes, measured type compartments for admitted uncommon shapes, complete textual icon/image metadata, terminal-cell wrapped node labels, compatible branch/merge channels with shared heads and private conditions, independent mixed-marker terminals, complete shape-valid non-corner terminal ports, marked-source congestion repair, local-direction branch clearance, normal/dotted/thick/invisible edge semantics, transverse mixed-stroke crossings, committed label reservations, subgraphs, nested groups, first-parent compound ownership, boundary-aware terminal-cell routes between nodes and container endpoints, exterior container self-loops with distinct non-corner contacts, interior ancestor/member contacts, container-scoped labels, measured compound bypass clearance, scene-level route occupancy, and family-owned Compact geometry. | Icon glyphs/image pixels, callbacks, links, remaining rich object and cross-family shapes, arbitrary dense routing (including marked fan-out), and non-transverse mixed-stroke junctions remain unsupported or incomplete; bounded fallback is a typed Plain compatibility projection, not clipped geometry. |
| Swimlane | Partial | Diagrammatic | Yes | Uses the same terminal projection and admitted profiles as Flowchart while retaining its separate language and feature identity. | Shares Flowchart's geometry and presentation limits; terminal layout is independent of the SVG lane renderer. |
| Sequence | Partial | Diagrammatic | Yes | Mermaid-valid spaced/Unicode participant IDs, typed headless/filled/cross/point/bidirectional/half-arrow messages, central decorations, notes, lifecycles, boxes, participant-bounded nested control frames, and a family-owned compact participant-spacing policy. | Actor presentation metadata and links are accepted but omitted from the primary scene; bounded fallback discloses typed fields as Plain structured text. |
| ZenUML | Partial | Diagrammatic | Yes | Family-owned lifelines over the borrowed ZenUML model retain declaration-ordered P participants, depth-first S statements, synchronous/asynchronous/return distinctions, self paths, creation points, nested calls, every control frame and section, group heading bands, references, and dividers. Complete appended records retain original fields, metadata, spans, and source configuration through iterative flat serialization. | Canonical and Strict only; both width profiles and standard primary encodings. Missing or ambiguous detached identities, invalid groups, and invalid lifecycles use complete StructuredText primaries. Resource/allocation failures, cancellation, and deadlines remain errors under unchanged budgets. Plain-only complete width fallback; no universal dense-scene or spatial extent guarantee. See [ZenUML support](ascii/ZENUML_SUPPORT.md). |
| State | Partial | Diagrammatic | Yes | States, transitions, notes, graph-like pseudostates, groups, terminal colors, structural empty concurrency regions, and transverse routes through intermediate region separators. | State is canonical-only and does not inherit Flowchart compact policy. Direct typed-model Divider endpoints retain dedicated routing; some presentation metadata and future shape variants are approximated. Bounded fallback is a typed Plain compatibility projection. |
| Agentflow | Partial | Diagrammatic | Yes | Family-owned task/tool/input/refdoc/action/decision/connector roles, sequence arrows, headless dotted references, failure cross markers, expanded flow frames and global ownership. Rich metadata, collapsed views and ambiguous containment have complete keyed StructuredText primaries. | Canonical only; Adaptive is Plain-only. Reading records disclose authored contents and original endpoints, including hidden children, rather than claiming collapsed browser geometry. See [Agentflow support](ascii/AGENTFLOW_SUPPORT.md). |
| Requirement | Partial | Diagrammatic | Yes | One global keyed graph retains shared endpoints and all seven typed relationships. Contains has a source circle and no target arrow; other kinds have dotted target arrows. Complete records retain requirement type/id/text/risk/verification, element type/docRef, styles, accessibility, and source metadata. | Canonical only; Adaptive is Plain-only. Ambiguous names, unresolved references, unknown kinds/directions, and empty models use complete StructuredText primaries. The source circle approximates Mermaid's circled plus. See [Requirement support](ascii/REQUIREMENT_SUPPORT.md). |
| Usecase | Partial | Diagrammatic | Yes | Family-owned actor/use-case roles, boundaries/packages, membership, notes, ordered JSON tables with indexed paths and literal typed values, four directions, association/include/extend/generalization identity, independent endpoint markers, and dotted relationships through the shared graph planner. | Canonical and Strict only. Actor/ellipse rendering is a terminal approximation; JSON tables use terminal columns and explicit indexed paths. Required routing failures remain diagnostics; complete keyed Plain records serve width fallback. See [Usecase support](ascii/USECASE_SUPPORT.md). |
| Block | Partial | Diagrammatic | Yes | Family-owned grid packing preserves declared columns, spans, spacers, row order and nested frames; shared occupancy routing preserves edge labels, strokes and independent markers. | Canonical and Strict only. Non-spatial numeric settings and unsupported shape variants have explicit diagnostics. Dense fixed-grid routing remains bounded; complete hierarchy and edge records serve Plain width fallback. See [Block support](ascii/BLOCK_SUPPORT.md). |
| C4 | Partial | Diagrammatic | Yes | Context, Container, Component, Dynamic, and Deployment views use declaration-order rows, role-labelled rectangles, nested frames, shape-first alias lookup, boundary endpoints, and independent arrowheads. Short E keys and final-order Dynamic labels link to complete authored fields and metadata records. | Canonical and Strict only. Overview headings show roles/names and boundary keys/names; technology, environment type, descriptions, and browser styles remain records. Hidden global boundaries retain B records. Non-spatial or invalid detached models use honest StructuredText primaries; routing/resource/cancellation failures remain errors. Plain-only width fallback; suffix directions do not force cardinal ports. See [C4 support](ascii/C4_SUPPORT.md). |
| Architecture | Partial | Diagrammatic | Yes | Family-owned services, junctions, nested groups, independent declared endpoint sides and arrowheads, service-anchored group contacts, ordered Row/Column constraints, and complete keyed records. Invalid or conflicting detached geometry uses StructuredText primaries. | Canonical and Strict only; both width profiles and standard primary encodings. Strict routing, resource, cancellation, and deadline failures remain errors. Plain-only width fallback; no universal dense-scene or spatial extent guarantee. See [Architecture support](ascii/ARCHITECTURE_SUPPORT.md). |
| Class | Partial | Diagrammatic | Yes | Class structure, notes, namespaces, four directions, independent source/target relation markers, directed component ranks, private channels for bounded cycles and DAG diamonds, simple sibling-namespace / namespace-to-root / nested-sibling facade routing with length-framed leaf identity, and explicit relation summaries. | Bounded channel search does not guarantee arbitrary dense routing. Dense or colliding namespace-crossing scenes, port-incompatible lanes, and other collision-prone relationships can use lossless `relations:` output. |
| ER | Partial | Diagrammatic | Yes | Entities, attributes, key tokens, attribute comments, four directions, relationship labels/cardinalities including the parent diamond, nested group frames and typed group endpoints, actual member identity on facade routes, directed component ranks, private channels for bounded cycles and DAG diamonds, and explicit relation summaries. | Ancestor/member edges use typed summaries alongside group geometry; unspecified group directions use TB. Duplicate groups, ambiguous ownership/route IDs, and hidden shadow-entity endpoints remain unsupported. Port-incompatible and dense/collision-prone topology can use lossless `relations:` output; bounded search is best effort. Accessibility, Mermaid diagram source comments, and styling metadata are intentionally omitted. |
| Gantt | Partial | Diagrammatic | Yes | One shared elapsed-time axis uses global minimum start/maximum scheduled end, visible-end bars, scheduled-midpoint milestones, independent status badges, source-index T keys, and repeated section occurrences. Complete appended records retain exact UTC dates, raw endpoints, literal typed dependency rules, flags, classes, raw fields, and source metadata. | Canonical and Strict only. The 60–120-cell envelope has a natural width of 80 and a 26-cell key/status/label column; records reflow to the viewport. Non-spatial domains, including compact/unknown display modes, use StructuredText primaries. Plain width fallback retains the same keyed records. No second scheduler, excluded-day compression, current-clock today marker, dependency arrows, or SVG styling/tick parity. Identity/section errors and operation failures remain errors. See [Gantt support](ascii/GANTT_SUPPORT.md). |
| Packet | Partial | Diagrammatic | Yes | Fixed two-cell bit positions and widths, gaps, physical row splitting, ascending/descending bit order, optional bit numbers, and complete authored labels plus raw numeric DB records after the plot. | Spatial admission requires integer `bitsPerRow` in `1..=64`. Labels never widen fields; narrow labels may use `*` with a legend and exact details. Non-spatial numeric values, overlaps, or out-of-order fields retain complete StructuredText records. Canonical-only; Plain width fallback uses bounded record reflow. |
| Pie | Partial | Diagrammatic | Yes | Explicitly named Pie proportions view: independent 50-cell bars share a 0–100% scale, preserving canonical section order, all labels, exact values, `showData`, title, accessibility, and source metadata in complete keyed records. Tiny positive values retain a distinct marker and their denominator contribution. | Circular/donut geometry, legend placement, and browser interaction are not reproduced. Empty, zero-total, negative, and non-finite borrowed models have complete StructuredText primaries with an explicit reason. Canonical and Strict only; width fallback uses complete Plain records. See [Pie support](ascii/PIE_SUPPORT.md). |
| XYChart | Partial | Diagrammatic | Yes | Model-owned x/y samples and point labels, band/linear axes, negative/reversed/degenerate ranges, grouped bars, jointly composed line topology, distinct line-series point markers, explicit shared sample cells, mixed series, titles, legends, display policy, injective record-oriented disclosure after the plot, empty-chart metadata reports, and horizontal/vertical variants. Parser-produced x coordinates derive from the typed axis/category domain and sample order. | Browser hover is replaced by terminal disclosure; coordinates are quantized, line markers repeat after six series, and dense bar/line overlap still needs exact records. Long individual fields and large geometry can exceed the viewport. Unknown direct-model orientations and band y-axes are rejected; accessibility title/description metadata is intentionally omitted. |

### Packet bit grid

Packet's bounded spatial projection uses exactly two terminal cells per bit. Field geometry follows
the inclusive `start..end` range, preserves gaps, and splits fields across physical rows. Label
length never changes the scale. Descending bit order mirrors the fields and gaps within each row
and swaps each field fragment's endpoint annotations, including when bit numbers are hidden.
A label that cannot fit may use `*`; an explicit legend and complete
byte-framed authored label records disambiguate it. Raw `start`, `end`, and DB `bits` values remain
in the records after the plot: upstream row splitting can produce `bits` values different from
`end - start + 1`, and the terminal renderer does not rewrite them.

Source rendering and source-backed reports honor `packet.bitsPerRow`, `packet.bitOrder`, and
`packet.showBits`. Bare borrowed `render_model` has no source configuration and uses 32 bits per
row, ascending order, and visible bit numbers. Spatial admission accepts only integer row widths
from 1 through 64; the maximum is an explicit terminal presentation bound, not a restriction on
Mermaid syntax. Coordinates must be nonnegative exact integers no greater than `2^53 - 1`, and
DB `bits` must be finite nonnegative integers (zero and inconsistent split counts are preserved).
Other numeric values, overlapping or out-of-order fields, and row widths outside the spatial domain
retain complete structured records. Their actual output report says
`structured_text`, even though the family's primary capability is `diagrammatic`. Existing source
validation, resource, and cancellation errors remain errors and are never converted into success.
An admitted coordinate range can still exceed grid or work budgets; that returns the corresponding
resource error rather than changing projections.

Packet admits only Canonical, both Unicode/CJK width profiles, and the five primary encodings in
the table above. Width fallback is Plain through the existing bounded record reflow; there is no
new layout profile, output schema, or dependency. The [Packet admission note](ASCII_PHASE_GATE_REPORT.md#packet-bounded-spatial-admission)
records the small/typical/dense 80/100/120-column comparison and the executable verification owner.

## Structured-Text Outputs

These families intentionally produce a terminal-safe semantic report or outline rather than a
diagram. The output is useful for logs, debugging, and narrow terminals, and can aid accessibility
when the typed model retains the relevant metadata, but it is
excluded from diagrammatic-family counts and from claims that Merman has an ASCII geometry
implementation. These families emit structured text as their primary output. An additional bounded
reflow is selected only by the explicit viewport `Fallback` policy; resource limits and cancellation
remain hard errors and never trigger that fallback. A host may choose source display or truncation
as its own product policy, but neither is the renderer's complete structured fallback contract.

| Mermaid family | Semantic coverage | Primary projection | What the report preserves | Why it is not counted as an ASCII diagram |
| --- | --- | --- | --- | --- |
| GitGraph | Partial | Structured text | Newest-first commit history with bounded branch lanes, quoted identities/messages/tags, explicit parent references, cherry-pick source disclosure, and separate sequence/type/id override details. | Lane crossings or capacity conflicts, repeated IDs, external/forward parents, and nonchronological input use complete parent records in model order. This is a history report rather than Mermaid's two-dimensional Git layout; unknown direct-model commit types are rejected. |
| Journey | Partial | Structured text | Sections, tasks, actors, and scores, with quoted byte-length framing for authored strings and list items. | Does not draw Mermaid journey geometry. |
| Kanban | Partial | Structured text | Columns, stable authored group/card ids, independent repeated card occurrences, group parent ownership, assignments, metadata, and deterministic `Unassigned` grouping. Authored labels and metadata use quoted byte-length framing. | Drag/drop and nested board geometry are not represented; group parent ownership is disclosed as text; duplicate group ids and empty ids are rejected. |
| Mindmap | Partial | Structured text | Label-first hierarchy with charset-aware connectors, cycles/disconnected components, and referenced details for distinct authored ids, nondefault shapes, icons, and custom sections. Ambiguous labels are quoted; detail fields retain quoted byte-length framing. | Standard branch-derived sections are omitted only when the entire graph follows that rule. Rich browser node geometry is not reproduced; duplicate internal/authored ids, missing authored ids, duplicate edges, and missing endpoints are rejected. |
| Timeline | Partial | Structured text | Direction, sections, and events in ordered grouped text; authored task, event, section, title, and accessibility fields use quoted byte-length framing. | Does not draw Mermaid timeline geometry; parser bookkeeping score is omitted. |
| TreeView | Partial | Structured text | Root and node identities, authored levels, hierarchy, file/directory types, charset-aware connectors, and icon/class/description disclosure. Authored fields use quoted byte-length framing. | It is an outline rather than two-dimensional diagram geometry; duplicate node ids and unknown node types are rejected, and browser icons/CSS classes are disclosed rather than styled. |

## Unsupported Families

These families may parse or render to SVG elsewhere in `merman`, but they are not currently exposed
as ASCII/Unicode render targets. Railroad, Ishikawa, and Quadrant have completed the
R34 evaluation rather than merely waiting in an unclassified backlog; the tracked
[Phase 0-3 and admission report](ASCII_PHASE_GATE_REPORT.md) records their representative tasks,
fixtures, spatial facts, StructuredText comparison, and 80/100/120-column rejection matrices.
Requirement has since reopened admission with shared-endpoint geometry and complete records;
the [new evidence](ASCII_PHASE_GATE_REPORT.md#requirement-spatial-admission) supersedes its
historical tree-expansion rejection.

| Mermaid family | ASCII status | Notes |
| --- | --- | --- |
| Cynefin | Unsupported | No terminal projection has been admitted. |
| Event Modeling | Unsupported | No terminal projection has been admitted. |
| Info | Unsupported | Not useful as terminal diagram output today. |
| Ishikawa | Unsupported | [R34 evaluation rejected the current depth-limited prototype](ASCII_PHASE_GATE_REPORT.md#ishikawa-r34); its typical case adds spatial value, but dense recursive ownership is incomplete. |
| Quadrant | Unsupported | [R34 evaluation rejected the current collision prototype](ASCII_PHASE_GATE_REPORT.md#quadrant-r34); small/typical cases add spatial value, but dense relative order collapses at all required widths. |
| Radar | Unsupported | No terminal radar approximation yet. |
| Railroad | Unsupported | [R34 evaluation rejected the current route-expansion prototype](ASCII_PHASE_GATE_REPORT.md#railroad-r34); choice paths are visible, but repetition loops and dense shared prefixes are not. |
| Sankey | Unsupported | Flow widths are SVG-specific. |
| Treemap | Unsupported | Rectangle packing is SVG-specific. |
| Venn | Unsupported | No terminal set-overlap projection has been admitted. |
| Wardley | Unsupported | No terminal map projection has been admitted. |

### Further projection candidates

The current dispositions above remain authoritative. These candidates have useful spatial facts
in their typed models, but require a complete terminal slice before their capability changes:

| Candidate | Useful first spatial slice | Required evidence |
| --- | --- | --- |
| GitGraph | Commit lanes with a branch and merge. | Each commit appears once; every parent edge remains traceable, including merges with custom display types and cherry-picks. |
| Agentflow | Family-owned spatial roles and complete reading records. | Expand spatial metadata or collapsed visible topology only with source-backed ownership and complete-information evidence. |

Compare small, typical, and dense cases against the existing complete text representation at
80, 100, and 120 columns. More spatial output must help recover relationships or quantities while
preserving identity, labels, and the existing width, encoding, cancellation, and resource contracts.
For Railroad, Ishikawa, and Quadrant, reopening the earlier decision additionally
requires resolving the concrete ownership or quantization failure recorded in the linked admission
report. A new outline or table remains StructuredText.

## Playground Filtering

The playground "ASCII supported" filter uses the runtime capability metadata when WASM is ready and
a tracked family/projection fallback copy before WASM finishes loading. The filter is currently
family-level:
every example whose diagram type has an available terminal projection is included, so membership is
not a separate claim that every semantic detail in that example is preserved. Basic `classDiagram`
output is supported, including nested namespace containers. The preview and export UI show
diagrammatic coverage or the Structured Text projection plus a concise limit for the active diagram
type. Simple sibling-namespace, namespace-to-root, and nested-sibling relationships route through
the nearest namespace facades with length-framed leaf identity; dense or colliding namespace scenes
remain partial and use the lossless `relations:` fallback. Class and ER share directed component
ranks and bounded private channels for rings, DAG diamonds, and other admitted scenes. This does
not guarantee routing for arbitrary dense topology.

## Testing Policy

The admitted Flowchart/Sequence matrix is executable in
`crates/merman-ascii/tests/viewport_characterization.rs`. It covers canonical/compact layouts,
ASCII/Unicode structure, Unicode/CJK width profiles, Plain/ANSI16/ANSI256/TrueColor/HTML geometry
parity, and 60/80/100/120-cell Allow/Fallback/Error outcomes. The representative measurements are:

| Fixture | Layout | Extent | Blank cells | Longest blank run |
| --- | --- | ---: | ---: | ---: |
| Issue #53 Flowchart | canonical | `76×40` | 2099 | 52 |
| Issue #53 Flowchart | compact | `59×50` | 2103 | 42 |
| Sequence self-messages/notes | canonical | `107×58` | 4681 | 41 |
| Sequence self-messages/notes | compact | `105×58` | 4567 | 41 |

These values are characterization evidence, not universal quality scores. Admission still requires
complete authored-field retention and typed overflow behavior; a narrower result that drops a field
is rejected. Issue #53 Compact spends more rows while reducing width and area. Its
independent contours share rank centers, retaining all seven heads and complete conditions beside
visible route turns. This remains a fixture-specific tradeoff rather than a universal density claim.

`presentation_floor.rs` fixes the complete-document outcomes for the original 13-case terminal
gallery at 60/80/100/120 cells, in ASCII/Unicode and Unicode/CJK width profiles. Twelve-way LR
branches and merges occupy 47 rows. Sixteen-way TD fallback occupies 20 rows at 60 cells and
19 at 80/100/120; long-label fallback occupies 13/11/11/9 rows. Legends and detail rows count toward
these limits. `graph_layout_quality.rs` and the independent generated scene oracle protect the
endpoint, type, condition, multiplicity, and containment questions behind these measurements.

The generated supported-small-graph set fixes 486 Interactive scene attempts and 3,042 visible
edge contributions, with 2/5/8 nodes and at most 12 edges. All attempts remain in its denominator,
including failures. The generator covers nine topology categories, LR/TD, three structural/gap
requests, and three edge orders. Four-direction, CJK, State concurrency, and Swimlane containment
have their own public semantic tests; the generator does not claim to exhaust arbitrary graphs.

- Use exact snapshots when the ASCII shape itself is the contract.
- Use semantic assertions for structured-text output and relation-summary fallback, ensuring every retained entity/endpoint/label remains visible.
- Use explicit `UnsupportedFeature` tests for unsupported semantics instead of silently dropping
  Mermaid input.
- Keep Class/ER dense topology cases on the shared `relation_graph` summary path when routed output
  would overlap boxes; a resource-limit error remains an error and never becomes a summary.
- Keep directed cycles and DAG diamonds covered by Class/ER parser tests plus the shared
  `relation_graph` tests for declaration-order stability, disjoint routes, exact/N−1 work
  admission, and speculative-work/document-cell ownership on collision fallback.
