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

### TreeView admission correction, September 9, 2026

The admission in `392c9b9ac` relied on an older report that did not establish canonical
routing. It is withdrawn. Running `compare-tree-view-svgs --check-dom --dom-mode parity-root
--dom-decimals 3` on `bd3dac32e` with TreeView admitted selected 17 fixtures: 12 canonical
outputs had DOM differences and 5 fell back for unsupported registry icons. None passed.
The local evidence is `target/compare/treeView_bd3dac32e_canonical_parity_root.md`.
At `53982cbfb`, all 12 builtin/no-icon canonical outputs pass the full-family DOM/root check;
5 registry-icon inputs still bridge. Evidence: `target/compare/treeView_public_paths_parity_root.md`.
Shared text/path CSS is derived from public styles, not replayed theme CSS. Heterogeneous edits
disable class sharing. Source containers retain edited accessibility/link metadata. Builtin icon
spelling is used only after exact public-path equality; edited geometry, strokes, or nonuniform
transforms keep generic public projection without an implicit private clip.

A three-fixture Chromium probe (`target/compare/treeview-public-paint-probe.json`) confirms text
and stroke styling. Remaining browser differences include RGBA8 alpha quantization (0.15 becomes
38/255) and the existing deterministic text-height residual (17.6px versus the browser's 18px),
which accumulates in row positions. These are not hidden by pixel adjustments or a new tolerance.

Registry assets now have a direct portable primitive lowerer, shared Iconify alias geometry,
explicit viewport clipping, and the source unknown-icon fallback. Filters, resource references,
asset text, nontrivial group opacity, and other unsupported asset effects remain explicit errors.
The unknown-icon projection now derives its nested viewport, rectangle, text, and paint from the
public command stream. Unsupported edits keep generic projection, and generic uses of a shared
clip still receive a definition. All 17 full-family candidate fixtures pass DOM/root comparison
with 17 observed canonical routes and no bridge. Evidence:
`target/compare/treeView_unknown_icon_public_parity_root.md`.

The five fixtures with registry-related names contain seven **unknown-icon fallbacks**, not
successful external pack assets. This corpus does not prove registered-asset SVG DOM parity.
That projection remains unfinished, so TreeView remains bridged; fixture names and focused
adapter tests are not evidence of a broader capability.

Registered assets now project their rectangular public clips into nested SVG viewports. The
source viewBox is only a coordinate representation hint: its implicit matrix is compensated
against the current public transform, including non-square assets and rotated aliases. Invalid
hints or edited nonrectangular clips retain generic projection. Focused tests use a real pack,
compare its coordinate basis with the direct legacy renderer, and mutate the hint and clip.
Nonempty asset primitives also retain their source tags and geometry spelling, but only after
the shared asset geometry decoder exactly matches every current public path segment. Paint,
opacity, and transforms still come from public commands. A real seven-primitive pack test
compares the direct legacy elements and mutates public paint, public geometry, and a malformed
hint; changed geometry keeps generic path projection.

Asset scopes now bind to matching public Save/Restore lifetimes. Source transform spelling is
used only for an exactly matching public command prefix; otherwise those commands are emitted
normally. Moving a public matrix onto a group resets only its local matrix, not leaf paint or
opacity. Group and nonempty primitive IDs use the registry's shared scope/formatting rules;
ID numbering includes empty primitives. Zero-extent/empty source elements retain inert DOM only
when their current public scope still contains no drawing and the geometry hint remains empty;
changing that hint cannot introduce paint, and inserting a draw disables the inert projection.
Alias wrappers use the registry's shared SVG formatting only after exact typed-matrix comparison
with public commands. Their Save/Restore scope is distinct from the viewport compensation.
A real nested pack test compares element hierarchy, IDs, and transforms across diagram IDs and
aliases, then edits public transforms, geometry, and scope hints. Complete registered-asset
presentation parity remains unfinished. These steps do not admit TreeView or establish complete
asset parity.

Registered leaf primitives now retain a fixed mask of inline property names, not source CSS or
paint values. Both original primitives and edited generic paths serialize those declarations from
the current public `PathStyle`/graphics state, including explicit default opacity and disabled
paint. Parent inline declarations do not become child inline declarations. Blend mode shares the
same style attribute. Public paint/geometry edits and stroke deletion cannot replay stale values.
The Chromium probe `target/compare/asset-style-browser-result.txt` confirms that host rules no
longer replace a source-inline green stroke or its two explicit opacity values; neighboring
presentation-only paths remain overridable. The focused test covers all twelve admitted inline
properties, edited geometry/paint, deleted stroke, and blending. This is not an admission receipt.
Group presentation is still flattened, alpha uses the public RGBA8 precision, and inactive `color`
or absent-stroke parameters cannot be recovered from public paint. In particular, a host rule
that adds a stroke to a source `stroke="none"` path can expose a width difference (source 2,
public default 1). These remain explicit registered-asset parity gaps, not silently accepted
browser residuals or a reason to store a second visual model in the sidecar.
All 80 focused TreeView/icon tests, Clippy, formatting, and the required full SVG structure gate
pass for this change. The gate uses the existing admission matrix; it does not certify registered
asset candidate parity.

Direct-child group paint promotion is now conservative and public-state based. A group is promoted
only when its complete direct path set has one current `PathStyle`, no resource paint, no nested
scope promotion, and no non-path drawing, clip, layer, or semantic scope. The source group retains
only fixed presentation/inline property placement; values are emitted from the shared public style.
Leaves omit only properties
actually supplied by that group and not explicitly declared on the leaf. An edited leaf disables
promotion, while matching edits can promote the new public paint. A Chromium receipt
`target/compare/asset-group-paint-browser-result.json` records the earlier default-rendering,
host-CSS, and group-CSS probe. It is not complete group CSS parity evidence: public color alpha
already combines source paint alpha and fill/stroke opacity. These factors cannot be uniquely
recovered, so fill/stroke opacity promotion has been withdrawn and the combined alpha stays on
each leaf. In particular, overriding inherited opacity on a translucent source paint remains an
explicit registered-asset parity gap; it is not an RGBA8 rounding residual. The source opacity
values are not retained as a second visual model. Group opacity remains unsupported and is not
distributed across leaves.
The follow-up adds one family-level shared-paint regression alongside the existing 80 TreeView/icon
tests; all focused tests, Clippy, formatting, and the full SVG structure gate pass.
An additional regression covers translucent fill and stroke with inherited presentation and
inline opacity, ensuring their combined public alpha is never reconstructed as group opacity.

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

The September 9 review follow-up checks half-arrow paths/reference points and marker-start
orientation in Sequence, the first-tspan offset of Architecture edge labels, and Mindmap gradient
fills/divider visibility against the pinned source. Gantt task paint now follows only the numbered
CSS rules that actually exist (0..3), with root-paint inheritance for unmatched categories;
task text collapses SVG whitespace before measurement while preserving authored semantic text.
Metadata decoding work is charged rather than only preflighted, with exact-budget replay coverage.
Required-nullable font, language, semantic, and visual-source fields reject omission in both
the schema and decoder while still accepting explicit null. These are direct-output regressions,
not new canonical SVG admissions or a claim of full-family comparison success.

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

The Gantt candidate also retains D3's axis container defaults (`fill="none"`, 10px `sans-serif`,
middle anchoring), domain `M/V/H/V` paths, tick line spelling, and bottom/top label `dy` behavior.
Simple axis strokes retain `stroke="currentColor"` while inline CSS resolves their color and width
from public paint; axis label fonts similarly come from public text styles, not external theme CSS.
Nonstandard line geometry, translucent/dashed strokes, blend modes, or changed label origins and
font shapes use the generic command serializer rather than silently restoring source defaults.
The existing mutation test covers edited domain geometry/color, blended output with valid XML
attributes, and changed tick geometry, paint and text. The September 9 strict-route probe at
`0a994df63` plus these changes selected 157 fixtures, rendered 150 canonically, retained five skips
and rejected two unsupported effects. All 150 still have other structural differences, but the
previous 2,083 `currentColor` mismatch diagnostics are gone
(`target/compare/gantt_0a994df63_axis_candidate_structure.md`). Temporary admission was removed;
this is not full-family readiness or a comparator normalization change.

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

