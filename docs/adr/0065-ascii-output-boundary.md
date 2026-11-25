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

## Family-owned terminal geometry amendment

Usecase owns the projection of actors, use cases, boundaries, notes, and relationship roles into
the shared graph scene. It resolves its own layout policy; admitting the family does not admit
Flowchart's wrapping, Compact/Auto selection, or adaptive presentation policy.

Block owns integer terminal packing from the flat semantic hierarchy. Columns, spans, source
order, expanded spacers, and nested frames determine positions. A private positioned-graph seam
measures contours and labels and submits this fixed geometry directly to the existing scene
router and painter. It does not run Dagre, reuse SVG coordinates, or reflow authored blocks to
repair congestion. Relationships still require complete validated routing, with the existing
resource ledger, endpoint markers, label ownership, viewport admission, and operation control.

Both families admit Canonical and Strict only. A valid primary that exceeds a Plain viewport can
select complete family-owned keyed records under the existing Fallback policy. Unsupported
semantics and failed routing remain diagnostics. Capability records expose partial semantic
coverage and the actual diagrammatic or structured projection; they do not imply unrestricted
family support.

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

## 2026-09-11 automatic layout selection addendum

Flowchart and Sequence additionally admit `AsciiLayoutProfile::Auto` with an explicit width bound.
Auto makes at most two primary attempts: Canonical followed by Compact on width overflow. It
selects the narrower valid diagram, keeping Canonical on ties, then applies the existing
Allow/Error/Fallback policy. Fixed Canonical and Compact requests retain their fixed selection.
Flowchart Compact now defaults to a three-cell horizontal rank gap, alongside its existing
24-cell node wrap; explicit family overrides win. Other family density policies are unchanged.

Primary work remains cumulative across attempts. Retained candidate text is included in storage
admission; resource/cancellation failures remain terminal. Canonical route errors are not converted
to width fallback. A route error encountered only in the optional Compact candidate can retain a
previously valid Canonical primary. The structured fallback remains Plain-only.

ASCII report and metadata schema 3 add `requested_layout_profile` and `compact_attempted`.
`layout_profile` and primary extent identify the selected primary geometry, including when the
final output is structured fallback. Compact selection is independent from fallback selection.
The options JSON schema remains version 2. Width fitting does not guarantee reduced height/area,
and it does not authorize edge-label truncation or automatic direction changes.

## 2026-11-16 scene planning and adaptive output addendum

Flowchart, Swimlane, and State now use one private scene planner for stable semantic demand,
actual-blocker transactional repair, and at most two measured capacity rebuilds per profile.
Required feasibility/replacement and optional complete-scene quality have separate finite
allowances within the same cumulative operation ledger. Geometry generations invalidate contacts
and occupancy; a failed trial retains the incumbent and the work already spent.

`AsciiGraphPresentation::Strict` remains the default. Explicit `Adaptive` is Plain-only for these
three families and may select the complete graph-document projection after an admitted routing or
capacity miss. It never catches arbitrary unsupported semantics, resource exhaustion, allocation
failure, or cancellation. Adaptive Auto may try Compact once after a Canonical planning miss;
explicit profiles remain fixed, and State remains Canonical-only. Width policy remains independent.

ASCII output schema 4 replaces the old `overflowed` field with nullable `primary_overflowed` and
measured `emitted_overflowed`, adds `requested_graph_presentation`, and makes primary dimensions
and effective layout nullable together when no primary geometry was accepted. Numeric zero still
identifies a valid empty scene. Options JSON remains 3 and outer operation metadata remains 1.
The [current migration](../migrations/render-reuse-host-integration.md) owns consumer instructions;
the [support matrix](../rendering/ASCII_SUPPORT_MATRIX.md) owns family admission. The dated
schema-2/schema-3 decisions above retain their historical meaning.

## Fixed routing space for flat Flowchart

