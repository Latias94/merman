# Mermaid 11.15 Root Viewport Residuals - Handoff

Status: Active
Last updated: 2026-06-02

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

M15RV-040 also received a diagnostics follow-up. Architecture compare commands now support
`--report-root`, `--report-root-all`, `--report-root-limit`, and `--no-root-overrides`, matching
the established root-audit flow used by Flowchart and Sequence. The Architecture renderer now
threads `SvgRenderOptions.apply_root_overrides` into final root viewport emission, so the CLI
switch no longer depends on a process-wide environment variable. Fresh focused and all-diagram
reports prove the path works and emits a `Root Viewport Deltas` table for Architecture. This did
not change residual counts: Architecture remains at 32 unaccepted root rows, but the bucket is now
much easier to inspect with the same tooling as the other diagrams.

On 2026-06-02, that diagnostics path immediately exposed a stale Architecture upstream baseline:
`upstream_architecture_docs_service_icon_text`. The old pinned upstream SVG claimed a
`343.884px` root width while both fresh Mermaid CLI output and the local renderer were in the
`454-464px` range. Refreshing just that upstream SVG via `gen-upstream-svgs --diagram architecture
--filter upstream_architecture_docs_service_icon_text` collapsed the apparent `+120px` residual
into a smaller `+10.145px` iconText root-bounds tail. This is important: do not treat that row as
evidence of a gross Architecture layout bug anymore. It is now an iconText `foreignObject` bbox
approximation problem.

The same follow-up also proved one local Architecture calibration had gone stale. The
`is_reasonable_height_profile(...)` width adjustment (`+0.380126953125px`) produced the same
overshoot with or without root overrides enabled, so it was a source-owned local bump rather than a
pinning artifact. Removing that bump made
`upstream_architecture_cypress_reasonable_height`,
`upstream_architecture_layout_reasonable_height`, and
`upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_a_reasonable_height_011`
all root-green, and reduced the aggregate Architecture bucket from 32 to 29 residuals.

M15RV-080 started a Sequence text-measurement policy audit from the central-connection RTL row.
Mermaid 11.15 source still computes Sequence message spacing through
`utils.calculateTextDimensions(...)` inside `getMaxMessageWidthPerActor(...)`, then feeds that into
`calculateActorMargins(...)`; `calculateTextDimensions(...)` measures browser SVG text with both
`sans-serif` and the configured family, rounds the browser bbox, and chooses the configured family
unless the sans-serif dimensions are strictly larger. Rust central-connection constants already
match Mermaid source, and new regression tests prove the default layout centers and first message
line are preserved by the SVG renderer. The focused headless SVG compare still fails as a root-only
residual: Mermaid 11.15 upstream root is `965px`, while the Rust headless SVG path emits `1028px`.

Two attempted fixes were explicitly rejected. A diagnostic substitution of deterministic message
measurement improved this focused row to `995px`, but increased the raw Sequence root mismatch
count from 168 to 169, so it is not a defensible parity fix. A first full
`gen-svg-overrides --mode sequence` refresh added a much larger override table and made the focused
row worse (`1034px`), which exposed that the generator was using the wrong browser path for
Sequence evidence. Refreshing the focused upstream SVG with `gen-upstream-svgs` produced no file
diff, confirming the `965px` baseline is not stale under the current upstream export path.

M15RV-080 then repaired that generator path instead of forcing a renderer constant. Sequence SVG
override generation now follows Mermaid CLI / Puppeteer's default bundled-browser behavior when no
explicit executable is provided, skips wrap-sensitive fixtures, and avoids raw message seeds without
actor endpoints. The regenerated table has 891 rows and passes the override budget. Exact final SVG
overrides are now kept out of incremental wrap probing through a wrap-specific measurement seam.
The central RTL fixture is root-exact at `965px`, full Sequence structural parity is green, and the
raw Sequence root bucket dropped from 168 to 68 rows. Full `compare-all-svgs --dom-mode
parity-root` accepts the existing `sequence/zed_pr_57644_sequence` row, leaving 67 unaccepted
Sequence residuals.

During the final structural gate, Architecture exposed fixture-corpus churn around
Mermaid 11.15 `svgDraw.ts`: refreshed fixtures can use diagram-scoped service/node/group IDs and
the current absolute fallback service background path, while older Architecture fixtures still use
bare IDs and the older relative path spelling. The renderer now uses the current 11.15 absolute
fallback path, and the parity comparator normalizes only those Architecture ID/path spelling
variants so structural gates do not encode stale fixture-generation details. Architecture
structural parity and full structural parity are green.

## Active Task

- Task ID: M15RV-085
- Owner: codex
- Status: PENDING
- Goal: Classify and reduce the remaining Sequence HTML `<br>` / wrap / note / participant root
  tails after the SVG override generator repair.
- Evidence: start from `target/compare/sequence_report_parity_root_after_sequence_override_cleanup.md`
  and the latest `target/compare/sequence_report_parity_root.md`.
- Concern: Do not add string-by-string browser constants. Prefer reusable HTML/wrap source rules,
  generator-backed evidence, or explicit diagnostic residual policy.

## Fresh Counts

- Total unaccepted full-root residuals: 175.
- Largest buckets: Sequence 67, Flowchart 61, Architecture 30, Class 12.
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
- Use the new Architecture `--no-root-overrides` / `--report-root-all` path before touching any
  Architecture root residual. That should be the default evidence source for deciding whether a row
  is stale-pin, source-rule, or browser-measurement debt.
- Recheck `target/compare/architecture_report_parity_root.md` after any upstream SVG refresh before
  reasoning from older focused reports; stale focused reports can still show the pre-refresh
  `+120px` iconText row even after the canonical aggregate report has moved to the refreshed
  `+10px` residual.
- Prefer deleting stale source-owned calibrations like `reasonable_height` before inventing new
  root heuristics. Those removals are lower risk and easier to defend against Mermaid source and
  fixture evidence.
- Treat Architecture diagram-scoped service/node/group IDs and fallback service background path
  spelling as compare-time fixture-churn normalization, not renderer behavior to toggle per
  fixture. Root gates remain the authority for visual viewport impact.