Section backgrounds now retain the source rectangle attributes and class, with fill alpha and
object opacity independently projected from public commands into inline CSS. Geometry and
transform edits remain effective; nonrectangular, stroked, resource-painted or blended rows keep
the general serializer. The 39 focused Gantt tests pass. A strict canonical probe of
`upstream_examples_gantt_basic_project_timeline_001` at `d1bfa73fa` plus this change preserves all
four rows' source attributes (apart from the resolved style); the whole fixture still fails on
task and label structure. Its report is `target/compare/gantt_d1bfa73fa_rows_candidate_structure.md`.
Temporary route admission was removed; no family or comparator gate changed.

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

The September 9 measurement follow-up aligns the raw task-label probe with the root theme font,
preserves trailing nonbreaking spaces, and uses Mermaid's signed interval width for label overflow.
It does not tune the deterministic glyph profile to match browser widths. The 37 focused Gantt
tests pass, including explicit measurement-input and reversed-date regressions.

A fresh strict-route candidate probe at `e893c1e11` plus the font/whitespace changes selected 157
fixtures: all 150 canonical renders still had structural differences, two unsupported effects
were explicitly rejected, and five existing skips remained. The temporary canonical admission was
removed after the probe. Evidence is in
`target/compare/gantt_e893c1e11_font_candidate_structure.md`; it predates the signed-interval fix.
The later historical report listing only six mismatches must not be used as proof that canonical
Gantt has only six remaining differences. Explicit paint/semantic attributes, source element
attributes, and today-marker structure remain open; no comparator rules were relaxed.

