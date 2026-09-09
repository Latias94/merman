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
integration as migration proceeds. The Gantt paint correction below removes its repeated CSS
interpretation; the earlier Radar candidate SVG opacity/CSS issue remains.
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

### Gantt paint and group-opacity correction

Gantt ticks now put their line and label in one public `BeginLayer`/`EndLayer` opacity scope.
The source's `.grid .tick { opacity: 0.8 }` composites the group, not each primitive independently.
Previously the public line had per-command opacity while the public label did not; the candidate
SVG then applied another parent opacity through legacy CSS. The new layer carries the union of
the stroke-expanded line bounds and reserved text bounds. Layer scopes use the existing builder's
nesting budget and LIFO checks; restore cannot cross an unfinished layer.

The candidate serializer no longer re-runs the Gantt theme stylesheet. Paint, fonts, opacity and
milestone geometry come only from public commands, while source cursor and rasterization hints
stream through the bounded SVG writer. A document/config independence regression and public paint
mutations verify this boundary without using the legacy route. Source-browser inspection also
confirmed that D3 tick lines own `stroke="currentColor"`; their parent `gridColor` stroke is not
inherited, and the isolated SVG resolves the lines to black. Labels retain their explicit
`stroke="none"` and theme text fill. A nondefault `gridColor` regression prevents restoring the
incorrect inherited paint.

This fixes direct DrawingList behavior and the candidate's paint ownership. Gantt remains a
legacy-bridge SVG family until its source DOM projection and full-family gates are complete.

The next projection step adds public collection scopes for excluded intervals, row backgrounds,
tasks and section labels, matching the ordered root groups created by the pinned Gantt renderer.
An enabled but empty exclude layer retains its group. The SVG document/title semantic scopes
do not add a second outer wrapper; task scopes and navigation targets remain intact. These new
collections pass through the existing bounded builder, including semantic and nesting limits.

Root accessibility references now distinguish an authored `accTitle` from the public document's
fallback title. The sidecar records only whether that title belongs in SVG accessibility chrome;
the title/description text still comes from public semantics. The first full-viewport white paint
uses the existing exact root-background projection. An edited translucent background instead
remains an explicit draw command with no implicit white root fill. Direct candidate tests cover
both accessibility cases, empty excludes, collection order and background mutation without using
the legacy route. Axis transforms, tick wrappers and section-text shells still require convergence;
this step does not admit Gantt as canonical.

The full candidate structure run at `83b688bbc` used temporary Gantt route/coverage admission:
`compare-gantt-svgs --check-dom --dom-mode structure --dom-decimals 3`. Of 157 selected fixtures,
five retained their existing baseline skips, 150 produced canonical SVG and all 150 had structural
mismatches. The remaining two were rejected by route evidence: `today_marker_custom_style` uses
the unsupported `stoke` property, and
`upstream_docs_gantt_timeline_with_comments_css_config_in_frontmatter_031` uses unresolved
`themeCSS`. Neither legacy fallback counted as canonical success. The report is
`target/compare/gantt_root_collections_candidate_structure.md`; it exposes axis/tick wrappers,
task/section text shells and explicit paint attributes as remaining work. This structure-only run
does not establish root geometry, browser-visible parity or native raster parity. Temporary
admission was removed after the probe; no comparator or residual policy was relaxed.

Gantt axes now carry their translation in public `ConcatTransform` commands, with domain, tick
paths, text and layer bounds in their local coordinate spaces. The SVG serializer projects the
axis transform onto `g.grid`; an exact semantic/layer/path/text scope projects to one `g.tick`.
Both protocol stack entries remain active, and the projected transform is restored on layer exit.
An edited command sequence that no longer has that exact shape uses the complete generic
serializer, not a legacy renderer. Public transform, opacity and blend mutations remain visible.
The existing direct tests now check both axes' world-space text positions, local stroke/text
bounds, state restoration before tasks and preservation of an inserted tick primitive. The host
integration contract explicitly defines layer bounds in the user space active at `BeginLayer`.
This resolves the redundant tick wrapper but not the remaining inherited-style/text-shell
differences from the full candidate report; Gantt is still not admitted.

The Gantt candidate now also retains D3's axis container defaults (`fill="none"`, 10px
`sans-serif`, middle anchoring), compact tick line spelling, and bottom/top label `dy` behavior
without reapplying external theme CSS. Compact projection is deliberately exact: a changed tick
line start/end or a changed label origin, size, anchor, opacity, blend, or font shape falls back
to the generic command serializer, preserving the edited public command instead of silently
normalizing it back to a source-shaped axis. The focused candidate test covers both the unchanged
source shell and edited line/text colors, opacity, geometry, and paint; this is not complete
family admission evidence.

