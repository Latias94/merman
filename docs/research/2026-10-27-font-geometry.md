# Pie, GitGraph and Timeline font geometry

Date: 2026-10-27. Maintenance PR: #247. Development counterpart: #246.
This implements three findings from the [text audit](2026-10-27-text-audit.md).
The pinned sources remain Mermaid 12.2.0 on main and 12.1.0 on maintenance.

## Contract and implementation

The stylesheet and the text probe must agree on the font at the measured DOM
element. Intermediate probes retain their own upstream semantics; a final face
must not be applied retroactively to an earlier wrapping or placement stage.

- Pie previously measured legends at 17px and titles at 25px regardless of their
  configured CSS sizes. Its private typography binding now resolves the same
  authored size tokens used by the stylesheet. It supports px, em, percentages,
  zero, inherit and unset against the configured parent size. Nonzero unitless
  sizes are invalid CSS. Unsupported host-dependent values retain their CSS and
  use the inherited size as an approximation. Client-rect measurement remains
  distinct from SVG bbox measurement. Zero sizes do not acquire a phantom width
  from a deterministic profile's minimum font size.
- GitGraph neo/redux commit labels now measure the role weight emitted by
  `.commit-label`. Redux branch prelayout remains normal, while the final bbox
  uses the weight of its numbered branch class. That weight applies only when
  the corresponding rule exists under `THEME_COLOR_LIMIT`; the visible palette
  still follows the upstream eight-slot ring. Neo branch labels, tags and titles
  retain their separate styles. The maintenance adaptation also binds layout and
  final probes to the stylesheet's font-family selection.
- Timeline's virtual height probes remain normal. Actual Redux section, task
  and event nodes acquire the section weight before wrapping, height and bbox
  measurement. Actual task heights carry forward within a task group and reset
  at the next section, matching the renderer's call order. Horizontal connectors
  use the updated height.

Raw CSS remains separate from its resolved measurement value. For example,
`bold !important` is emitted unchanged but measured as weight 700. Inherited
values retain the parent face. Dynamic and unknown weights remain available to
capable hosts. Main does not certify typed font-stack or font-size application
when visible role text still has an unresolved weight, including a graph with
hidden commit labels and visible branches. Maintenance reuses its existing font
parser and does not import main's theme evidence architecture.

## Evidence

Before repair, a Pie configuration requesting a 40px legend still reached its
probe as 17px, and GitGraph commit labels reached a weight-sensitive probe
without their emitted weight. Independent review also found the missing
numbered-rule case: with `THEME_COLOR_LIMIT: 1`, branch `side` has no
`.branch-label1` weight rule. Its regression test failed before the rule-aware
selection and passes after repair.

The new tests use size- and weight-sensitive measurement profiles. GitGraph
checks both host-routed operations and ordinary `TextMeasurer` implementations,
final background geometry, normal prelayout, CSS family precedence, priority
parsing and hidden-commit theme evidence. Timeline checks virtual versus final
phases, rendered rows, group-local height propagation and connector positions.

Real Chromium renders of both pinned Mermaid runtimes confirm Pie's configured
60px title and 40px legend, and Timeline's normal virtual probes followed by
Redux 600/900 final faces. GitGraph's ordering is traced to the pinned renderer
source and tested through Rust measurement adapters; this is not a separate
browser rendering qualification for every GitGraph configuration.

## Maintenance adaptation and validation

Mermaid 12.1 assigns Timeline section classes on a fixed 12-slot ring even when
`THEME_COLOR_LIMIT` emits fewer CSS rules. The private `TimelineNodeTypography`
selects the inherited or section face for the actual emitted class, shared by
node wrapping and final bounds. Changing the class ring to main's newer rule
would alter the pinned maintenance behavior. A regression test reproduces the
old weighted wrapping of an unstyled section before repair and covers LR/TD,
limits 1/2, named sections and orphan tasks after repair.

- 77 affected-family library and integration tests pass with
  `all-diagrams,layout-elk,layout-cytoscape,math`.
- All three layout snapshot contracts, ten xtask root contracts and strict
  renderer-library Clippy pass.
- The full 37-family release comparison passes in structure, parity and
  parity-root modes at three decimals with the existing exact diagnostic policy.

All 17 maintenance root-viewport oracle contracts also pass in real Chromium.

The macOS corpus audit includes 69 Pie, 258 GitGraph and 93 Timeline fixtures,
with no blocking result in these families. Among the 3,720 live maintenance
inputs, `flowchart/stress_flowchart_title_padding_subgraph_029` still fails its
environment-bound filtered-title receipt. Both its local and upstream SVG
SHA-256 values exactly match the pre-existing catalog entry; this batch changes
neither SVG nor that receipt. The darwin-arm64 audit fingerprint differs, so the
macOS whole-corpus command is not recorded as passing. The unchanged Linux owner
must validate its own accepted evidence. No epsilon or receipt is broadened.

The shared target also retained 17 main-only SVGs. They are not maintenance
fixtures. The isolated browser input tree copies every live maintenance
upstream fixture's generated local SVG without changing its bytes, and checks
that none is missing.

The new-head Linux workspace, platform and painted-content checks are owned by
the replacement CI run. Local macOS evidence does not qualify that run in advance.

## Scope

Pie keeps the upstream fixed 450px height; correcting horizontal text extents
does not introduce a new vertical containment guarantee. Dynamic CSS and
document-relative units are not a complete browser CSS engine.

XYChart literal direct-text measurement, Architecture styled service bounds and
the public HTML helper's quoted-attribute scanning remain separate audit items.
No public ABI, dependency graph, upstream SVG, comparator policy or accepted
residual is changed. This work makes no performance qualification claim.