The today-marker follow-up restores the source's exact `today` group class and line-only child
structure. Public accessible title/description are projected as ARIA attributes rather than extra
child elements, and edited metadata remains observable. Linked or debug-hidden semantic scopes
retain the generic projection. All 38 focused Gantt tests pass; this does not resolve the remaining
paint attributes or admit the family.

Task labels now project source-shaped text elements from the public resolved run, without replaying
theme CSS. Inline styles retain public paint, font and anchor edits; IDs, links and ARIA metadata
remain on the existing task scope. Significant spaces use `xml:space="preserve"` for both browser
and native SVG consumers. Unsupported compact cases retain the generic text serializer. The 39
focused Gantt tests pass, including literal text and public position/style edits; this scoped change
does not refresh the full-family structural report or admit Gantt as canonical.

Task rectangles likewise project resolved solid paint into inline CSS, keeping color alpha,
stroke width and public transforms independent. Resource paints, non-default strokes and blend
modes retain the general serializer. The 39 focused Gantt tests pass, including public alpha/width
edits and dashed-stroke fallback. A strict single-fixture canonical probe is recorded in
`target/compare/gantt_b82482b07_task_paint_candidate_structure.md`: task paint attributes converge,
but the fixture still fails on transform-origin, semantic metadata and other element attributes.
Temporary admission was removed; this is neither full-family nor browser-visible parity evidence.
The pinned source emits inert transform-origin values for non-milestone tasks based on the full
date interval and task row, not necessarily the painted rectangle's center. Milestone transforms
are already owned by public matrices; deriving every origin from rectangle bounds would be wrong.

The title and today-marker follow-up reuses those resolved text/paint projections. Title position,
font and literal content remain public, including an empty text command for Mermaid's persistent
empty `titleText` element. The retired title-only helper that skipped empty text was removed.
Compact and generic text output share whitespace preservation, so changing a public title/task
paint to a translucent color cannot collapse its significant spaces during serializer fallback.
`target/compare/gantt_db772032c_chrome_candidate_structure.md` records a strict single-fixture
canonical probe before the empty-title repair: title and marker paint attributes converge, while
transform-origin, semantic metadata and section whitespace attributes still block the fixture.
The public Gantt route remains a legacy bridge; no structural normalization was relaxed.

Collection projection now retains public navigation, descriptions, roles and independent names:
when source-shaped wrappers cannot express those fields, the existing semantic serializer handles
the scope. Axes retain explicitly supplied accessible names and ticks retain their visible-text
names; a root name independent of
the public title is exposed even when the original source lacked `accTitle`. This intentionally
adds observable accessibility attributes rather than dropping semantics to reduce a DOM diff.
Focused regressions exercise ten collection scopes and name-only edits separately; the earlier
strict comparison predates these semantic corrections and is not current admission evidence.

The full canonical-route probe at `f42805334` selected 157 fixtures: 150 rendered canonically and
all 150 still failed strict structure comparison, five retained existing skips, and two produced
explicit unsupported-effect legacy routes (`stoke` and `themeCSS`) rejected by the route gate.
`target/compare/gantt_f42805334_canonical_structure.md` records that historical full structure run;
temporary admission was removed afterward. It does not cover root parity or browser rendering.
Inspection of its raw SVGs exposed a visual defect: a task rounded to zero width became a stroked
closed path, unlike Mermaid's non-rendering zero-width rectangle. Zero-extent task commands now
retain geometry and semantics with no paint; the 41 focused Gantt tests pass after that repair.

Today-marker declarations now follow Mermaid's source order: replace every comma, restore entity
placeholders, then resolve CSS declarations. The existing CSS tokenizer preserves function
boundaries and `!important` priority. Provably invalid paint declarations retain the prior valid
stroke; valid but unsupported paints such as `var(...)` still return a structured error. Temporary
source normalization is charged before allocation and uses the existing placeholder decoder.
Nine source cases agree with Chromium's computed stroke, width and opacity, including invalid
RGB separators, hash-like entities, entity-encoded commas and repeated important declarations.
The 43 focused Gantt tests passed at that stage. A later browser check corrected the initial
empty-task assumption: invalid `NaN` line lengths use zero and do paint (see the empty-marker
evidence below). This historical run did not admit Gantt as canonical.

Source-shaped projection now retains zero-extent, unpainted tasks as rectangles. If a host adds
stroke to the public path, SVG keeps that path so the edit remains drawable rather than disappearing
under SVG's zero-extent rectangle rule. Section text emits `xml:space` only for significant public
whitespace; ordinary labels keep the source default, while edited or translucent fallback text
retains meaningful spaces. The 43 focused Gantt tests cover both projection and edited-document
fallback. The full-family structural report above predates these changes.

The non-visual task `text-height` annotation is captured once in the SVG structural sidecar under
the document's source security policy. Both text projections share that attribute writer and no
longer reread encoder configuration or confuse the annotation with measured text bounds. The
44 focused Gantt tests cover strict/loose source policy, changed encoder policy and both opaque
compact text and translucent generic text. This removes one external configuration dependency;
it does not imply that all serializer configuration dependencies have been removed.