The default `AsciiGraphRoutingSpace::Expand` reserves node contacts and routing corridors before
search and can rebuild a layout after a planning miss. In issue #370, those reservations brought
the output to 48 rows at a preferred two-cell rank gap and zero vertical node padding.
[#314](https://github.com/Latias94/merman/pull/314),
[#323](https://github.com/Latias94/merman/pull/323),
[#370](https://github.com/Latias94/merman/issues/370)

`AsciiGraphRoutingSpace::Fixed` lays out an ungrouped Flowchart from measured content, shape
geometry, requested spacing, authored edge lengths, and the minimum stroke and marker cells.
It uses the same bounded route, port, and label search on that geometry. The host receives
`RoutingSearchExhausted` or `PortCapacityExceeded` on a planning miss and can submit a new request
with `Expand`. Fixed requires Strict presentation. Each selected width profile applies the same
routing-space policy, and the viewport still handles the measured width of a valid scene.

## Terminal layer ordering and coordinate ownership

Flowchart, State, and Swimlane retain Dugong's rank seed. Cross-axis placement uses bounded
forward/backward barycenter ordering and integer layer compaction, allowing both branches and
merges to influence alignment. State side constraints remain part of the ordering. Compound
placement continues to enforce containment and local direction before frame allocation.

Terminal coordinates and node dimensions follow each node's own text, shape, and contact demand,
including scenes with containers. Shared axis bands reserve channel space without stretching
unrelated node contours. Compound frames retain the complete internal label footprint separately
from node outlines. Consecutive unallocated axis bands share one default routing corridor after
local direction changes; explicit node, label, edge-length, spacing, and empty-group allocations
retain their full dimensions. Vacated ranks do not accumulate empty wire tails at group exits.
Actual-contact orthogonal channels and terminal search own route geometry.
Finite candidate paths admit their retained cells before allocation. The final scene owns the
two-dimensional grid admission, so a discarded projection or unselected exterior candidate cannot
reject a fitting diagram by charging an imaginary canvas.

Scenes identify feedback connections before route admission. Forward links reserve their channels
before feedback connections; within each class, shorter rank spans precede flexible skip edges.
This keeps a return arrow from consuming an aligned forward link's only straight corridor.
Feedback routes still pass the same endpoint, label, and scene validation, with bounded congestion
repair available when an earlier commitment blocks them. Short direct and exterior
channels are compared before terminal search. Candidate labels receive valid host positions before
scoring, and scene growth includes those positions. A growing scene can trigger bounded refinement
even without a port conflict. Optional repairs share one scene-wide expansion allowance, while a
search for the first valid path remains subject to the operation budget. Predetermined parallel
self-loops retain deferred label placement until their complete nested batch is known. Exhausting
the optional allowance cannot flush that batch. Every candidate must carry its visible edge label;
geometry that drops a condition is not admissible. These changes replace the sequential node-slot
placement mechanism. The common presentation corpus separately checks complete output and tracing
effort; layer alignment by itself is insufficient evidence.

Compound Flowchart routing uses the nearest common frame's explicit direction, or the canonical
root direction when an edge spans frames, to order forward links and select endpoint sides.
Local directions are interpreted before the existing root output transform. Unresolved implicit
scopes retain generic routing. A forward contour gap offers at most three candidate contacts per
preferred side; direct candidates also try turns just beyond the endpoint reading margins. The
bounded direct planner and terminal search retain the same preference when replacing coarse
candidates or replaying a congestion proposal. Endpoint-port reuse precedes the forward-contact
preference; a blocked preferred side or an independent alternate contact can therefore justify
changing sides. Jointly allocated branch channels keep their scene-wide ownership. Flat graph
routing retains its original work accounting, and optional compound refinement shares the existing
scene allowance rather than adding a new budget.

Within the selected endpoint preference, candidate quality compares combined path length,
shared-cell cost, and scene growth before the number of legal transverse crossings. A crossing does not categorically
justify a longer route that passes its target and returns from below. Hard occupancy checks still
reject unrelated collinear sharing, mixed-stroke junctions, and label or marker collisions.
Reciprocal edges obey the same crossing rule and retain separate transit paths.

## Whole-scene demand and compact contours

Before allocating coordinates, scene demand validates every endpoint, including invisible layout
constraints, and measures complete edge-label blocks. Compatible flat branches and merges may
reserve one common contact. Parallel, reciprocal, mixed-style, and compound demand retains private
contact capacity. Invisible constraints do not disable an otherwise valid visible channel. Compound
scenes reserve both contact axes before local directions and boundary routing choose a side.
Growing a slanted contour also grows its width, preserving the label's interior clearance.

Flowchart and Swimlane default to zero vertical node padding and a one-cell preferred vertical
rank gap. Canonical horizontal rank spacing and Compact's family-specific wrap and horizontal gap
remain unchanged. Explicit axis padding or `with_node_padding` / `with_graph_padding_y` overrides
retain their requested values. State resolves its original family policy independently.
Visible edges reserve marker cells and an actual stroke even at zero preferred spacing. A label
cannot replace a turn or either immediate continuation at a crossed container frame. Those reading
guards are extracted once per plan and reused across candidate label positions.

## Reversible route contributions

If ordinary routing has no valid path for a non-loop edge, scene routing may give
that edge priority over a bounded set of earlier contributors. A scene shares a four-proposal allowance and
the existing optional terminal-search expansion allowance. Each proposal replays the retained
contributors into a provisional occupancy, admits the blocked edge, and replans the displaced
edges. Every replacement must materialize its markers and labels before the proposal replaces the
incumbent. Routine proposal failure restores the original route order without allocating or
replaying the old scene. Cancellation and resource failures restore geometry and propagate;
consumed work is never refunded.

Search uses an implicit compressed orthogonal channel graph. Its sorted axes contain obstacle
boundaries, actual contour ports, route turns, marker claims, and complete label-reservation
boundaries with reading/transit clearance. The Cartesian product is never allocated. A* moves
between adjacent coordinate events, preserving direction, bend, crossing, and congestion costs;
each segment still checks every covered terminal cell against occupancy and container access.
Only a winning channel expands to the unit-cell path used by the existing canvas. Long empty
spans therefore consume few search states without skipping resource work or collision checks.

Replaying preserves canonical edge identities while remapping dense occupancy indices. Active
cells, exact materialized markers, label-to-host reservations, and original terminal candidate
claims are distinct contributions. A relocated marker must not create a new candidate window
when its route is replayed. Provisional occupancy retains and admits the original scene's route
capacity, including replacement and later edges.

Repair considers at most three recent contributors intersecting the blocked corridor. It tries
each alone, then the complete retained conflict set, before required terminal search can consume
the remaining operation budget. The two-contributor regression requires both earlier conditions
to move; neither single removal opens the new edge. No-route results remain heuristic failures,
not certified spatial impossibility. `RoutingSearchExhausted` retains the original, zero-based
authored edge index through canonical ordering and public diagnostics. Resource and cancellation
errors remain distinct and terminal.

Fixed Text, State pseudostate, choice, and fork/join contours also have a necessary capacity
preflight after joint allocation. It counts independent marked endpoints against an upper bound
of admitted contacts times the bounded marker-candidate window, collapsing only explicitly shared
heads. `PortCapacityExceeded` certifies pressure beyond that current presentation's bound; it does
not establish that every alternative layout or representation is impossible. Obstacles and failed
heuristic searches cannot tighten this certificate.

## Compatible branch and merge channels

Flat forward branches or merges may allocate their common contact, channel, and private endpoint
contacts as a group before route selection. The group retains every canonical edge identity and
keeps conditions on private legs. Common-source groups require the same source marker kind; their target
markers remain independent. Common-target groups require the same target marker kind; their source
markers remain independent. Both require distinct private endpoints with distinct cross-axis
intercepts and compatible strokes and styles. Parallel, reciprocal, self-loop, container,
incompatible, or insufficient-space groups continue through ordinary route planning. Complete
label measurements and authored minimum main-axis spans are checked before an allocation is offered.

An edge belongs to at most one channel group. Source groups have deterministic precedence over
target groups, avoiding a joined bus that implies unrelated source/target pairs. A common target
marker or common source marker may share only its allocated terminal berth. Each route retains its own materialized marker;
occupancy needs one representative, which replay chooses again after removing any contributor.
Marker relocation does not grant a new sharing position.

Each admitted contributor owns its allocated geometry, not just a sharing identifier. A valid
joint channel takes precedence over shorter per-edge alternatives. Optional searches cannot
dismantle it, and a congestion reorder must re-admit the same channel or reject the proposal.
Its compatibility identity allows shared trunk cells without a congestion penalty. Unrelated
contributors may cross transversely but cannot merge collinearly into that trunk and imply new
source/target combinations. Conditions require a private label host; placing clear text beside
an already shared host is insufficient.

Collinear sharing requires a declared compatible channel, an exact common terminal ray, or the
actual shared contact. Unrelated same-stroke routes may cross transversely but cannot join into a
bus that implies an additional relationship. Short rank intervals reserve their scarce corridors
before flexible skip edges. Canonical edge identity breaks ties deterministically.

An unrelated transverse crossing paints the horizontal stroke as an overpass, leaving a one-cell
gap in the vertical stroke. The retained plan keeps both directions and stroke identities; a real
branch or merge retains its junction. Frame portals remain part of the closed container outline.
This distinguishes a crossing from a shared bus without adding a legend to ordinary diagrams.

Before materialization, the retained scene is checked against canonical edge identity, endpoint
ownership, complete labels, marker requests, shape contacts, container access, and shared-channel
geometry. The canonical leader of an allocation may have taken a private route; validation uses
the actual retained members as its geometric representatives. Validation performs no route search.
Reciprocal step connectivity joins both terminals and every active contribution, allowing only
the suppressed terminal-tail exception. One retained presentation index detects label/host-gap
collisions with other relationships, markers, and labels before text overlay can erase information.

## Readable graph width fallback

Flowchart, State, and Swimlane use a graph document when the selected primary exceeds an explicit
Fallback viewport. Node and group keys distinguish identity, membership, and actual endpoints;
each edge retains a separate relationship record with its markers, stroke, and complete condition.
Flowchart declares nodes on first relationship use, reducing duplicate tables and key lookups;
isolated nodes and extra authored details remain available. Legends describe distinctions that
are actually used. State retains its pseudostates, composite membership, concurrency, notes, and
authored document facts. Grapheme-safe reflow preserves complete text under Unicode and CJK width
policies, using the existing bounded serialization and shared operation ledger.

Node and relationship records prefer whole space-delimited words, including short type cues.
Physical continuations use two leading layout spaces when the next encoded fragment fits beside
them. Long tokens still split only at safe grapheme/escape boundaries; a narrow viewport may omit
the indent to retain an otherwise representable atomic escape. Authored spaces remain part of
their fields. Word lookahead streams borrowed slices through the existing terminal-safe visitors
without allocating an escaped record. Its work and all continuation bytes/cells remain charged to
the operation, including failed or cancelled attempts.

This changes presentation without adding a backend option or changing report schema 3. Allow,
Error, fixed layout profiles, Auto's two-attempt bound, Plain-only fallback admission, and the
precedence of cancellation/resource errors retain their established meanings. A failed initial
route search does not turn into a width fallback.