Source inspection during this projection found a pre-existing nondefault-padding error in both
render paths: Mermaid's `makeGrid` places the bottom axis at `height - 50`, not
`height - topPadding`. The layout type now owns that fixed-inset calculation for both emitters.
The existing SVG config and public-axis tests use `topPadding: 70` and verify the corrected bottom
axis while retaining the top axis at 70. Both tests failed before the correction (a 20px bottom
axis displacement); default-padding fixture geometry is unchanged. This is a source-backed
geometry correction, not a comparator normalization or browser measurement residual.

Gantt section labels now retain every explicit `<br>` line, including empty trailing lines,
instead of using the FlowDB-oriented text normalizer that trims them. The public command stream
records each line's central baseline and resolves the source's cross-tspan XML whitespace before
the bounded text builder allocates it. Empty spans do not advance the source's text cursor;
leading empty spans also affect which `dy` reaches the first addressable character. Lines with
no addressable text have empty payloads and zero bounds, while semantic titles retain the source.
The candidate serializer combines only compatible, opaque, host-shaped lines whose public
positions and normalized text reproduce that cursor. It uses public runs rather than layout
coordinates or source labels; edited sequences retain the generic canonical serializer.
Both projections preserve already-resolved spaces, including spaces owned by an adjacent line.

The section regression covers leading/intermediate/trailing empty lines, cross-line spaces and
tabs, and a public line-position edit that prevents compaction. All 31 focused renderer/Gantt SVG/
effect tests and two facade Gantt tests pass. A local Chromium probe compared per-character visible
extents for the four source-shaped cases against both grouped and independent text projections; all matched. This
does not establish a full Mermaid browser comparison or native font parity. No family admission,
fixture comparator or accepted residual changed, and the earlier full-family report still records
its original snapshot rather than a current pass count.

Milestones now retain untransformed rounded-rectangle resources inside a public save/transform/
restore scope. This replaces the separate path-segment rewriting code and preserves the source's
45-degree rotation and 0.8 scale for both geometry and stroke (a two-unit stroke becomes 1.6 in
world space). The transform origin follows the task row even for combined `milestone, vert`
tags, where it is not the tall rectangle's center. The candidate SVG projects the same resource
as a transformed rectangle without replaying milestone CSS. A direct document-to-SVG regression
checks both tag combinations, the fixed origin, stroke scale, untransformed labels/next tasks,
and public transform edits. The focused 32 renderer/Gantt/effect tests, two facade tests and
renderer Clippy check pass. Gantt remains outside canonical admission pending the remaining
full-family DOM and presentation work.

Single-primitive task scopes now project their public semantic identity, accessible name and
description onto the actual rectangle, path or text instead of adding a synthetic group. Linked
bars and labels keep separate anchors so every bar still precedes every label. The protocol scope
remains on the serializer stack; hidden nodes, unnamed labels and edited scopes containing extra
commands retain the generic semantic group. Regression coverage checks escaped public metadata,
milestone geometry edits, links, painter order and those generic fallbacks. All 86 focused family,
document serializer, Gantt SVG and effect tests pass. This does not establish complete upstream
link DOM or accessibility-tree parity, change comparator rules, or admit Gantt as canonical.

The next source-backed paint correction resolves numbered section background classes after the
generic `section` token instead of accidentally falling back to `textColor`. Task label colors
now follow the exact built-in CSS importance, specificity and declaration order: clickable labels
override active/done inside colors, done-outside labels override clickable colors, and
`activeCritText` overrides `vertText` only after the more-specific rules. Vertical marker labels
also retain the final middle anchor when layout classifies their text as outside. An eight-case
regression checks public text paint, weight and anchor, distinct section fills, and direct candidate
SVG output. The active-clickable case failed before the fix; this remains focused evidence, not
full-family canonical admission.