Task and exclude `transform-origin` attributes now use captured source coordinate bases. Both the
legacy emitter and document builder share the complete-date and local-time-of-day calculation;
the SVG serializer conjugates only the public element matrix, so changing a private basis cannot
change its visual transform. Origin and matrix numbers are emitted together without independent
integer snapping. Tests cover shortened tasks, vert's `order = -1`, exclude ranges, identity and
milestone matrices, changed bases and an already-projected clip ancestor. All 45 focused Gantt tests
pass. The shared time scale also replaces duplicate i64 subtraction with the layout's i128 arithmetic.

`target/compare/gantt_e4d251287_origin_candidate_structure.md` records a one-fixture canonical probe
of `upstream_docs_gantt_milestones_009` from the working candidate after `e4d251287`: source origins
match, but strict structure still fails on added semantic/diagnostic attributes and the element
`transform` attribute. Temporary admission was removed. Chromium confirms the two milestone CTMs
within a maximum component/translation delta of `4.83e-5`; the source CSS rotate/scale and SVG
matrix numeric paths have different floating-point precision. This is scoped browser evidence,
not full-family admission or a comparator tolerance change.

The next path projection emits the public conjugated matrix as CSS `matrix()` alongside paint and
blend in one declaration block, following the pinned source's CSS transform representation.
General paths, resource paints and edited dash styles retain their paint attributes. All 45 focused
Gantt tests pass, including combined dash/blend/transform edits and independent origin mapping.
`target/compare/gantt_160088c47_css_matrix_candidate_structure.md` still fails the single milestone
fixture on 45 semantic/diagnostic attributes; the two extra transform attributes are resolved.
Temporary admission was removed. Chromium's two milestone CTMs differ from the pinned source by
at most `9.71e-6`, with identical transform origins. This scoped numeric evidence does not change
comparison tolerances or admit Gantt; full-family structure and semantic projection remain open.

Section accessible names now come from the final public text runs, separated by newlines, rather
than source labels containing HTML break syntax. The bounded builder charges metadata copying
before allocation and does not decode the resolved name a second time. Compact section SVG omits
an extra name only when the current public runs reproduce it exactly; independent literal names
remain `aria-label`, and edited roles, links or descriptions retain the general semantic projection.
Tick groups likewise rely on their visible text only for an identical default name. Regressions
cover empty lines, XML whitespace, NBSP, entities, independent literal names, changed roles, and
exact reported-work replay. This addresses section/tick semantics, not the remaining task, axis or
today projection differences, and does not refresh full-family comparison or admit Gantt.

Task bar and label default names now both come from the final public label run. Their metadata is
created during the label pass without changing all-bars-before-labels paint order, and each owned
name is budgeted before copying. A Gantt-only, single-pass index recognizes unique single-run label
scopes; deleting, duplicating, expanding or changing their roles retains the general semantic
projection. Ordinary native text and unlinked, undescribed bars no longer create duplicate named
images. Independent names stay explicit; described bars and separately clickable bar anchors keep
their names, since a sibling text cannot satisfy those obligations. Source section descriptions
remain exposed rather than being treated as implicit private relationships. This is task-name
projection work; diagnostic attributes, axis/today semantics and full-family admission remain open.

All 47 focused Gantt/metadata tests pass, including independently edited names, unsectioned linked
bars, removed/duplicated/multi-run labels, changed roles, and exact work replay. The scoped canonical
probe `target/compare/gantt_7f87885fa_names_candidate_structure.md` still fails on 19 attributes,
compared with 45 in the earlier CSS-matrix probe (the intervening section/tick fix is also included).
Remaining differences are diagnostic attributes and axis/today semantics. Temporary admission was
removed. Chromium's accessibility tree exposes the same four task StaticText/InlineTextBox names
as the pinned milestone fixture, without duplicate named task images; this is not full-tree or
full-family accessibility parity evidence.

SVG-to-DrawingList association attributes are now explicitly diagnostic. The shared encoder writes
`data-merman-resource`, `data-merman-semantic-id`, `data-merman-bounds`, and
`data-merman-text-obligation` only when `include_drawing_list_metadata` is enabled. Source DOM IDs,
functional attributes, rendering references, links and ARIA are unchanged. Public-document mutation
tests opt in; source-DOM comparisons keep default production output. The new regression compares
all nodes, text and non-diagnostic attributes between both modes, including native IDs, links and
descriptions. The renderer suite ran 1,796 tests: 1,794 passed initially, and the two stale Railroad/
Cynefin assertions passed after source-backed corrections. The full run also required synchronizing
two test measurers with the previously introduced normal-line metrics operation; 27 existing tests
remain skipped. C4's future expanded-marker candidate evidence still needs explicit diagnostics or
source marker reconstruction; default comparison remains fail-closed when marker evidence is absent.

