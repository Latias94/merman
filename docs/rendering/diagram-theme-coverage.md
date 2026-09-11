# Diagram Theme Coverage

This is a migration snapshot, not a stable support contract. It distinguishes a direct typed
consumer, an executable legacy compatibility route, and an explicitly Unsupported mechanism.
A compiled theme is opt-in; the removed `PresentationTheme`/`HostTheme` APIs are not aliases.

## Snapshot: 2026-09-11

The source inventories, not this summary, authorize rendering and retirement:

| Inventory | Snapshot | Source |
| --- | --- | --- |
| Families with at least one direct typed surface | 33/33 | `crates/merman-render/src/diagram_theme/family_mechanism_matrix.rs` |
| Families without a family-owned Legacy route | 26/33 | `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs` |
| Families with executable Legacy routes | 7/33; 162 routes | Same bridge inventory; exact matrix and dispatch reconciliation |
| Base FontStack | 32 Typed / 0 Legacy / 1 Unsupported | Mechanism matrix |
| Base FontSize | 18 Typed / 0 Legacy / 15 Unsupported | Mechanism matrix |
| Public support claims | Revision 67 | `crates/merman-render/src/diagram_theme/support_manifest.rs` |
| KTD17 scalar cutover | v69; 372 routes / 514 route-profile witnesses | `crates/merman-theme-acceptance/src/cutover_manifest.rs` and `cutover.rs` |
| KTD23 historical retirement | v5; 66 routes / 132 value probes | `crates/merman-theme-acceptance/src/route_retirement_manifest.rs` |

KTD23 v5 retires Sequence's four static unqualified/Default `Text.fill` and `Title.fill`
identities against baseline `096a8f7f3`. Both projected only `themeVariables.titleColor`, which
neither the pinned Mermaid 11.17.2 stylesheet nor the renderer consumes. The eight before/after
value probes authorize deletion without claiming a new typed text consumer. Three-look rendered
SVG comparisons preserve the actual title, actor, group, message, control, and note paint.

Each KTD17 route-profile witness binds SVG and PNG evidence; it is not a count of individual
output files. Version 69 adds two Radar Title.fill Default routes. Version 68 adds four Flowchart and four Swimlane Title.fill routes with Classic,
Neo, and HandDrawn witnesses for cluster and swimlane titles. Their typed classification suppresses
only those winners in the shared bridge; Class, Block, and generic Text fallback still retain their
existing title-color projections. Version 67 adds four Requirement Text.fill routes (unqualified/Default,
solid/transparent) with Classic, Neo, and HandDrawn witnesses. Separate native PNG pairs keep one
text-color owner fixed while proving the other node or SVG relation-label consumer. Version 66 adds
four Timeline Event.stroke routes (unqualified/Default, solid/transparent). Their Redux witnesses prove the visible activity axis with SVG and native
PNG; family receipts and SVG integration tests cover the shared node/label/line fanout. This does
not qualify shadow-bearing nodes for native export. Version 65 adds four Timeline Text.fill routes
(unqualified/Default, solid/transparent) with Classic, Neo and HandDrawn witnesses for inherited title and referenced
arrowhead fill. Version 63 adds eight Requirement Relation.fill/stroke routes (unqualified/Default,
solid/transparent), with Classic, Neo and HandDrawn witnesses for relation paths and referenced
markers. Explicit `relationColor` retains ownership; any specified stroke suppresses the fill
fallback. Version 62 adds the remaining eight ER scalar routes: unqualified Table.fill,
Default Text.fill, and unqualified/Default Relation.fill. Classic witnesses cover the row fills,
relation stroke channel, and visible text including the diagram title. Version 61 adds four ER
Table.fill routes (Odd/Even, solid/transparent) with Classic witnesses. Family tests also cover Neo/HandDrawn and HTML/SVG labels. Version 60 adds four GitGraph Node.stroke routes with Classic, Neo, and HandDrawn
witnesses covering tag outlines. Version 59 added four GitGraph Node.fill routes with Classic, Neo, and HandDrawn
witnesses covering state glyphs and tag backgrounds. Version 58 added four GitGraph Edge.fill routes with Classic, Neo, and HandDrawn
witnesses for branch-line stroke. Version 57 added eight GitGraph NodeLabel.fill/EdgeLabel.fill routes with Classic,
Neo, and HandDrawn witnesses covering visible tag and commit labels. Version 56 added four
GitGraph Text.fill routes with the same looks covering visible title, commit, and tag terminals. Version 55 added four Kanban Text.fill
routes with the same three looks. Version 54 added four Journey Text.fill routes with Classic
witnesses. The four C4 Text.fill routes retain Classic, Neo, and HandDrawn witnesses. Gantt Warning today and vertical
strokes retain separate profiles.
The today profile fixes the runtime clock to 2024-01-03 within the task interval; `todayMarker`
is a style directive, not a clock override. Neither terminal can supply the other's pixel evidence.
Neither KTD17 nor KTD23 is a percentage of all theme capabilities. Their selector/value domains
differ from the live bridge inventory and must not be added together to calculate migration progress.

