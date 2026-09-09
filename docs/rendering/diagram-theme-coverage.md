# Diagram Theme Coverage

This is a migration snapshot, not a stable support contract. It distinguishes a direct typed
consumer, an executable legacy compatibility route, and an explicitly Unsupported mechanism.
A compiled theme is opt-in; the removed `PresentationTheme`/`HostTheme` APIs are not aliases.

## Snapshot: 2026-09-09

The source inventories, not this summary, authorize rendering and retirement:

| Inventory | Snapshot | Source |
| --- | --- | --- |
| Families with at least one direct typed surface | 33/33 | `crates/merman-render/src/diagram_theme/family_mechanism_matrix.rs` |
| Families without a family-owned Legacy route | 20/33 | `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs` |
| Families with executable Legacy routes | 13/33; 246 routes | Same bridge inventory; exact matrix and dispatch reconciliation |
| Base FontStack | 32 Typed / 0 Legacy / 1 Unsupported | Mechanism matrix |
| Base FontSize | 18 Typed / 0 Legacy / 15 Unsupported | Mechanism matrix |
| Public support claims | Revision 50 | `crates/merman-render/src/diagram_theme/support_manifest.rs` |
| KTD17 scalar cutover | v54; 300 routes / 338 route-profile witnesses | `crates/merman-theme-acceptance/src/cutover_manifest.rs` and `cutover.rs` |
| KTD23 historical retirement | v4; 62 routes / 124 value probes | `crates/merman-theme-acceptance/src/route_retirement_manifest.rs` |

Each KTD17 route-profile witness binds SVG and PNG evidence; it is not a count of individual
output files. Version 54 adds four Journey Text.fill routes with Classic witnesses. The four C4
Text.fill routes retain Classic, Neo, and HandDrawn witnesses. Gantt Warning today and vertical
strokes retain separate profiles.
The today profile fixes the runtime clock to 2024-01-03 within the task interval; `todayMarker`
is a style directive, not a clock override. Neither terminal can supply the other's pixel evidence.
Neither KTD17 nor KTD23 is a percentage of all theme capabilities. Their selector/value domains
differ from the live bridge inventory and must not be added together to calculate migration progress.

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
| Sequence | Typed | Typed | Required |
| Class | Typed | Typed | Required |
| Block | Typed | Typed | Required |
| Mindmap | Typed | Unsupported | None |
| Tree View | Typed | Unsupported | None |
| GitGraph | Typed | Typed | Required |
| Gantt | Typed | Unsupported | None |
| Kanban | Typed | Typed | Required |
| Requirement | Typed | Typed | Required |
| ER | Typed | Typed | Required |
| Pie | Typed | Unsupported | None |
| XY Chart | Typed | Unsupported | Required |
| Radar | Typed | Typed | Required |
| Quadrant Chart | Typed | Unsupported | Required |
| Timeline | Typed | Typed | Required |
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
- Kanban has no independent diagram-title terminal. KTD23 v4 retires the unused
  `title.fill` projection to `themeVariables.titleColor`; Title rules are Unsupported and
  NotApplicable, including documents with frontmatter titles. Column labels use Text paint and
  card labels use TaskLabel paint. The Text compatibility projection and Kanban bridge remain.
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
- Timeline static unqualified/Default Event.fill is in KTD17, independently of its ordinal palette,
  radius, opacity, and typography routes. Remaining text/title/stroke compatibility keeps its bridge.
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

The semantic baseline is pinned by `docs/adr/0001-upstream-baseline.md` and
`tools/upstreams/REPOS.lock.json`. Config merge and precedence are documented in
`docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` and
`docs/alignment/MERMAID_THEME_STYLE_PRECEDENCE.md`. Historical implementation decisions remain in
`docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md`; this snapshot
supersedes its older progress counts, not its architectural or release requirements.