The builder no longer invents fixed English names for unnamed axis/today scopes. Their public IDs,
roles and commands remain; tick names and authored root accessibility are unchanged. The today
source wrapper has no synthetic ID, and it gains ARIA only for an explicit public name or description.
Explicit `"Today"`, description-only edits, links and changed roles remain observable. All 48 focused
Gantt/metadata/diagnostic tests pass. This changes the shared document's default semantics instead
of teaching the serializer to discard nonempty public names.

The full follow-up `target/compare/gantt_3287556d0_candidate_structure.md` selected 157 fixtures:
150 rendered canonically, six passed strict structure and 144 failed; five existing skips and two
explicit unsupported-effect routes (`stoke` and `themeCSS`) remain. The milestone fixture now passes.
Most remaining attribute differences are task section descriptions with their accessibility names,
exclude paint attributes, zero-radius attributes, and a few whitespace/opacity attributes; linked
fixtures also differ in element structure. This records that full structure run, not root or
browser evidence. Temporary admission was removed; Gantt remains a legacy bridge publicly.

The following task-description correction stops synthesizing English `"<section> section"` text
from model membership. Source Gantt has no such task description, and DrawingList does not define
prose as a machine-readable section relationship. Public section drawing, task identity/name/role
and navigation remain. Explicit task descriptions, including that same literal phrase, continue
to project verbatim; serializer code never filters public description values. The now-unused
source-description-only builder helper is removed. Exclude rectangles also reuse the public solid
paint CSS projection; edited translucent paint and non-default dashed strokes retain their values
through compact or general projection without replaying source CSS.

All 49 focused Gantt/metadata/diagnostic tests pass. The full canonical candidate refresh
`target/compare/gantt_19617815f_candidate_structure.md` selected 157 fixtures: 150 rendered, 110 passed
strict structure and 40 failed, with the existing five skips and two explicit unsupported-effect
routes unchanged. This supersedes the prior 6/144 full structure result. Remaining mismatches
include source radius representation, whitespace, today-marker presentation, and static navigation
structure; root/browser comparison is not implied. Temporary admission was removed. The source's
non-sandbox URL navigation uses runtime event listeners absent from exported SVG; preserving static
headless links needs an explicit navigation projection decision and scoped evidence before admission.

Task rectangle radius attributes now retain their source spelling only when per-axis SVG clamping
reproduces the current public path's radius. The comparison reuses the rectangle recognizer's
existing coordinate tolerance for nonzero radii; a public zero radius requires exact zero.
Changed hints cannot add rounding to a straight-corner public path. Zero-extent paths project the
source radius only when both public paints are absent; adding stroke still emits the drawable
path. Regressions cover narrow/short tasks, changed hints, straight corners, individual corner
edits, and zero-width stroke edits. This is equivalent SVG representation, not a second geometry
source. The 49 focused tests pass; the final zero-radius boundary refinement also passes both
radius and zero-width focused regressions. Full-family structure evidence remains the preceding
110/40 report until refreshed.

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

A September 9 refresh at `0202f8730` with temporary canonical route/coverage admission reproduces
the same three text-node structural failures across all 12 fixtures, with no skips or route
fallbacks (`target/compare/venn_0202f8730_canonical_structure.md`). The ordinary legacy-route run
passes all 12 and is not canonical evidence. Temporary admission was removed; the text projection
contract above remains the outstanding decision, not an obsolete report or a paint regression.

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

Gantt primitive opacity now shares the same CSS declaration block as public paint, transform and
blend. Today-marker mutations cover opacity 0, 0.25, 0.5 and 1 without adding a second attribute
or multiplying stroke alpha. ADR-0078 now specifies static anchors for task URLs on both hit
targets; one public semantic-link edit changes only its own href. The focused regression uses the
`click_multiple_ids_href_loose` case and retains all-bars-before-labels order and source IDs.
Chromium confirms that removing just its four anchor wrappers leaves each target's bounds,
computed paint, opacity and font unchanged; no navigation was executed.

The candidate refresh `target/compare/gantt_aacc5c9bf_candidate_structure.md` includes the radius
and opacity changes: 157 selected, 150 rendered, **121 strict-structure passes / 29 failures**,
five existing skips and the same two explicit unsupported-effect route failures. Both today-marker
opacity fixtures now pass structure; Chromium also matches their computed stroke, width,
stroke-opacity and opacity against the pinned SVGs (`gantt_aacc5c9bf_today_browser.json`).
This is not full-family admission: static navigation wrappers remain reported, alongside empty
source marker DOM, title whitespace, and text/geometry differences. The comparison does not
check root viewports. Temporary canonical admission was removed after the command completed.

The subsequent title fix resolves source entities and SVG normal whitespace before measuring the
Gantt title. Its default label name is copied from the final public run without a second entity
pass; authored document accessibility remains independent. Tests cover repeated/encoded spaces,
non-breaking spaces, an empty text command, measured bounds and explicit `accTitle`. All 51 focused
Gantt/metadata tests pass. This fix has not yet been included in another full-family comparison;
the 121/29 report above remains the latest full structure evidence.

