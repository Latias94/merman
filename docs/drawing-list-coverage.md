# DrawingList family coverage

`fixtures/drawing-list/v1/family-coverage.json` is the machine-checked inventory for the
renderer-neutral document migration.  It deliberately distinguishes two facts that are easy to
confuse:

- `document_adapter: direct` means the family builds a typed `RenderDocument` without parsing SVG.
- `svg_serializer: canonical` means the SVG target currently serializes that document directly.
  `legacy-bridge` is a temporary, explicit migration status; it does not mean that the family is
  missing DrawingList output.

The coverage test compares the fixture with `RenderFamilyKind::ALL`, so adding a family without an
inventory row fails at test time.  The current inventory is:

| Family | Direct document adapter | SVG serializer |
| --- | --- | --- |
| Error | yes | canonical |
| Mindmap | yes | legacy bridge |
| State | yes | legacy bridge |
| Sequence | yes | legacy bridge |
| ZenUML | yes | legacy bridge |
| Flowchart | yes | legacy bridge |
| Swimlane | yes | legacy bridge |
| Architecture | yes | legacy bridge |
| Class | yes | legacy bridge |
| C4 | yes | legacy bridge |
| Cynefin | yes | canonical |
| Wardley | yes | legacy bridge |
| Railroad | yes | legacy bridge |
| Kanban | yes | legacy bridge |
| Gantt | yes | legacy bridge |
| Pie | yes | canonical |
| Packet | yes | canonical |
| Timeline | yes | legacy bridge |
| Journey | yes | legacy bridge |
| Requirement | yes | legacy bridge |
| Sankey | yes | canonical |
| Radar | yes | legacy bridge |
| Info | yes | canonical |
| Treemap | yes | legacy bridge |
| Block | yes | legacy bridge |
| ER | yes | legacy bridge |
| QuadrantChart | yes | legacy bridge |
| XYChart | yes | legacy bridge |
| GitGraph | yes | legacy bridge |
| TreeView | yes | legacy bridge |
| Ishikawa | yes | legacy bridge |
| EventModeling | yes | legacy bridge |
| Venn | yes | legacy bridge |

This is an honest migration snapshot, not a release claim.  A family may move from `legacy-bridge`
to `canonical` only after both focused fixtures and the complete family SVG comparison prove that
root geometry, style, DOM/a11y obligations, and effect disposition remain source-backed.  ZenUML
uses a separate external-plugin evidence lane and remains bridged until that lane is admitted.
Unsupported browser-only effects must remain explicit DrawingList errors or bounded raster
fallbacks; they must not be hidden by the bridge.

### Admission correction, September 8, 2026

The structure comparison generated at 15:15 +08:00 failed for 14 of the 17 then-admitted families.
Their public canonical admission is withdrawn; the direct adapters and candidate assertions remain.
Info adds a background path absent from the upstream DOM, and Sankey changes the source group
hierarchy while applying both command opacity and stylesheet stroke opacity. The other failed
cohorts are Railroad, Kanban, Gantt, Journey, Requirement, Radar, XYChart, GitGraph, TreeView,
Ishikawa, EventModeling, and Venn. These are not classified as font or floating-point residuals.

Error (4/4), Packet (33/33), and Pie (69/69) passed that structure run. This is historical evidence,
not a claim that the current HEAD has passed the complete root/parity/release matrix: those reports
were not bound to a commit, and earlier root reports predate later migration changes. Candidate
tests that require withdrawn public routes are explicitly ignored pending migration; ignored tests
are not admission evidence. Live route tests check the explicit compatibility result separately.

### Direct-output audit follow-up

SVG admission and DrawingList correctness are separate gates. The September 8 follow-up addresses
Mindmap depth-based edge widths, Flowchart node-label opacity isolation, Sequence Point marker
geometry, Journey background paints, Radar text fill inheritance, exact JSON float round trips,
and Web/Python error-detail exports through direct-output or public binding tests.

