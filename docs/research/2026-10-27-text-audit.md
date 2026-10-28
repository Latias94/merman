# Text audit and CI follow-up

Date: 2026-10-27. Main PR: #246; maintenance PR: #247.
The preceding implementation and verification are recorded in
[terminal text repair evidence](2026-10-26-text-terminal.md).

## Confirmed CI failures

Main run `40336543692` executes 13,448 workspace tests: 13,446 pass and two fail,
with nine additional skips. Both failures reproduce locally before repair:

- Requirement SVG names receive an inline bold role after ordinary SVG word
  wrapping. The shared word-style reset removes the role from measurement,
  although the writer still emits it. Source font-family/size alone exposes the
  missing bold request in `requirement_source_fonts_reach_node_measurement_and_retire_inherited_claims`.
- The Usecase ELK boundary-label guard test freezes relationship text dimensions
  but leaves node dimensions dependent on the old font behavior. Correct root
  font inheritance changes the topology assumed by its TB assertion. Freeze the
  complete fixture geometry and retain its independent collision oracle, route
  ordering, ports and marker assertions.

Requirement now wraps before applying final SVG role/source faces to the first
row, then measures the retained word rows. Rendering uses the same ordinary-word
wrapping probes.
This follows the pinned `requirementBox.ts`: `createText` runs first, row styles
are applied next, and `getBBox` runs after that update. Source declarations retain
their precedence over the default name role. HTML child faces and root/config
`calculateTextWidth` probes remain separate.

An independent review catches an initial all-row override. Both pinned sources
use `text.children[0]` to select only the first outer tspan. A real Chromium
render of the pinned main runtime confirms that `W<br/>WW WW` has a bold `W`
and two normal words on its second row. The repaired writer limits inline
overrides to that first row. Measurement combines its styled bbox with the
ordinary remaining rows at the emitted 1.1em offset, including each profile's
createText bbox Y. Multi-row and source-normal/Markdown-strong cases guard this
scope independently of the single-row contract.
Leading blank rows in the remaining block still consume their emitted row
positions. They are trimmed before measuring that block, and their indices enter
the Y offset. Native-like tests skip empty-row ink and cover an omitted code span
between visible rows, an empty first row and an empty tail.

A face-sensitive test gives ordinary text a 50px width and final bold text a
100px width under a 60px wrapping budget. It verifies one row with the wider final
geometry, plus source weight overrides. Source-face integration tests distinguish
ordinary wrapping requests from final row measurements instead of asserting that
every intermediate request already has the terminal face.

Maintenance has a different Requirement implementation and keeps its existing
source-style capability boundary. Its adaptation repairs the SVG bold-role bbox
after wrapping without importing main's typography/evidence architecture.

Maintenance run `40336772044` initially fails while launching the Linux VS Code
Electron window, before extension assertions run. A single rerun passes all 59
selected checks, including `pr-gate`; four other checks are intentionally skipped.
This recovery does not justify changing the extension runner or adding retries.

## Other family findings

These are source-traced measurement/output divergences, not a claim that every
example has been rendered in a browser or that all 37 families were exhaustively
audited. The source audit covers roughly 18 families.

| Surface | Divergence | Next discriminating check |
| --- | --- | --- |
| Pie | `pie/theme/css_binding.rs` fixes legend/title measurement at 17/25px, while the SVG stylesheet consumes `pieLegendTextSize` and `pieTitleTextSize`. Those measurements affect root bounds. | Large configured title and legend sizes, with a size-sensitive measurement profile. |
| GitGraph neo/redux | `gitgraph/theme/typography.rs` leaves commit-label weight unset; the emitted `.commit-label` rule uses `noteFontWeight`. | Bold configured labels whose measured extents differ from normal. |
| Timeline redux | Section measurements use ordinary base weight, while emitted section CSS selects the theme's heavier weight. | Separate the virtual normal-height probe from the final section-label bbox. |
| XYChart | Direct SVG title text is XML-escaped literally, but the generic deterministic text normalization interprets `<br/>` as a line break. | A literal `AAAAAA<br/>BBBBBB` title; verify literal text and a single direct-text measurement. |
| Architecture | Group-free service bounds flatten styled rows before bbox measurement, although the writer retains strong/emphasis faces. | A standalone bold service title with a face-sensitive profile and zero diagram padding. |
| Public HTML measurement helper | `measure_html_with_inline_styles` stops a tag at the first `>` even inside a quoted attribute, so `<span title="a > b">X</span>` can measure attribute text. | Call the helper directly; family sanitization already escapes this attribute delimiter, so this is not evidence that every family render is affected. |

ER, Kanban, Journey, Gantt, Quadrant, Packet, Sankey, Radar and Info/Error yield no
additional confirmed divergence in the inspected paths. This is not proof of
absence. ER's unsupported source-font residual is already explicit. Gantt's
post-measurement clickable class matches upstream ordering. Treemap title-origin
behavior still needs browser evidence.

## Architecture decision

Keep the existing styled-row analysis, entity provenance, Class note label plan,
Flowchart prepared label, State geometry and font cascade. They already provide
useful ownership and reuse. A complete text-system rewrite is not justified by
these findings.

Converge the remaining consumers on one prepared private label result containing:

1. The visible text projection and styled rows, including entity provenance.
2. The resolved font at the actual rendered element, including role/source order.
3. The measurement primitive and stage: wrapping budget, advance probe, final
   bbox or client rect.

Migrate call sites that share the pinned `createText` semantics first. Direct SVG
text and HTML retain their own semantics. Resolve declaration winners before
evaluating relative values against the real parent; preserve Flowchart's distinct
compiled-style invalid-winner behavior.

Continue validating final SVG bytes independently. A prepared label cannot prove
that the writer and later transformations actually emitted the promised style.
Do not introduce a generic CSS engine, replace family DOM structures, broaden
comparator tolerances, or hide browser font/getBBox residuals to complete this
migration.

## Verification

The two CI failures reproduce before repair. The focused repaired run passes
123 Requirement/Usecase tests. After the first-row review correction and the
rendered-DOM regression, the complete renderer run passes 5,358 tests, with five
existing skips, using `all-diagrams,layout-elk,layout-cytoscape,math`. Strict
renderer-library Clippy and the complete release-mode 37-family comparison in
`structure`, `parity` and `parity-root` modes also pass. These checks are repeated
after the empty-row correction. Focused independent review confirms that both
first-row scope and empty-row offsets are repaired, with no remaining blocker
in that reviewed change. No upstream SVG, comparator policy or residual receipt changes.

The new-head Linux workspace and painted-content oracle remain owned by remote
CI. The local multi-policy comparison does not claim a successful remote run or
a fresh Linux browser paint audit.