Empty Gantt inputs now retain the today-marker semantic scope and paint a finite line at x=0.
The pinned renderer's empty D3 domain writes `x1="NaN" x2="NaN"`; Chromium uses zero for both
invalid lengths and visibly paints the line at the left edge. The earlier assumption that this
was inert DOM was incorrect. `weekday_monday` and `today_marker_semicolon_truncates` have identical
browser pixels after replacing only those attributes with zero; their computed stroke is red,
width 2px (`target/compare/gantt_empty_today_browser.json`). No private visual sidecar is needed:
public path coordinates, paint, and the existing semantic scope drive both outputs. A mutation
test moves the public path to x=37 and verifies the serialized line follows; `todayMarker off`
still omits the scope entirely.

The remaining overflow-label classes reflect a discrete decision sensitive to font measurement.
For `Create tests for renderer`, the source raw bbox is 123.78125px, the deterministic estimate
116.82px, and the task width 121px; the resulting x changes from 992 to 926.5. For
`Implement parser and jison`, the corresponding widths are 135.375px / 122.65px / 129px, and x
changes from 209 to 139.5. The source calls getBBox before applying task classes, using the root
font at 11px; the existing Rust raw-bbox route and placement formulas match this ordering.
These are visible placement/color differences, not ignorable class noise. The focused source
measurement replay injects the browser widths through the existing profile and asserts the
outside class and source x. Production deterministic measurements and thresholds remain intact.

The refresh `target/compare/gantt_6417ed1cd_candidate_structure.md` still reports 121/150 strict
structure passes: empty diagrams now contain the marker, but the comparator distinguishes source
`NaN` attributes from the equivalent finite zero coordinates. It also exposed redundant root
accessibility after title whitespace normalization. The default root name now comes from the
same resolved title; explicit `accTitle` and descriptions still use source metadata resolution,
and public edits retain their independent meaning. The next full report must include that repair.

The first candidate `parity-root` run is recorded separately in
`target/compare/gantt_6417ed1cd_candidate_parity_root.md`: all 150 rendered fixtures have raw
parity differences, with the same five skips and two unsupported-effect route failures. Its
stronger mode observes presentation attributes versus CSS, including tick opacity and inline
resolved text styles that structure mode omits. No blanket normalization or admission was added;
this report extends the outstanding evidence beyond strict structure and checks root viewports.

After the default-name repair, all 53 focused Gantt/metadata tests pass, including both injected
source-width placement cases and the empty marker/public-coordinate mutation. The full comparison
reports above predate that final repair; neither is claimed as final family admission evidence.

Gantt tick layers now retain D3's `opacity="1"` presentation attribute while one inline CSS block
projects the public layer opacity and blend mode. The CSS overrides, rather than multiplies, the
same element's attribute. Title font size is emitted in a diagram-scoped `.titleText` rule only
for a unique eligible public run; unsupported paint/text and multi-run scopes withdraw the rule
and retain independent inline/attribute projection. No theme/config values are consulted.
Browser replay of `duration_units` and `acc_descr_block_multiline` leaves pixels unchanged after
these representation changes (`target/compare/gantt_public_css_projection_browser.json`).

The non-Strict DOM comparator recognizes the pinned empty Gantt Today line's two literal `NaN`
x lengths as used zero values. Recognition requires the direct `svg[gantt] > g.today > line.today`
structure and both x attributes; Strict keeps the source spelling. Other families, shapes and
paint differences remain checked. Non-Strict numeric masking already omits coordinate magnitudes;
this rule is not position evidence and does not replace the public mutation or browser checks.

Venn content, title and area scopes now preserve independently edited public metadata. Content/title
role or descriptive edits use the generic semantic projection when the source shell cannot carry
them; area names and descriptions are emitted on their existing group. Source paint order is
unchanged. This also exposes existing default area descriptions previously dropped by SVG, so the
old 9/12 Venn report is not evidence for the updated metadata surface. Neither family is newly
admitted by these changes.

The updated Gantt/Venn projections pass all 87 focused render tests, including the new Venn
metadata mutations and Gantt title-rule withdrawal. Full-family reports have not yet been
refreshed for this projection change; historical Gantt 121/150 structure and Venn 9/12 results
must not be used as current admission evidence.

The Gantt empty-length normalization test passes its positive, wrong-family/shape, one-invalid-x,
paint-difference and Strict-preservation cases. It adds no new geometry tolerance or CSS parser.

## September 9 candidate refresh at 3c740a7a9

Temporary canonical admission was used only while rendering these reports and was removed after
all four invocations exited. Gantt selected 157 fixtures, rendered 150 canonically, retained five
existing skips, and reported the two known unsupported-effect bridges (`stoke` and unresolved
`themeCSS`). Structure passes **139/150**; parity including the root contract passes **137/150**.
Reports are `target/compare/gantt_3c740a7a9_candidate_structure.md` and
`target/compare/gantt_3c740a7a9_candidate_parity_root.md`. The remaining failures are static
navigation wrappers, raw-bbox-sensitive inside/outside labels, and (in parity) two vertical-label
font-size representations. Neither an ignored bridge nor masked coordinate magnitudes count as
geometric evidence.