The follow-up repair batch restores all-bars-before-labels Gantt ordering, single-object Treemap
fill/stroke compositing, Class relationship-label backgrounds and transparent terminals, and
Architecture service-title baselines/bounds. Class translucent HTML relationship backgrounds
remain an explicit unsupported effect until inline box geometry can represent both source
background layers; SVG text labels retain the exact `mainBkg` alpha. Protocol-limit failures now
latch in the shared operation control before returning, so later cancellation cannot replace
their resource terminal. Public quota errors keep the same stable limit ID and exact counts.

The shared validator now accepts a caller-owned admission/cancellation callback independently of
document correctness. The existing `validate`, bounded decoder, and JSON encoder still use the
same protocol defaults or explicit limits. Resource counts are admitted before expensive payload
walks; path segments, glyphs, dash entries, extension values, and PNG rows have cancellation
checkpoints. The bounded builder uses this path during final validation and returns no document
on cancellation. Focused tests retain the default decoder's nesting rejection, exercise a different
in-memory admission policy without accepting invalid scopes, and reject/cancel PNG validation
before or during pixel decoding. This is a validation foundation, not yet an SVG policy change.

Canonical SVG construction, validation, and serialization admission now use `SvgOperation`, not
the default DrawingList quantity limits. DrawingList output and external decoding retain their
exact protocol limits. Both targets share correctness checks and the existing operation meter;
validation preflights resource work and serialization charges the complete footprint once.
The public Packet regression expands a roughly 2 KiB label source beyond 16 MiB of rendered text:
it previously failed at the implicit protocol text ceiling even with both input and rendering
profiles unbounded, and now requires an actual canonical SVG result. A separate exact-byte test
checks success at the final SVG length and sticky resource rejection one byte below it.

Canonical emission and root construction share a fallible buffer that admits absolute SVG bytes
before appending, preserving cancellation/resource errors across formatting. Path data, attribute
escaping, and image/font Base64 emission stream into that buffer; existing icon byte reservations
are not charged twice. This bounds retained SVG output, not total process memory. Some family
CSS/HTML component strings and metadata still materialize before append and need further resource
integration as migration proceeds. Earlier Gantt/Radar candidate SVG opacity/CSS issues also remain.
None of these local corrections admits another SVG family or completes the all-family goal.

Pie path elements now stream geometry, inline paint, dash entries, transform, and opacity directly
into that buffer without intermediate path/CSS/element strings. Rounded and ordinary-number path
formatting share the same segment traversal; exact spelling tests retain their distinct number
policies, and a rejecting sink checks that formatting stops at the first failed write. The nested
formatter resource-error test also exercises Pie geometry. The remaining component-string caveat
above still applies to stylesheets and other family projections.

### Info migration evidence

The Info candidate now projects its initial full-viewport white command into the source root
background style, without adding a path or deleting the public DrawingList background. Modified
or removed background commands retain their ordinary paint or transparent root, respectively.
The version label and its resolved font, color, alpha, and position live only in public commands;
the redundant sidecar version string and external-config stylesheet interpretation are removed.
Shared text styles are emitted from those commands. Noncompact text/state edits retain the full
command serializer instead of losing direction, baseline, language, stroke, or graphics state.

`info_document_owns_background_and_text_styles` directly exercises the candidate serializer,
including conflicting external CSS/config, modified and removed paints, font/color/position edits,
and a noncompact baseline/opacity edit. The public-route test checks `CanonicalDocument`, the exact
source child-element structure, and the Courier fixture's font projection. Dynamic colors and
fonts continue to report an explicit effect-specific bridge rather than silent substitution.

Info is admitted after the September 8, 2026 complete structure and parity-root runs, each selecting
and rendering all 15 pinned fixtures with zero skips and no accepted residual policy. Reports are
`target/compare/info_canonical_structure.md` and `target/compare/info_canonical_parity_root.md`.
The latter checks all 15 root viewports. Stylesheet text is outside those comparators' scope, so
the font/color/alpha mutation assertions remain necessary evidence; these runs do not establish
browser-computed-style equivalence. This advances default canonical admission to five families,
not completion of the remaining 28 family migrations.
The existing `boundary_fixtures_render_typed_resvg_safe` test also passed with `png`, covering
the boundary corpus (Error, Info, ZenUML) through terminal SVG validation and nonblank raster output.