The full candidate comparison at `51ff7fda0` was refreshed with temporary Gantt canonical
route/coverage admission, which was removed afterward. It selected 157 fixtures: 150 canonical
renders all still had structure differences, two unsupported effects failed the route check
(`today_marker_custom_style`'s `stoke` property and the documented `themeCSS` fixture), and five
existing baseline skips remained. The report is
`target/compare/gantt_51ff7fda0_candidate_structure.md`. This snapshot does not include the corner
fix below. Remaining differences include explicit paint/semantic attributes versus inherited
source defaults, axis/title/today structure, a literal `#;` text rewrite during SVG escaping, and
six text-measurement threshold crossings that switch labels between inside and outside placement.
The last category is not addressed by forcing label positions or changing comparator rules.

Narrow task bars and vertical markers now clamp horizontal and vertical corner radii independently,
as SVG rectangles do. For example, a three-unit-wide bar keeps radii `(1.5, 3)`, not `(1.5, 1.5)`.
The public arc resources own both radii; the SVG serializer recognizes the same elliptical corners
and emits a rectangle only when all four corners agree. An independently edited corner remains a
path. Existing circular-radius callers retain their prior geometry through the shared primitive.
Regression coverage exercises narrow bars, tall vertical markers and a short bar height, including
the saturated-radius floating-point boundary; all 88 focused family, document, Gantt and effect
tests pass. Gantt remains outside canonical admission.

Canonical text serialization now XML-escapes public strings without decoding them again. Packet,
Pie, Sankey, Cynefin and the Gantt candidate resolve Mermaid preprocessing placeholders in their
source adapters before emitting public text and accessibility metadata. Literal `#;`, ordinary
`&quot;` and the final `#quot;` produced by `#35;quot;` retain their intended spelling; public document
edits are never reinterpreted as Mermaid source. Error and Info emit fixed text, not authored labels.
The shared source resolver visits fragments with cancellation checkpoints and preflights resolved
text bytes before allocating its temporary string. Metadata uses its separate admission path,
not the visible-text quota. Gantt's pre-collapse section strings use operation work admission;
only their final normalized fragments consume text quota. An all-space entity section is tested
at the exact final document text length. This does not claim a bound on all adapter metadata
allocations.

The source-to-document-to-SVG regression checks those five families directly, and a Gantt mutation
test verifies literal public text and accessible descriptions. Existing legacy renderers retain
their separate source escaping helper. This addresses the `#;` mismatch in the historical report;
it neither refreshes that full-family report nor admits Gantt or the remaining bridged families.

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

The subsequent candidate run in `target/compare/venn_canonical_progress_structure.md` and
`target/compare/venn_canonical_progress_parity_root.md` rendered all 12 fixtures through the
canonical route without fallback. Seven classic fixtures pass both modes. The five failures are
the three hand-drawn fixtures and two additional text-node fixtures: rough stroke color tokens
use RGBA rather than the source's HSL/hex spelling, while text areas still differ in their
`font-size` attributes and direct `foreignObject` child structure. Neither mode passes overall.
Temporary route/inventory admission was withdrawn after the diagnostic.

The color-equivalence follow-up (`target/compare/venn_canonical_color_parity_root.md`) also rendered
all 12 fixtures canonically. Nine pass parity-root; only the three fixtures with text nodes still
fail on text-area font attributes and `foreignObject` ownership. Venn rough-path static strokes
now compare by resolved 8-bit RGB and alpha to twelve decimal places, removing only HSL/hex/RGBA
spelling and color-conversion roundoff. Changed RGB channels and opacity differences below one
8-bit alpha step remain detectable; dynamic/invalid paint and strict mode are unchanged.
This is not a geometry or text residual waiver. The temporary route/inventory admission was again
withdrawn, and the full-family gate remains unsuccessful.

The native-text ownership follow-up (`target/compare/venn_canonical_native_text_parity_root.md`)
again renders all 12 fixtures canonically, with nine passing. The text-area font attributes and
direct `foreignObject` children now agree with the source. The three text-node fixtures still
fail because the source span has direct text, whereas the candidate span contains the positioned
SVG projection. This remaining structural difference is not an accepted normalization; temporary
route/inventory admission was withdrawn after the diagnostic.

The earlier classic circle/title mismatches are resolved without comparator changes. Source-shaped
`M/m/a/a` paths are emitted only when the relative offsets reconstruct the public coordinates
exactly. Otherwise the serializer retains absolute commands. Title CSS comes from the public
`TextStyle`; the scaled presentation attribute remains, while the author rule preserves the
actual 32px default size. Public edits to font size, position, paint, and stroke caps still reach
SVG. No external config or private geometry is reinterpreted to reconstruct those values.

This is not full-family admission: Venn remains on the explicit legacy SVG route. Remaining
text-node DOM differences have not been accepted as residuals or hidden by normalization, and
must be resolved before full-family admission.

Venn's hand-drawn circle and intersection generators now stream typed Rough.js operations from
`venn/rough.rs` directly into the DrawingList builder. Legacy SVG wrappers collect and format the
same operations. Hachure/crosshatch
now use fallible operation production, with work admission for vertices, edge tables, and every
scan row (including empty rows). Both crosshatch passes share the operation meter; an error drops
the local output instead of returning a partial fill. Rotation occurs in place and lines are
consumed individually, without the former complete line array or polygon copies. Fixed-size
per-line Rough.js operation batches still use the existing allocator, and sorting checkpoints
bound admitted input size rather than guaranteeing fixed-latency cancellation inside sorting.

Ellipse sampling now uses a four-point rolling curve window. Work admission precedes each
sample; outline operations are emitted during sampling instead of after a complete point array.
The first outline's fill-estimation points grow fallibly, while the second stroke does not retain
an unused polygon. Rejecting the second operation of a high-resolution ellipse stops after four
points, and a stalled or underflowed angular step returns an error rather than looping forever.
Venn circle generation uses this path and shares the same operation meter with hachure filling.

Intersection curve subdivision and Douglas-Peucker simplification now use fallibly growing
explicit stacks. Work is admitted during subdivision and every simplification scan, without the
legacy recursive full-result clones. Normalized paths are sampled once and reused for outline
generation. The outline is consumed one source segment at a time solely to preserve its random
draws; no complete discarded outline is retained. Tests compare exact sampled points, fill
operations, and seeded outline RNG progression with the collecting implementation.

Source segments are admitted and collected fallibly. Absolute-coordinate conversion now advances
lazily, and normalized output is admitted before each retained segment. Both compatibility and
bounded entry points use the same normalization formulas; complete absolute/intermediate arrays
are gone from the bounded path. The existing per-arc cubic conversion scratch has constant size
and retains its allocator behavior. Venn uses this bounded entry and drops its source array before
sampling.

The public hand-drawn DrawingList gate is now open for the source-backed vector implementation.
The builder creates the outline resource under its path/resource budget before painting, then
emits fill and outline references in source painter order. Both painted and deferred resources use
one transactional segment collector. Empty hachure output leaves no path resource or paint command;
unstyled intersections retain their unpainted source geometry and semantic group.

Source hand-drawn paint differs from classic styles: circles use `transparentize(fill, 0.7)` and
styled intersections use `transparentize(fill, 0.3)`, without applying classic `fill-opacity` or
`stroke-opacity` a second time. Exact alpha lives in command state rather than quantized color
alpha. Direct-vs-legacy SVG tests compare every rough path, color, width, opacity, and painter
order for seeded and fallback-random cases. Public tests reject the first path segment over the
caller limit, and cancellation is armed after layout to exercise document construction.
The candidate SVG serializer now projects nonempty rough paint scopes into the source's nested
`g/path` structure directly from public resources. It streams round-trippable Rough.js coordinate
spelling without rebuilding operation sets, preserves fill-before-outline order, and combines the
stroke color's alpha with the public scope opacity once. Direct serializer evidence compares circle
and custom-intersection path bytes and paint against the existing SVG, then edits public paint to
prove those edits reach SVG. Noncompact edited strokes retain the general canonical projection.
Empty hand-drawn intersections and full text-node DOM parity still need family-level evidence.
This vector/projection work does not admit Venn's canonical SVG serializer.
The new roughr API also needs its independently versioned package release before a Merman
release can resolve this implementation from the registry.

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
not behavior-equivalent substitutes.

[ADR-0088](adr/0088-atomic-normal-line-metrics.md) defines atomic line-metrics measurement and its
append-only native callback evolution. The direct adapter now resolves normal text through that
operation: HTML ASCII whitespace is collapsed, NBSP and literal markup are preserved, Unicode
soft breaks and min-content width protect indivisible words, and each finalized line receives its
own paired line height and baseline. Candidate widths use complete text runs, not summed token
widths. Both candidate probes and final metric probes charge operation work before host calls.
Normalized scratch bytes and line records are admitted before allocation; exact output quotas
are checked using finalized lines, so discarded wrap spaces do not cause false quota rejections.

Venn text nodes use their own theme/default or node-ID color override, follow the existing text-area
grid, and retain interleaved debug circle/cell/text order. Discretionary hyphens and Unicode hard
line separators remain explicit unsupported effects pending break-glyph projection, rather than
being silently treated as ordinary soft breaks. Focused public and builder tests exercise literal
markup, NBSP, CJK, long-word overflow, content-specific baseline pairs, exact budgets, and mid-layout
cancellation. Venn SVG still uses its recorded legacy route until full-family parity is verified.

The candidate SVG now projects each text node's `foreignObject.venn-text-node-fo` and XHTML
`span.venn-text-node` from a public, unpainted container rectangle. Inside that shell, positioned SVG
text uses the same canonical origins, baselines, fonts, paint, and literal strings as DrawingList.
The inner SVG has no viewBox or clipping; its translation cancels only the container origin.
There is one native text group, not an adjacent duplicate branch. The internal marker
`data-merman-native-text="v1"` declares the complete native projection in the foreignObject
parent's coordinates. Readable processing skips guessed HTML overlays for that group; resvg-safe
processing promotes it out of the browser-only positioning shells without remeasuring or
regenerating its content. Public title/description use ARIA attributes on the group so metadata
does not pollute the span's text content. The stable semantic ID and debug visibility survive
promotion, including an empty text node. Ordinary unmarked HTML retains its existing fallback.
Projection discovery and group matching use the existing markup scanner, so comments and CDATA
cannot create or prematurely close a native projection.
Text-area `font-size` attributes are derived from agreeing public text styles; a mixed-size area
omits that shared attribute instead of overriding individual runs.

This is an explicit structural change from upstream span-owned direct text: the identity shell
contains an inner SVG whose native group retains metadata and headless output. It is
not byte-identical upstream DOM and has not been hidden by comparator normalization. Candidate
mutation tests cover changed baselines, text, font size, paint, container coordinates, metadata, and a box
that becomes painted (which correctly falls back to generic canonical commands, not legacy).
Chromium characterization of the new promotion path on the two-label docs fixture found identical
text, font, and fill. The maximum bounding-box delta was 0.007568359375px; the linear transform was
unchanged and the translation differed by up to 0.00757598876953125px. A trial 1e-6px equality
assertion therefore failed. This is a recorded browser-coordinate residual, not a production
offset or a claim of exact screen-coordinate equivalence.

A separate source-versus-candidate Chromium probe covers all 19 text nodes in the three failing
fixtures (`upstream_docs_venn_text_nodes_and_style_003`,
`upstream_cypress_venn_handdrawn_custom_styles_018`, and
`upstream_cypress_venn_spec_13_should_render_a_complex_venn_with_labels_text_nodes_and_style_013`).
All are single-line in that browser. Literal text, computed font/paint, and HTML container boxes
agree, but the deterministic candidate's text top is about 1.5px higher for the default font stack
and 0.5px higher for Courier. This is distinct from the promotion-only coordinate residual above.
The built-in 24/18 line-height/baseline pair at 20px is a declared em-box heuristic; the browser
provider returns 23/19 for these default-font labels and 23/18 for these Courier labels.

As a controlled diagnostic, replacing only the candidate text baselines using the existing Web
browser measurement session and the same centered-line formula reduces the observed source Range
rectangle differences to at most 0.015533447265625px vertically and 0.013763427734375px horizontally.
This is DOM reprojection evidence, not an end-to-end WASM run or a portable font constant. The
Rust family regression separately builds Venn using two supplied normal-line pairs and verifies
their exact public origins and line boxes, their SVG projection, and zero additional host calls
during both browser SVG serialization and native promotion. Neither check admits the remaining
span child-structure difference. Restoring automatic HTML wrapping would violate ADR-0088 rather
than resolve that structural contract: SVG must retain the already-resolved public origins.

Quoted whitespace labels also preserve Mermaid's truthiness-before-normalization order: `[" "]`
normalizes to empty paint while retaining its semantic anchor, whereas `[""]` is absent and uses
the node ID.

## Exercised effect accounting

`fixtures/drawing-list/v1/effect-coverage.json` is the focused effect evidence used by
`drawing_list_effect_accounting`.  It covers the visual constructs currently exercised by admitted
fixtures: portable path paint, host text, gradients, clips, semantic links, opacity, blend modes,
transforms, expanded marker geometry, and Venn hand-drawn paths, plus explicit fail-closed outcomes
for browser-wrapped text, filters, and external icon registry content. Each row is executed
against the typed renderer and must produce its declared vector or structured-error disposition;
no row may be an unclassified best effort.  The test also collects the effect kinds actually
observed across successful fixtures and fails when one has no corresponding assertion, so an
adapter cannot start emitting an already-modeled visual command without extending the evidence.

The matrix intentionally does not claim that every protocol resource kind is emitted by a current
family.  Patterns, inline images, glyph/outline text, and raster subtrees remain protocol-level
capabilities with dedicated display-list validation fixtures until a family admits them through a
source-backed renderer slice.