Venn renders all 12 fixtures canonically, without skips or bridges, but both structure and
parity-root report 12 failures after the public area metadata repair. Reports are
`target/compare/venn_3c740a7a9_candidate_structure.md` and
`target/compare/venn_3c740a7a9_candidate_parity_root.md`. The extra area ARIA comes from adapter
prose and fallback intersection names absent from the source; the positioned text shell difference
is also still recorded. These are the pre-default-semantics-repair reports, not a new admission.

Venn default area metadata now follows the authored visible label: the builder no longer creates
English `Sets: …; size: …` descriptions or names an unlabelled intersection from its memberships.
The default name is copied from final public text with the existing allocation/work budget;
`color:none` retains a transparent text command and the label identity. Set membership/size remain
typed model/layout data, not an accessible prose API. The serializer omits redundant area ARIA
only when direct public runs represent the current name; nested or independently edited metadata
retains explicit output. An explicitly supplied old generated description is still emitted.

The repaired Venn defaults pass all 88 focused Gantt/Venn render tests. The complete candidate
refresh renders 12/12 without skips or bridges and passes **9/12** in both structure and
parity-root. Reports are `target/compare/venn_3c740a7a9_default_names_structure.md` and
`target/compare/venn_3c740a7a9_default_names_parity_root.md`; they include the working-tree
metadata repair. All additional default area ARIA differences are gone. The three remaining
fixtures are hand-drawn custom styles 018, complex labels/text nodes 013, and docs text nodes 003;
their only reported differences are source span text versus the canonical positioned SVG child.
Temporary Venn admission was removed after both invocations completed. The shell decision and
its cross-consumer evidence remain required before admission.

## Journey candidate status — September 9

Journey has a direct bounded DrawingList adapter and a canonical SVG candidate. Public SVG still
uses the explicit legacy bridge. Full-family admission remains incomplete; focused tests and
browser checks below do not substitute for that gate.

### Public document and SVG projection

- `fo`, `old`, and tspan placement resolve literal markup, whitespace, fonts, colors, line origins,
  and semantic names in the adapter. SVG does not wrap or measure those runs again.
- Root dimensions derive from the public viewport, retaining the source's 25px intrinsic-height
  addition. Explicit accessibility metadata and independently edited names remain visible.
- Command order interleaves sections and tasks. The source section/task/expression groups and
  actor-circle titles project public semantic scopes. Edited roles, links, descriptions, names,
  and diagnostic visibility retain full semantic output.
- The activity marker uses the source `(5,2)` reference in stroke-width units. Only an equivalent
  public line/triangle pair can be folded into a marker. Geometry or compositing edits retain
  separate commands. Circle recognition requires diameter endpoints, not just matching radii.
- Marker and public clips share one root `defs`, with the marker first. Clip paths come from the
  public resource table; no private label geometry is replayed.
- `fo` labels have explicit public rectangular clips. Their XHTML identity shells consume those
  clips and already-positioned text, using visible overflow so no second clipping rule applies.
  The native pipeline promotes the marked SVG group with its clip and removes only the container
  compensation. Edited/non-rectangular clips or graphics state keep the ordinary projection.
- Legends retain `text > tspan`. The source container's fixed `x=40` is inert because the tspan
  explicitly receives the public x coordinate; y, font, paint, language, and state remain public.
- Section/task backgrounds, task/activity lines, faces, eyes, actors, and mouth primitives use inline presentation derived
  from public paint. Resource references and paint alpha remain separate from object opacity.
  Opacity/blend share one CSS block, and each public transform applies once. The encoder never
  reads the original theme or layout paint.
- Neutral mouths project exact public line geometry. Two-arc mouths can use a local basis derived
  from the first arc's public endpoints. Every point is translated by the inverse basis, preserving
  edited radii, flags, and endpoints. Non-default transforms, resource paints, other shapes, or
  non-finite rebasing retain the ordinary absolute path. Rebasing a gradient/pattern without also
  transforming its user-space coordinates would change its appearance, so it is not admitted.

- Title `em`, `%`, and `ex` units inherit the root SVG font size, with theme font size taking
  precedence over the numeric top-level setting; they never inherit the separate task font size.
  The deterministic profile uses half-em for font-dependent `ex` and a 16px host-root baseline
  for `rem`. Public runs contain resolved px, so SVG does not reintroduce CSS unit evaluation.

### Focused verification