### Journey color correction

The direct adapter resolves section and task backgrounds from the theme's `fillTypeN` rules,
falling back to the layout fill only when that class has no theme entry. Default visible HTML
labels use `textColor`, independently of those background fills. The candidate SVG serializer
no longer injects Journey's legacy stylesheet or external theme CSS over the resolved commands.
`journey_document_owns_visible_label_and_background_colors` checks distinct text, theme-background,
and layout-background colors under default and dark themes, then exercises the candidate serializer
directly with conflicting external configuration. This is color regression evidence, not complete
DOM or browser parity: the source HTML shell, non-default `textPlacement` behavior, and full-family
comparison remain migration work. Journey remains on the public legacy SVG route.

### Sankey migration evidence

`sankey_document_owns_paint_order_and_svg_styles` exercises the candidate serializer directly,
without the public legacy route. The document now paints all nodes, then all labels, then all
links in pinned source order. Candidate SVG does not inject the legacy stylesheet or reapply
external theme CSS: changing a public link opacity or text paint changes that output exactly once.
Outlined labels use public text strokes and two ordered text layers, including the resolved
background alpha, rather than being rejected for a protocol feature that now exists.

Node geometry now uses public local rectangles and transforms; the SVG node group projects those
transforms into source-shaped translation/x/y attributes and source-numbered, diagram-scoped IDs.
The node mutation regression checks that moving a public transform moves the serialized node.
The backend-local crisp-edge hint uses the source `.node rect` stylesheet selector; a missing
public stroke uses SVG's default, while an explicit stroke remains on the rectangle.

Single-stroke link scopes project their common public opacity into the links collection and
their public blend mode into the individual link group. The encoder leaves its drawing state
intact. Nonuniform opacity, translucent solid strokes, added fill/draws, or nondefault stroke
styles use the full command serializer instead, without falling back to the legacy renderer.

Referenced linear gradients are now placed with their link paint, retain source numbering, and
project edited public colors/transforms without copying them into the sidecar. Unreferenced
resources cannot create painted links.

Linear gradient projection now omits only SVG-equivalent default ordinates, identity transforms,
and pad spread, and spells Sankey stop offsets as percentages. Mutation coverage preserves
nondefault ordinates, transforms, reflect spread, fractional offsets, and stop alpha.

The root background is an explicit public paint command. SVG projects the full-viewport white
paint into its root background style; changing or removing that command changes the output
without retaining an implicit white background. The mutation regression covers both cases.

Labels now use direct source-shaped text elements and a shared font-size group when their public
styles agree. Inherited CSS is derived only from those commands; outlined strokes remain on each
text. SVG y/dy decomposes the public baseline without adding an offset, including after public
font-size or position edits. Heterogeneous styles retain explicit per-command text attributes.

Sankey is admitted after its 2026-09-08 complete structure and parity-root comparisons: each
selected and rendered all 33 fixtures, with zero skips and no accepted residual policy. The
parity-root run checked all 33 root viewports. Reports are
`target/compare/sankey_compact_candidate_structure.md` and
`target/compare/sankey_compact_candidate_parity_root.md`. These are source-SVG/DOM results, not a
claim of separate browser-computed-style validation. The public-route regression
requires `CanonicalDocument` and checks source-shaped nodes, labels, link groups, and live
gradient references. Public-command mutations cover the projection's noncompact cases.
The separate `all_supported_fixtures_render_typed_resvg_safe_audit` also passed with
`MERMAN_RESVG_SAFE_AUDIT_FAMILY=sankey` and the `png` feature: terminal SVG validation, PNG
rasterization, decoding, and nonblank-ink checks ran on the supported Sankey fixture corpus.

