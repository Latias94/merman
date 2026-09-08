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
| Cynefin | yes | legacy bridge |
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
| Info | yes | legacy bridge |
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

Caller control over canonical SVG document limits remains an open repair unit. SVG construction,
pre-serialization admission, and encoder validation still apply protocol defaults; fixing only
one boundary does not remove the hidden limit. The intended integration separates shared
correctness validation from target policy: SVG uses the existing operation/work/output budgets,
while DrawingList output and external decoding retain exact protocol limits. Bounded SVG writing
and cancellation during expensive validation must remain part of that integration, not be bypassed
with maximal numeric limits. Earlier Gantt/Radar candidate SVG opacity/CSS issues also remain.
None of these local corrections admits another SVG family or completes the all-family goal.

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

The candidate serializer is exercised directly by
`cynefin_document_projects_source_groups_and_distinct_text_baselines`, without allowing the public
SVG bridge to satisfy its assertions. Canonical commands now own content scopes and item-local
translations; the SVG projection retains the source group classes, rounded badge rectangles,
distinct `middle`/`central` text baselines, and explicit accessibility-title exposure. The same
test verifies that changing external configuration after construction cannot change the SVG,
while edits to public text, paint, and background commands do change the serialized result.
Resolved presentation attributes replace the legacy theme stylesheet in the candidate serializer;
the empty style element remains structural only. The public DrawingList regression also checks
normal and overflow badge centers against their text.

Marker definitions now reuse public local paths and paint with the source `refX`/`refY` placement.
The marker-edit regression checks deduplication, changed shape/color, removed drawing commands,
and a non-clipping ordinary-path projection for edits outside the marker viewBox.

Cynefin remains bridged. Diagram-ID-dependent default boundary seeding still needs migration
before complete family comparison can admit its canonical SVG route.
The legacy dispatch test retains the pinned marker and accessibility ordering contracts;
it is not evidence for the candidate serializer.

The table is a family-level default, not a promise about every request.  The typed SVG result
exposes `SvgSerializationRoute`: `canonical-document` means the result was serialized from the
renderer-neutral document, while `legacy-bridge` records an explicit compatibility path selected
for a legacy family, a browser-only effect, or a diagnostic request.  When the bridge is selected,
`serialization_bridge_reason()` exposes a structured reason (`LegacyFamily`, diagnostic kind, or
the original `DrawingListUnavailable` family/effect message).  This distinction is useful for
migration telemetry and prevents an effect-specific fallback from being mistaken for complete
canonical coverage.

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