The render tests cover independent viewport and semantic edits; marker/circle equivalence;
shared definitions; source/readable/native text and clip mutation/removal; legend positions and
styles; resource paint and translucent stroke with custom dash/cap/join, opacity, blend and
transform; and mouth path equivalence through an independent SVG path parser. The mouth regression
also changes arc flags, radius, and endpoints, then verifies generic output under a public transform
and a user-space gradient. Background coverage checks absent paints and alpha through source/native
SVG, and title coverage varies root/task sizes independently across px/em/%/ex/rem.
The finalized native pipeline normalizes source `1px` widths to the equivalent numeric `1`.

The background/title follow-up passes 27 focused render tests and 6 facade/exact-budget tests.
Background rectangles retain source class/geometry/fill/stroke attributes with complete current
public CSS; absent paints, alpha and stroke width are verified through the native pipeline.
The serializer does not reconstruct the layout fill that source theme CSS overwrote.

### Browser evidence

- `target/compare/journey-fo-overflow-probe.json` confirms pinned `foreignObject` overflow is hidden.
  For a 150×50 box, long content expands the HTML table to about 613px wide or 1140px tall. Default
  and explicit-hidden output match with no outside ink; visible overflow paints outside the box.
  This is why clipping belongs in the public document.
- `target/compare/journey-shell-ctm-probe.json` covers 3 fixtures, 16 shells and 18 text runs.
  Ordinary/font-precedence cases have equal text/clip CTMs before and after native promotion.
  The fractional long-label case differs by at most 0.007184px in CTM and 0.027736px in first-character
  screen position, with matching text/clip displacement and unchanged styles. Its strict 0.001px
  probe threshold fails; this recorded subpixel residual is not a claim of pixel identity.
- `target/compare/journey-paint-browser-probe.json` covers 3 fixtures and 124 circles/lines.
  The primitive-style migration preserves identities, titles, effective paint, stroke, dash,
  opacity/blend, CTM and screen CTM exactly. Face, eye, task-line and actor colors match pinned SVG.
  Source lines inherit a fill while canonical lines have `none`; a line does not paint its fill.

- `target/compare/journey-title-font-browser-probe.json` identifies the title inheritance error:
  the stress fixture has a 24px root and pinned `4ex` computes to 50.2031px, while the old candidate
  incorrectly used 14px task font size and emitted 28px. The repaired deterministic value is 48px.
  Controlled browser experiments confirm `em/%/ex` follow SVG root size whereas `2rem` stays 32px
  for a 16px HTML root. Exact x-height remains a recorded measurement residual.
- `target/compare/journey-mouth-browser-probe.json` covers 3 fixtures and 22 mouths, including all
  three expressions. New primitive types, lengths, and 33 screen-space samples per path match
  pinned SVG exactly; all pre/post paint and compositing properties are unchanged. Two old/new
  arc comparisons exceed the strict 0.001 probe threshold (maximum screen delta 0.008976px and
  length delta 0.024235), while the new paths match pinned values. This is recorded browser arc
  arithmetic convergence, not a claim of zero pre/post numeric difference. Solid-paint sampling
  does not replace the resource-paint regression that keeps gradients in absolute coordinates.

- `target/compare/journey-background-browser-probe.json` covers 3 fixtures and 26 rectangles.
  Paint/stroke/alpha/state, CTMs, and local/screen bounds are unchanged by the CSS projection.
  The repaired default title is 32px (pinned 33.4688px) and stress title is 48px (pinned 50.2031px),
  both previously 28px. Font families are unchanged; the remaining difference is the documented
  deterministic `ex` approximation, not task-font inheritance.

### Full-family gate and remaining work

The latest full reports are `target/compare/journey_5e2265766_background_title_structure.md` and
`target/compare/journey_5e2265766_background_title_parity_root.md`: 26 selected, 25 canonical outputs,
no skips, **0/25 passes** in both modes. The fixture `upstream_cypress_journey_spec_should_correctly_render_the_user_journey_diagram_title_with_the_011`
contains a trailing `size: 2rem` task line. Pinned Mermaid emits a literal `size` task whose face,
eyes, and neutral mouth coordinates are `NaN`; the source SVG is not finite geometry. The public
DrawingList protocol rejects non-finite points, so the adapter records `DrawingListUnavailable`
with `Journey score produced non-finite face geometry` and uses the explicit legacy bridge. This
is a structured safety disposition, not a candidate to admit by serializing NaN. The reports still expose background attributes, added public clips,
text shells, and color/font spellings. They are failure evidence, not admission.
Temporary runtime and fixture admission were removed after the processes exited. No comparator
normalization was relaxed. These reports include the background presentation and title-inheritance
repairs. They do not establish exact x-height/font rendering or approve the remaining DOM residuals.

### Gantt vertical label correction, September 9, 2026

`9b916eca2` incorrectly treated the source `font-size="11"` presentation attribute as the
resolved font size. Pinned `gantt/styles.js` sets `.vertText { font-size: 15px; }`, overriding
that attribute. Chromium computed style for both labels in
`upstream_docs_gantt_vertical_markers_011.svg` is `15px`. The public text run therefore retains
15px; matching the inactive attribute cannot justify changing its visible size.