The 2026-09-08 candidate check at `47831d275` temporarily enabled the canonical route and ran
`compare-sankey-svgs --check-dom --dom-mode structure --dom-decimals 3`. All 33 fixtures rendered
with route evidence; all 33 still failed structural comparison. The temporary admission change
was removed. That run's differences included link-layer fill/stroke-opacity attributes, per-path
generic attributes, gradient default attributes and percentage stop spelling, and rectangle
stroke/rasterization-hint attributes. The local diagnostic report is
`target/compare/sankey_label_candidate_structure.md`; no comparator normalization was changed.

### Cynefin migration evidence

Cynefin is admitted after the September 8, 2026 full-family structure and parity-root runs:
`target/compare/cynefin_exact_alpha_structure.md` and
`target/compare/cynefin_exact_alpha_parity_root.md`. Each selected and rendered all 13 fixtures
with canonical route evidence and zero skips; parity-root also checked all 13 root viewports.
The restored public SVG test asserts the canonical route, source marker references, content
groups, and accessibility metadata. This evidence does not admit other families.

Canonical commands own content scopes, item translations, domain/boundary geometry, and resolved
text and paint. The SVG sidecar retains structural identity/classes, root width policy, and title exposure.
The Confusion overlay follows boundaries and precedes labels. Both source accessibility DOM
copies consume the same public semantics. Direct serializer tests verify that external config
cannot reinterpret the document, while public color, font, geometry, and command edits do affect
the output. Normal and overflow badge centers are also checked through the public DrawingList API.

Independent fill opacity is represented exactly with Save/SetOpacity/fill-only DrawPath/Restore,
then a stroke-only DrawPath referencing the same resource. No extra resource or protocol variant
is needed. This removes the former `0.5 → 0.502` and `0.95 → 0.949` quantization rather than accepting
it as a browser residual. SVG and shared CSS use one complete-window recognizer; only equivalent
normal-blend operations with unchanged external opacity coalesce into a source element. Edits
to opacity, added/removed strokes, or external opacity/blend retain their actual public-command
semantics. Command/dash admission is checked before appending a resource reference.

Identical registered styles may share class CSS; heterogeneous styles retain explicit attributes.
Marker definitions reuse public paths and paint with the source refX/refY and close-command
spelling. Mutation tests cover deduplication, changed/removed geometry and paint, unregistered
transition IDs, and non-clipping fallback outside the source marker viewBox. The comparator only
normalizes valid hex-fill letter case on four source element classes; distinct color channels,
alpha, dynamic/invalid paints, other families, and Strict mode remain sensitive.

Default boundary seeding consumes normalized render-instance identity before construction.
DrawingList exposes `DrawingListRequest.diagram_id`; bindings forward the existing `svg.diagram_id`
option. Default, empty, normalized identity, and explicit Mermaid seed precedence are tested
against source-shaped boundary segments. Unsupported browser effects retain the explicit
policy/error bridge described below; default canonical admission is not an all-effects claim.

The table is a family-level default, not a promise about every request.  The typed SVG result
exposes `SvgSerializationRoute`: `canonical-document` means the result was serialized from the
renderer-neutral document, while `legacy-bridge` records an explicit compatibility path selected
for a legacy family, a browser-only effect, or a diagnostic request.  When the bridge is selected,
`serialization_bridge_reason()` exposes a structured reason (`LegacyFamily`, diagnostic kind, or
the original `DrawingListUnavailable` family/effect message).  This distinction is useful for
migration telemetry and prevents an effect-specific fallback from being mistaken for complete
canonical coverage.

### Venn paint and style migration

Classic Venn paths preserve independent fill and stroke opacity with exact command-state values,
not rounded color alpha. Both draws reference one path resource; transparent intersections retain
an unpainted path inside their semantic group. Public facade tests cover custom `0.42` fill and
`0.95` stroke opacity, while the direct serializer test covers default paints and transparent
intersection geometry. The latter also checks that external configuration cannot override public
text/paint edits and that only an unchanged full-viewport white command projects into the root
background style. Candidate SVG no longer regenerates Venn visual CSS from configuration.