ER Table.fill now has separate direct Odd and Even row consumers. Explicit `rowOdd` and
`rowEven` configuration owners suppress only their matching row route. Entity source fill owns
the even row path; odd row fill remains independent. Missing matching rows are NotApplicable.
Unqualified Table.fill directly consumes both row variants and retains both historical replacement
obligations. Unqualified/Default Relation.fill follows stroke-first fallback and uses the native
stroke channel. Unqualified/Default Text.fill covers visible labels, the diagram title, and SVG
subgraph labels when their color falls back from `titleColor` to `textColor`. Explicit configuration
and source styles retain ownership of each consumer. HTML labels consume `nodeTextColor`; SVG
labels and the diagram title consume `textColor`. Ownership is captured before theme-default
projection, so derived colors cannot become explicit owners. Title.fill remains Unsupported.
ER no longer requires its family bridge.

Radar static-unqualified and explicit `Default` `Title.fill` now share one direct typed writer and KTD17 SVG/PNG
authorization for solid and transparent values.

## Family Boundaries

`Typed` below describes a base-typography classification, not unconditional application. Explicit
source/config ownership, absent visible terminals, unsupported sibling properties, and output
qualification still affect the execution evidence. Role-local sizes remain independent where the
family writer owns them. A bridge-free family may deliberately support only a narrow typed surface.

| Family | Base FontStack | Base FontSize | Family-owned legacy bridge |
| --- | --- | --- | --- |
| State | Typed | Typed | None |
| Flowchart | Typed | Typed | Required |
| Swimlane | Typed | Typed | Required |
| Sequence | Typed | Typed | None |
| Class | Typed | Typed | Required |
| Block | Typed | Typed | Required |
| Mindmap | Typed | Unsupported | None |
| Tree View | Typed | Unsupported | None |
| GitGraph | Typed | Typed | None |
| Gantt | Typed | Unsupported | None |
| Kanban | Typed | Typed | None |
| Requirement | Typed | Typed | None |
| ER | Typed | Typed | None |
| Pie | Typed | Unsupported | None |
| XY Chart | Typed | Unsupported | Required |
| Radar | Typed | Typed | Required |
| Quadrant Chart | Typed | Unsupported | Required |
| Timeline | Typed | Typed | None |
| Journey | Typed | Typed | None |
| Architecture | Typed | Typed | None |
| C4 | Typed | Typed | None |
| Treemap | Typed | Unsupported | None |
| Packet | Typed | Unsupported | None |
| Sankey | Typed | Unsupported | None |
| Railroad | Typed | Typed | None |
| Info | Typed | Unsupported | None |
| Error | Typed | Unsupported | None |
| Cynefin | Typed | Unsupported | None |
| Wardley | Typed | Unsupported | None |
| Ishikawa | Typed | Typed | None |
| EventModeling | Typed | Typed | None |
| Venn | Typed | Unsupported | None |
| ZenUML | Unsupported | Unsupported | None |

For exact target/facet/selector coverage, inspect the family mechanism matrix and family-local
plans under `crates/merman-render/src/<family>/theme*`. Public discovery is independently declared
in the support manifest and reconciled against the matrix by tests; it does not imply Browser SVG
or native-export qualification for an otherwise unverified output.

Important boundaries that a family-level count cannot express:

- Journey static unqualified/Default Text.fill owns root inherited paint, native/HTML labels,
  actor legends, and generic line strokes. Explicit `themeVariables.textColor` remains the owner;
  `journey.titleColor` owns only the diagram title. Actor/task paints and the mouth's fixed stroke
  remain independent. Even a text-less diagram emits an activity line, so Text.fill is applicable.
  KTD17 replaces `text.fill`. KTD23 v3 independently retires the unused `title.fill` projection:
  `themeVariables.titleColor` styles only nonexistent cluster text. `Title.fill` is Unsupported;
  a visible title with no local color owner leaves a theme residual, while an absent title or
  explicit nonempty `journey.titleColor` is `NotApplicable`. No typed Title consumer is implied.
