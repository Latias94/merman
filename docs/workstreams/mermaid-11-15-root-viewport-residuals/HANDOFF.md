# Mermaid 11.15 Root Viewport Residuals - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

This lane was split from `mermaid-11-15-complete-adaptation` after structural implemented-matrix
Mermaid 11.15 `parity` passed, while full `parity-root` remained red for root
`viewBox`/`max-width` residuals.

`xtask compare-all-svgs --dom-mode parity-root` now produces bounded failure summaries instead of
attempting to print every residual line in the final error.

M15RV-020 classified the Sequence bucket. Sequence now has a reusable
`compare-sequence-svgs --no-root-overrides` diagnostic path, and
`SvgRenderOptions.apply_root_overrides` is respected by the Sequence renderer. Three stale
11.12-derived Sequence root pins were removed after a pinned-vs-unpinned run showed they were
making Mermaid 11.15 root output worse. Mermaid 11.15 central-connection source rules were checked
against `sequenceRenderer.ts`; the Rust layout/render constants already match the upstream
central-connection offsets, so the remaining central rows are root-bounds/text-measurement
residuals rather than missing central-connection semantics.

M15RV-030 classified the Flowchart bucket. Fresh all-row reports show 61 root mismatches with root
overrides enabled: 60 `style=max-width` mismatches and 1 `viewBox` mismatch. The largest absolute
root-width delta is about 2.24px and comes from SVG text measurement drift in markdown/htmlLabels
false shape fixtures; a focused label report for the largest row showed two node labels at about
`-0.602px` width each. Disabling Flowchart root overrides increases mismatches to 96, so retained
Flowchart pins are mostly still paying down root drift. One stale pin
(`upstream_cypress_flowchart_spec_17_render_multiline_texts_017`) was removed because computed
bounds are closer to Mermaid 11.15 than the old override.

M15RV-040 classified Architecture, Class, and C4. C4 is now root-green: 15 existing
fixture-derived root pins were refreshed from the current Mermaid 11.15 upstream SVG root
`viewBox`/`max-width` values, reducing C4 root residuals from 15 to 0 without increasing override
count. Disabling C4 root overrides still produces 35 mismatches, so the table is paying down real
browser-root drift. Architecture remains at 32 unaccepted root residuals; disabling Architecture
root overrides increases raw mismatches to 63, and the remaining enabled rows are dominated by
group/port/disconnected-component layout-root drift rather than stale pins. Class remains at 18
unaccepted residuals after the 2 existing accepted class rows; Class has no root override table,
and its largest rows are namespace/layout-width residuals.

M15RV-050 classified the smaller ER, Sankey, Timeline, and Journey buckets. ER and Sankey are now
root-green after refreshing existing fixture-derived root pins to Mermaid 11.15 upstream root
values. Timeline was reduced from 7 to 3 by refreshing 4 existing root pins; the 3 remaining rows
are unpinned 0.5-1px root-width tails and were not converted into new fixture pins. Journey
remains at 2 unpinned 1.25-2px root-width tails and has no root pin table.

M15RV-060 continued the Class source-rule follow-up. The focused repro
`upstream_pkgtests_classdiagram_spec_003` showed that the previous local output laid out `Admin`
and `Report` horizontally inside nested namespaces (`1014px` local max-width vs `499.75px`
upstream). Mermaid 11.15 source inspection replaced the earlier v2 assumption with the active v3
unified path: `classDiagram.ts` uses `classRenderer-v3-unified.ts`, `ClassDB.getData()` emits all
namespace group nodes before class/note/interface nodes, and the shared rendering-util Dagre
extractor uses child-before-parent `copy(...)`, moved child extraction reparenting, and recursive
`ranksep: parent.ranksep + 25`. Rust now mirrors those source rules. `upstream_namespaces_and_generics`,
`upstream_pkgtests_classdiagram_spec_006`, `stress_class_nested_namespaces_many_levels_021`, and
`stress_class_nested_namespaces_cross_edges_008` are root-green. Class structural parity is green
after the Class renderer was corrected to use Mermaid's `mainBkg`/`nodeBorder` defaults for node
styling. The SVG Class text path now wraps titles with normal-weight `createText(...)` measurement
before the final outer bolder bbox, and it preserves raw numeric `themeVariables.fontSize` CSS
spelling without treating unitless CSS as a headless 24px text size.

M15RV-070 extracted the font-size policy that M15RV-060 made explicit. `config_css_number_or_string(...)`
now captures Mermaid stylesheet interpolation where JSON numeric theme values are emitted without
adding `px`, and `config_f64_explicit_css_px(...)` captures the Class SVG measurement rule where
only explicit `px` strings are layout-effective. Class and Radar call sites that already carried
local versions of these rules now share the helpers. This was intentionally scoped to Class/Radar;
other diagrams still need per-source-path checks before using the new helpers.

## Active Task

- Task ID: M15RV-070
- Owner: codex
- Status: DONE
- Goal: Extract shared font-size config helpers for Mermaid raw CSS interpolation and explicit-px
  SVG text measurement policy.
- Evidence: `target/compare/radar_font_size_after_css_font_helper.md`,
  `target/compare/radar_numeric_font_size_after_css_font_helper.md`,
  `target/compare/class_font_size_025_after_css_font_helper.md`, and
  `target/compare/class_font_size_026_after_css_font_helper.md`
- Concern: The helper is not a license to migrate every diagram blindly. Sequence, Architecture,
  C4, State, and shared CSS paths need source checks because some Mermaid code uses
  `parseFontSize(...)` or diagram-specific `.style('font-size', value + 'px')` semantics.

## Fresh Counts

- Total unaccepted full-root residuals: 277.
- Largest buckets: Sequence 167, Flowchart 61, Architecture 32, Class 12.
- Smaller buckets: Timeline 3, Journey 2.
- Closed in M15RV-040: C4 15 -> 0.
- Closed in M15RV-050: ER 3 -> 0, Sankey 3 -> 0, Timeline 7 -> 3.

## Guardrails

- Keep structural `parity` green.
- Do not add hand-written per-string browser metric constants at renderer call sites.
- Prefer Mermaid source rules, generated browser-probe tables, or explicit diagnostic residual
  policy entries.
- Do not close M15RV-090 by accepting the current residual set. The remaining Class and
  Architecture rows include real layout/root-bounds differences.
- Use the shared config helpers for future font-size work only when the Mermaid source path matches
  their policy; otherwise leave diagram-local behavior explicit.