The candidate now projects the content translation and area groups from public semantic scopes,
coalesces equivalent independent fill/stroke scopes into one source path, and emits single-line
labels as text/tspan pairs. Empty intersection labels keep public text geometry rather than
putting coordinates in the sidecar. Accessibility metadata precedes styles; structural content
is not hidden by the cluster debug filter. Edited transforms, alpha, stroke caps, text styles,
text blend modes, and title positions retain their public-command meaning. External `themeCSS`
is not reapplied at the end of canonical serialization.

The September 8, 2026 candidate structure run selected all 12 fixtures: the seven classic fixtures
rendered canonically without structural mismatches, while three hand-drawn and two text-node
fixtures failed the canonical route requirement. The command therefore failed overall. The report
is `target/compare/venn_migration_candidate_structure.md`; the companion stricter report is
`target/compare/venn_migration_candidate_parity_root.md`. Temporary route/inventory admission used
for this diagnostic was withdrawn; neither run is evidence of full-family readiness.

This is not full-family admission: Venn remains on the explicit legacy SVG route. The stricter
candidate parity-root comparison still reports circle path command spelling and title
presentation-attribute/style differences. These have not been accepted as residuals or hidden by
normalization. Text-node layout/text-area geometry and typed RoughJS path projection also remain
migration work. These features are not inherently outside the vector protocol; missing
implementations must be resolved before full-family admission.

### Venn text-layout source characterization

The September 8, 2026 Chromium 151 probe corrected the earlier assumption that ordinary area
labels require width-based wrapping. Mermaid calls `@upsetjs/venn.js`'s `wrapText` while its dummy
SVG is detached, before final font styles or attachment. Instrumenting the pinned Mermaid render
showed all computed-length probes returning zero with `isConnected=false`; a deliberately long
label retained one tspan. Calling the same library wrapper after attachment produced three lines.
The direct adapter therefore keeps one area-label run, but now reproduces JavaScript whitespace
tokenization, including NBSP, EM SPACE, and BOM. Borrowed fragments pass through the bounded text
builder before allocation and measurement. Visible titles keep their separate SVG whitespace
rules. The public regression checks both behaviors without relying on the legacy SVG route.

HTML text nodes have a different contract: plain `.text()` content, normal whitespace/word
breaking, intrinsic flex sizing, and `line-height: normal`, resolved after attachment. In the
same browser probe a 20px node used two 23px line boxes, not 1.5em lines; this observation is not
a portable font constant. Implementing this surface requires explicit normal-line metrics and
resolved line positions. Existing SVG wrappers that split long words or interpret `<br>` are
not behavior-equivalent substitutes, so text-node output remains an explicit capability error.

[ADR-0088](adr/0088-atomic-normal-line-metrics.md) defines the missing atomic line-metrics
measurement and its append-only native callback evolution. Neither the new operation nor Venn
text-node support is admitted by that design record.

## Exercised effect accounting

`fixtures/drawing-list/v1/effect-coverage.json` is the focused effect evidence used by
`drawing_list_effect_accounting`.  It covers the visual constructs currently exercised by admitted
fixtures: portable path paint, host text, gradients, clips, semantic links, opacity, blend modes,
transforms, and expanded marker geometry, plus explicit fail-closed outcomes for browser-wrapped
text, filters, hand-drawn RoughJS output, and external icon registry content.  Each row is executed
against the typed renderer and must produce its declared vector or structured-error disposition;
no row may be an unclassified best effort.  The test also collects the effect kinds actually
observed across successful fixtures and fails when one has no corresponding assertion, so an
adapter cannot start emitting an already-modeled visual command without extending the evidence.

The matrix intentionally does not claim that every protocol resource kind is emitted by a current
family.  Patterns, inline images, glyph/outline text, and raster subtrees remain protocol-level
capabilities with dedicated display-list validation fixtures until a family admits them through a
source-backed renderer slice.