- GitGraph static unqualified/Default Text.fill owns root inherited paint, tag holes, visible
  titles, and Text-origin tag/commit label colors. KTD17 v56 replaces the inherited `text.fill`,
  `node-label.fill`, and `edge-label.fill` contributions together. KTD17 v57 directly migrates
  the eight static unqualified/Default solid/transparent NodeLabel.fill and EdgeLabel.fill routes,
  replacing only `node-label.fill` and `edge-label.fill`, respectively. Generic Text retains its
  three historical replacement obligations. Explicit `textColor`, `tagLabelColor`, and
  `commitLabelColor` remain independent owners; Text and label rules keep global source-order
  precedence. Color-generated commit text and branch colors retain their existing ownership. Missing inherited consumers are NotApplicable;
  unsupported winning siblings remain residuals. KTD17 v58 replaces Edge.fill's `edge.stroke`
  projection to `commitLineColor` with direct branch-line stroke, limited to static
  unqualified/Default solid and transparent paint. It preserves stroke-first fallback semantics
  and explicit `commitLineColor` ownership. An independent `lineColor` does not suppress this
  fill fallback; the existing direct Stroke route retains its source-selection policy. Hidden
  branches provide no terminal evidence. Unsupported ordinal combinations preserve static
  BestEffort CSS without certifying it as Applied. Arrows remain node-palette surfaces.
  KTD17 v59 migrates static unqualified/Default Node.fill, preserving the independent raw
  `primaryColor`, `mainBkg`, and `tagLabelBackground` owners captured before fallback projection.
  Materialized derived colors do not become explicit owners. The writer records the actual
  state glyph, tag background, and theme-dependent branch/highlight sources; absent consumers
  are NotApplicable. KTD17 v60 replaces the remaining Node.stroke projection. The same node paint
  plan owns `primaryBorderColor`, `nodeBorder`, and `tagLabelBorder` independently, including
  color-generated text/geometry and the gradient fallback where the writer actually consumes it.
  Existing gradient colors are not rederived, and unused CSS/defs do not establish application.
  GitGraph no longer has a family-owned legacy route or bridge dispatch.
  KTD23 gains no historical retirement rows.
- Kanban has no independent diagram-title terminal. KTD23 v4 retires the unused
  `title.fill` projection to `themeVariables.titleColor`; Title rules are Unsupported and
  NotApplicable, including documents with frontmatter titles. Column labels use Text paint and
  card labels use TaskLabel paint. KTD17 v55 replaces the Text compatibility projection and
  removes Kanban's final family bridge. Static unqualified/Default Text.fill updates the root and
  default label colors; explicit textColor, TaskLabel, and source inline colors retain ownership.
  With no inheriting visible labels Text.fill is NotApplicable; unverified source ownership and
  unsupported winning sibling properties remain residuals.
- C4 static unqualified/Default Text.fill owns the root inherited color and optional diagram title.
  Shape, boundary, and relationship labels retain independent colors. Explicit
  `themeVariables.textColor` owns the root; a title-less document with statically owned label colors
  is `NotApplicable`. Source-directed inheritance or browser-dependent colors remain unverified.
  KTD17 replaces `text.fill` and retires its unused `title.fill` fallback; this does not introduce
  a C4 `Title.fill` consumer.
- XY Chart shares a resolved `FontStack` across title/axis/legend measurement, layout, CSS, and
  visible-text receipts. Only base `FontSize` is Unsupported; role-local `xyChart.*` sizes remain.
- Class shares base typography with layout and final CSS; fixed-size cardinality terminals do not
  acquire the base FontSize. Source-owned descendant fonts require their own measurement evidence.
- Treemap shares its FontStack plan with text measurement and final output. Static unqualified and
  Default Text.fill preserve independent label/value config ownership; no participating text
  terminal means `NotApplicable`. Role-local sizes do not become a base FontSize consumer.
- Sequence no longer has a family bridge. Its generic Text.fill and Title.fill remain
  Unsupported: `titleColor` is not a consumer, while ActorLabel, MessageLabel, LoopLabel, and
  NoteLabel retain their independent typed paint. Successful terminal writers supply the text
  and title occurrence domains used to reconcile unsupported requests. A matching winner remains
  residual even when the unused `titleColor` config is explicit; absent, out-of-range, or shadowed
  rules are NotApplicable. This retirement adds no positive typography or text-color support.
- Requirement static unqualified/Default Text.fill directly replaces `requirement.text`.
  Node SVG/HTML labels and HTML relation labels consume `nodeTextColor`; SVG relation labels
  independently consume `relationLabelColor`. Source `color` declarations retain ownership.
  The retired `requirementTextColor` projection only addressed absent `.reqTitle`/`.reqLabel`
  terminals; the actual diagram title is outside this Text.fill surface. Receipts own the three
  active CSS declarations and reconcile prepared label identities with emitted label facts.
  Empty labels and self-loop anchors do not create ordinal occurrences. Matching unsupported
  ordinal rules and unknown dynamic colors remain residuals; overridden rules and absent or
  entirely source-owned consumers are NotApplicable. Requirement no longer has a family bridge.
- Timeline static unqualified/Default Event.fill is in KTD17, independently of its ordinal palette,
  radius, opacity, and typography routes. Static unqualified/Default Text.fill directly owns the
  root inherited paint; explicit textColor retains ownership. The writer records titles,
  referenced arrowheads, and node fill inheritance when the color-scale domain is empty.
  Independently colored node labels retain their owners, and dynamic source colors remain
  residuals. Unused marker definitions do not prove application. Static unqualified/Default
  Event.stroke directly owns Redux's shared `nodeBorder` paint: ordinary shapes, section labels,
  connectors, and the activity axis. Explicit `nodeBorder` retains ownership; Classic has no
  consumer. Redux Color and Neo shapes keep their independent border/gradient colors while
  labels and lines still consume the shared stroke. A title-only Redux diagram retains its
  activity-axis consumer. Writer-owned receipts bind the original CSS declarations to emitted
  terminals; KTD17 v66 separately proves the visible axis in SVG and PNG. Ordinal stroke remains
  Unsupported, with matching winners reported as residuals. Timeline has no family bridge;
  Title.fill remains Unsupported because Timeline has no titleColor consumer.
- Gantt Task[Warning].stroke is typed and in KTD17. Today-marker and vertical-marker source/config
  ownership remains terminal-local; no applicable marker is not evidence of typed application.
- Error-family bridge freedom does not erase unrelated detected-family compatibility from a lenient
  parse fallback. ZenUML's direct Title.fill does not imply generic text or typography support.

## Evidence and Release Boundary

| Stage | Evidence owned by the stage |
| --- | --- |
| Compile | Valid bounded recipe, fingerprint, declared/inferred capabilities |
| Family/document | Applicability, property winners, source/compatibility residuals, emitted terminals, prepared-text evidence |
| Export | Target-specific resource closure and SVG/PNG/JPEG/PDF admission |

An unevaluated stage cannot upgrade the result to portable. The private C6a ledger covers 18
representative Standalone SVG/PNG cells in nine Flowchart/State/Sequence render groups. Its
eligibility receipt requires that exact ledger; scalar cutovers and JPEG/PDF smoke do not add cells.
C6b equal-depth certification remains paused. Browser SVG qualification and the remaining C7a
authoring/consumer/preset gates are not complete. These counts do not establish stable readiness.

Route and raster-paint binding receipts are sealed by the opaque
`FamilyRenderCompletion<ResvgCompatibleSvg>` before the facade separates the output and report.
The former public seal functions accepting a report plus caller-provided artifact digests are
removed. The `internal-theme-acceptance` Cargo feature is also removed. The private harness uses
an explicit workspace cfg through `scripts/run_theme_acceptance.py`; producer Cargo packages
exclude its independent acceptance modules. `scripts/verify_theme_acceptance_boundary.py` checks
package lists and compiles a production consumer whose acceptance imports must fail. These
boundary checks do not qualify presets or close the remaining C7a authoring/consumer gates.

The preset admission runner executes all ten exact catalog recipes against fixed Flowchart,
State, and Sequence sources, then exports each completed document to PNG. On 2026-09-11 the
60 artifact observations comprise 6 HostDependent State outputs from the native candidates and
54 Rejected outputs. The seven retained recipes keep explicit Mermaid `base` and `darkMode`
requests (two compatibility residuals per family). Native candidates omit those requests.
All recipes leave font resources to the host; standalone SVG fonts are not self-contained and
native text retains host font dependencies. Flowchart still reports three bridge residuals and Sequence still reports
one unsupported generic `Text.fill` residual for every recipe. State admission alone does not
prove semantic or visual preset qualification. All catalog qualified scopes remain empty.

`inspect_preset_admission` binds the exact compiled recipe/resource fingerprints, source digests,
shared document/resource identities, target receipts, and actual artifact bytes. The admission
inventory is the first stage of preset qualification; it does not issue
`PresetQualificationReceipt`, assert semantic/visual qualification, or populate `qualified_cells`.
Preserve the seven retained recipes' compatibility behavior and the host's font policy. A test-only
resource profile cannot qualify the unmodified catalog recipe. Do not suppress these residuals to
make qualification pass. The fixed 18-cell C6a ledger remains unchanged.

The 2026-09-11 installed Python wheel consumer now runs shared light/dark definition and spec
oracles through generated UniFFI bindings. It verifies Flowchart/State/Sequence SVG reuse and
isolation, terminal State fill and a family-scoped override, complete-spec cold start, exact native
preset export/render equivalence, and explanatory Unsupported discovery. The existing wheel smoke
invokes this witness in Python CI, preflight, and final-wheel verification. The local run uses the
`python-uniffi-native` profile on aarch64-apple-darwin; other platforms retain their CI owners.
The default Python wheel has no PNG capability. This closes the missing executable non-Rust SVG
consumer witness, not C7a eligibility, contract freeze, or preset qualification.

The 2026-09-11 rebuilt Web/Playground text-surface smoke run passes all 21 cases in Chromium
and WebKit. Firefox passes 20 of 21: the Packet first byte label fails the existing 1px root-bound
containment assertion. A local Firefox probe reports the text element at x=6px against a root at
x=8px, while its DOM Range starts at x=9px; Chromium reports the element at x=9px. This is an open
browser geometry discrepancy, not proof that the glyph is clipped or permission to widen the
comparator tolerance. Keep the assertion and withhold cross-browser qualification until the
terminal geometry/pixel evidence resolves it. The production-mounted State dark Note case passes
in all three browsers after rebuilding the current transport artifacts and correcting the test's
duplicate share-fragment prefix.

Family paint adapters declare which targets require config-default ownership to be captured
after detection and before theme derivation. The shared compiler binds those declarations;
capturing ownership neither applies paint nor certifies a terminal. The legacy bridge separately
maps each family to its executable compiler or an explicit bridge-free classification. Its
dispatch inventory remains independent of the mechanism matrix.

KTD18 ordinal and KTD19 typography decisions remain in the convergence addendum, backed by
family tests and matrix classification. They do not yet constitute an independent combined
bridge-removal authorization. The current route/dispatch inventory is deliberately only an
inventory: it does not observe provider removal or close all release gates.

## Retirement and Verification Rules

- Do not delete the bridge while any executable Legacy route remains. Typed writers must preserve
  the intended behavior; an intentionally removed behavior needs an explicit Unsupported boundary
  and retirement evidence, not an empty overlay silently interpreted as success.
- Removing the final route also requires removing the compiler/parse provider and the historical
  probe dependency, then reconciling KTD17/KTD18/KTD19/KTD23 and support claims. Zero route counts
  alone do not authorize that deletion.
- Keep acceptance authority independent of runtime classification. Do not generate an authorization
  from the same inventory it is meant to check.
- Validate the final semantic terminal, not an arbitrary SVG substring. XML parse errors fail the
  test; unused CSS, defs, metadata, or absent terminals do not prove application.
- Preserve source/config precedence per property and occurrence. Raw theme CSS, custom
  postprocessors, host-dependent measurement, and browser behavior retain explicit evidence grades.
- Keep comparator normalization narrow and non-semantic; document residuals instead of hiding them.

Focused checks live in `merman-render` support-manifest/matrix/bridge unit tests and family SVG
tests, plus `merman-theme-acceptance` cutover and retirement tests. KTD17 library tests require
the `png` feature; running that crate with no default features alone does not exercise them.
Bridge inventory changes also require the `legacy_projection_retirement` integration target,
whose frozen counts and digests are independent of the renderer library tests:

```sh
python3 scripts/run_theme_acceptance.py nextest run --locked -p merman-theme-acceptance \
  --no-default-features --features png,layout-cytoscape \
  --test legacy_projection_retirement --test route_cutover_runtime
```

The semantic baseline is pinned by `docs/adr/0001-upstream-baseline.md` and
`tools/upstreams/REPOS.lock.json`. Config merge and precedence are documented in
`docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` and
`docs/alignment/MERMAID_THEME_STYLE_PRECEDENCE.md`. Historical implementation decisions remain in
`docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md`; this snapshot
supersedes its older progress counts, not its architectural or release requirements.
