# Mermaid 11.15 Root Viewport Residuals - Handoff

Status: Active
Last updated: 2026-06-05

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

M15RV-085 refined the Sequence wrap evidence policy. Mermaid 11.15 source confirms that
`lineBreakRegex` splits HTML `<br>` variants before wrapping, while `wrapLabel(...)` short-circuits
when a label already contains `<br>`. Message and note widths still route through browser-derived
`calculateTextDimensions(...)`, so the fix stayed in generated evidence and measurement policy
rather than broad deterministic layout substitution. The Sequence SVG override generator now keeps
final emitted `actor`, `messageText`, and `noteText` nodes from wrap fixtures, but still filters
raw wrapped actors/messages/boxes as incremental wrap seeds. A guarded single-line fit helper lets
exact final SVG text evidence prevent false wrapping only when that exact text fits the current
wrap width with margin. The four largest HTML `<br>` / wrap rows are root-exact:
`stress_br_in_messages_notes_011`, `stress_long_participant_labels_br_031`,
`stress_sequence_batch5_wrap_html_br_spans_042`, and `html_br_variants_and_wrap`. The generated
Sequence SVG text metric table now has 1036 rows, `report-overrides --check-no-growth` passes, full
Sequence structural parity is green, and raw Sequence root mismatches dropped from 68 to 64. Full
all-diagram root policy accepts the existing `sequence/zed_pr_57644_sequence` residual, leaving
63 unaccepted Sequence rows.

M15RV-087 refined the Sequence actor-type/root-bound policy. Mermaid 11.15 `svgDraw.js` shows
database cylinders use `rect.width / 3`, and boundary/control/entity actor-man glyphs use 22px
source radii/offsets. Rust now mirrors those glyph rules, positions created actor tops from
`lineStartY - actor.height / 2` using the pre-render lifecycle height, and includes Mermaid's
footer-row `maxHeight` cursor bump in root bounds. Six stale Sequence root pins were deleted after
focused `--no-root-overrides` checks proved the computed roots were exact. Raw Sequence root
mismatches dropped from 64 to 28, and the full all-diagram root policy leaves 27 unaccepted
Sequence rows after the accepted `zed_pr_57644_sequence` residual.

M15RV-088 re-grounded Architecture against the current Mermaid 11.15 source and fixture corpus.
Fresh `gen-upstream-svgs --diagram architecture` output for all 185 stored fixtures proved that
some apparent root and DOM differences were stale baselines, not Rust renderer regressions. Mermaid
11.15 `svgDraw.ts` emits diagram-scoped Architecture IDs for edges, services, fallback service
backgrounds, junctions, and groups, and uses the current absolute fallback service background path.
Rust now emits those source-derived IDs/paths directly, the 185 stored Architecture upstream SVGs
were refreshed, and the Architecture comparator no longer needs fixture-vintage ID/path
normalization for the refreshed corpus. All 31 stale Architecture root viewport pins were deleted,
and the old 11.12-era groups-within-groups root calibration was removed after fresh 11.15 evidence
showed it introduced deterministic errors. Architecture structural parity is green, Architecture
root overrides are now zero, and full root evidence reports 32 unaccepted Architecture rows. This
is an honest increase from the pre-refresh 30-row count because stale baselines/pins no longer hide
current 11.15 FCoSE/group-port root tails.

M15RV-089 started with the largest Architecture FCoSE/group-port rows and found a source-backed
bug in the Rust FCoSE input model. Mermaid 11.15 `architectureRenderer.ts` creates junction
Cytoscape nodes with `parent: junction.in`; it does not infer a group from neighboring services.
Rust had a local heuristic that assigned ungrouped junctions to the most frequent neighboring
service group. In `stress_architecture_junction_fork_join_026`, that incorrectly put `fork` inside
`left`, changing the `fork -> auth` layout edge from a cross-group weak spring into a same-group
strong spring. Removing the inference keeps Architecture structural parity and full all-diagram
structural parity green. Architecture root residuals dropped from 32 to 30:
`stress_architecture_fan_in_out_021` and
`stress_architecture_batch6_junctions_multi_split_with_group_edges_087` are root-exact, while
`stress_architecture_junction_fork_join_026` shrank from about `-1551px` to about `+14px`.
The next largest row, `stress_architecture_deep_nesting_013`, exposed a second source-backed
FCoSE input bug. Mermaid 11.15 `ArchitectureDB.getDataStructures()` updates `groupAlignments`
while reducing `this.nodes` and each node's `service.edges`, so the same edge can update the map
once per endpoint and later endpoint traversal overwrites earlier group alignment values. Rust had
collapsed that to one global edge pass, leaving the `core`/`data` alignment horizontal where
Mermaid's endpoint traversal leaves it vertical. Mirroring the source traversal changed the
focused constraints from `horizontal=[[lb, api, api, db]]` / `vertical=[[lb, ext]]` to
`horizontal=[[lb, api]]` / `vertical=[[lb, ext], [api, cache]]`, making
`stress_architecture_deep_nesting_013` root-exact. Architecture residuals are now 29.

The next Architecture row,
`stress_architecture_batch6_init_fontsize_icon_size_wrap_093`, turned out to include a smaller
source-owned group-padding defect rather than a pure text-width issue. Mermaid 11.15
`architectureRenderer.ts` sets `.node-group` padding from `db.getConfigField('padding')`, while
Rust had been using `iconSize / 2` as a proxy. The Architecture browser probe was fixed to parse
fixture `%%{init: ...}%%` directives before `mermaid.initialize(...)`, proving this fixture's
effective config is `iconSize=40`, `fontSize=18`, `padding=30`. Rust now sizes group rectangles
from `padding + 2.5`, and the duplicated Cytoscape canvas-label width approximation has been
extracted into a shared helper used by both layout and SVG root-bound code. This moved the focused
row from about `-22.5px` to about `-2.5px` while preserving the same 29-row Architecture failure
set. A tested `+1px` exact-browser bbox tweak was rejected because it increased Architecture root
mismatches to 31.

A follow-up source check on `stress_architecture_junction_fork_join_026` found that Mermaid
`getRelativeConstraints(...)` does not skip duplicate queued grid positions when popping from the
BFS queue. The browser probe reports 9 relative constraints for this fixture, with duplicated
`join -> db` and `join -> cache` constraints. Rust previously skipped duplicate pops and emitted
7. The relative-constraint BFS is now extracted into a helper and preserves Mermaid's duplicate-pop
behavior, with a unit test covering the fork/join diamond. This did not change the remaining
`+13.976px` root tail, so treat it as source-input parity plus residual classification rather than
a viewport fix.

Later HPD-050 correction: a fresh local audit compared the old saved Mermaid debug probe
`target/compare/arch_junction_fork_join_probe_m15rv089.json` against the current local SVG, the
stored upstream SVG, and a fresh Edge-backed `check-upstream-svgs` output. Local service positions
matched that old saved debug probe to floating-point noise, but the stored upstream fixture is
reproducible by the current CLI/Edge baseline path. A refreshed HPD-050 probe still does not
reproduce the CLI fixture, so treat
`stress_architecture_junction_fork_join_026` as a debug-probe harness / CLI-harness divergence plus
solver/phase residual candidate before touching manatee again.

The group-padding source rule now applies in both places that approximate Cytoscape compound
bounds. Final SVG group rectangles had already moved from `iconSize / 2` to configured
`architecture.padding`; the pre-layout compound bbox used before FCoSE now also uses
`padding + 2.5`. This is source-consistency work rather than a count reduction: the custom-init
rows remain in the same residual set, and the remaining drift is label/bbox measurement dominated.

On 2026-06-02, two more focused Architecture probes refined the remaining M15RV-089 direction:

- `stress_architecture_batch5_long_titles_and_punct_076` is not primarily a title-wrap width bug.
  The local and upstream `pipeline` group rect dimensions match; the residual is a global
  left/right phase shift of roughly `10px`, so the root `max-width` grows from `543px` upstream to
  `553px` local.
- `stress_architecture_disconnected_islands_046` is not a width drift at all. The root width is
  exact, while the local root height is about `+7.19px` and the disconnected components land in a
  different vertical phase than upstream.
- A same-turn experiment changed `manatee`'s pre-layout compound relocation bbox to avoid
  recursive child-compound padding accumulation inside `bounding_box_center_eles()`. Focused
  `parity-root` checks for `disconnected_islands_046` and `long_titles_and_punct_076` were
  unchanged, so that experiment was reverted immediately and should not be repeated as an
  evidence-free guess.

Current implication: the next promising Architecture follow-up is still in the
component-relocation / initial-center / Cytoscape phase-alignment family, but not that specific
compound-padding hypothesis. Prefer new probes around FCoSE relocation center semantics,
disconnected-component ordering/phase, or top-left-vs-center coordinate transfer before changing
shared bbox padding rules again.

One small refactor landed during the same lane to support that investigation without changing the
residual set: Architecture's pre-layout Cytoscape node-bbox extras now convert to
`manatee::BoundsExtras` through a shared helper in `architecture_metrics.rs` instead of repeating
field-by-field mapping inside `architecture.rs`. This keeps the "Architecture approximation ->
FCoSE input extras" seam explicit and easier to audit before further measurement extraction.

Another behavior-preserving extraction followed immediately after: Architecture service bounds in
the SVG parity renderer now go through a dedicated helper that returns three explicit views of the
same service geometry: icon-only bounds, root `svg.getBBox()`-style bounds, and Cytoscape
compound `node.boundingBox()`-style bounds. Previously those two label-bbox approximations were
interleaved inside one local loop variable. The new split did not aim to change residual counts;
it makes future root-tail audits much easier because root vs compound measurement policy now has a
named seam instead of duplicated inline arithmetic.

HPD-050 continued the same boundary cleanup by extracting Architecture's pre-layout Cytoscape bbox
adapter into `architecture_fcose_prelayout_bounds(...)`. That helper now owns the FCoSE
`initial_center` and node `BoundsExtras` approximation. Group title state was removed from the
layout view because current source/evidence says group titles do not affect the pre-layout
`eles.boundingBox()` center. The batch5 long-title focused root tail stayed unchanged at upstream
`542.926px` vs local `547.926px`, so this is auditability work rather than a hidden viewport tune.

Superseded by a later HPD-050 cleanup: the renderer-side `initial_center` / pre-layout group bbox
model was removed after confirming it was not consumed by layout. The retained renderer seam is
the node `BoundsExtras` adapter; relocation and element bbox policy remain in `manatee`.

Fresh 2026-06-02 focused probes then narrowed the remaining Architecture label-width family even
further. Mermaid 11.15 source confirms the Cytoscape layout phase only sees single-line canvas
`label` + `font-size` on `node[label]`; it does not reuse the final SVG `createText(..., width:
iconSize * 1.5)` wrapping path for group sizing. That means the current Architecture residuals
cannot be fixed by feeding root-wrap width back into compound sizing; a direct experiment doing
that overshot badly and was reverted immediately. Focused baseline rows at this point are:

- `stress_architecture_batch5_long_titles_and_punct_076`: upstream `543px`, local `553px`
- `stress_architecture_batch4_init_small_icons_061`: upstream `187.75px`, local `178.5px`
- `stress_architecture_html_titles_and_escapes_041`: upstream `480px`, local `485px`
- `stress_architecture_unicode_and_xml_escapes_019`: upstream `469.75px`, local `472.75px`

The attempted follow-up that *did* show a useful signal was narrower: reducing the single-line
canvas width scale only for very long labels moved `batch5_long_titles_and_punct_076` from
`553px` to `548px` without immediately breaking `batch4_init_small_icons_061`. That experiment was
also reverted because it had not yet been validated against the broader Architecture bucket, but it
defines the next realistic avenue: if we continue on this family, use a source-compatible,
headless-only piecewise approximation for long single-line canvas labels, not a root-wrap
substitution or a global scale change.

The newly added ignored diagnostic test
`architecture_root_width_diagnostic_matrix` makes that experiment easy to rerun without invoking
four separate compare commands. The matrix was later expanded to six rows so we can distinguish
service-title compound-label effects from pure group-title rows. On the current baseline it prints:

- `stress_architecture_batch5_long_titles_and_punct_076`: `552.9256591796875`
- `stress_architecture_batch4_init_small_icons_061`: `178.57139587402344`
- `stress_architecture_html_titles_and_escapes_041`: `484.9256286621094`
- `stress_architecture_unicode_and_xml_escapes_019`: `472.8219299316406`
- `stress_architecture_long_group_titles_018`: `481.3125`
- `stress_architecture_batch6_long_group_titles_wrapping_extreme_095`: `532.53125`

That long-label piecewise-scale rule is now active in
`architecture_cytoscape_canvas_label_metrics(...)`: when the measured single-line title width is
`>= 200px`, Rust uses scale `1.01` instead of the generic `1.055` Cytoscape canvas-label
approximation. Re-running the six-row diagnostic matrix with that rule shows only the intended long
service-title row moves:

- `stress_architecture_batch5_long_titles_and_punct_076`: `552.9256591796875 -> 547.9256591796875`
- `stress_architecture_batch4_init_small_icons_061`: unchanged
- `stress_architecture_html_titles_and_escapes_041`: unchanged
- `stress_architecture_unicode_and_xml_escapes_019`: unchanged
- `stress_architecture_long_group_titles_018`: unchanged
- `stress_architecture_batch6_long_group_titles_wrapping_extreme_095`: unchanged

The focused root compare confirms the same improvement at the viewport level:
`stress_architecture_batch5_long_titles_and_punct_076` moved from `+10.000px` to `+5.000px`
against the Mermaid 11.15 root width while the nearby small-icon / HTML / unicode rows stayed at
their previous values. The two added group-title rows matter because they should *not* move much
under a service-label canvas-width experiment; their flat readings are the current evidence that
the approximation is scoped to the service compound-label family rather than the separate
group-title/root-title path.

This means the long-label piecewise rule is no longer a hypothetical experiment or reverted branch:
it is the current committed headless approximation. Continue treating it as a reusable,
Architecture-specific Cytoscape canvas-label policy seam rather than as a fixture-tuned root pin.

Additional 2026-06-02 diagnostic result:

- `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` now prints the top-level node/compound/edge/edge-label bbox
  contributors inside `bounding_box_center_eles()`. Focused probes show the current Architecture
  phase drift is not primarily edge-label driven:
  - `stress_architecture_batch5_long_titles_and_punct_076`: run1 `orig_center=(-24,17)` comes from
    the single top-level compound bbox `(-260.462816,-174.462816)-(212.462816,208.462816)`. The
    four edge endpoint rows stay inside that compound bbox and do not change the total extents.
  - `stress_architecture_disconnected_islands_046`: run1 `orig_center=(0.75,17.75)` comes from the
    union of two top-level compound bboxes plus the root-level isolated node `D`; the single edge
    endpoint row also stays inside those extents.
- Therefore, the next aligned probe should focus on how top-level compound bounds are derived in
  `manatee::fcose::bounding_box_center_eles()` versus Cytoscape's `eles.boundingBox()` for
  Architecture, rather than on edge-label geometry or the outer `architecture.rs` helper
  `initial_center`.

Another focused 2026-06-02 result narrowed `stress_architecture_disconnected_islands_046`
further:

- Running the focused compare with `MANATEE_FCOSE_DEBUG_RELOCATE=1` and
  `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` shows the Rust FCoSE relocation path is active and
  deterministic:
  - run0 `orig_center=(0,8.5)` from the pre-layout bbox
    `(-82.5,-82.5)-(82.5,99.5)`, then relocate delta `(-21.915432,+11.192230)`.
  - run1 `orig_center=(0.75,17.75)` from the pre-layout bbox
    `(-350.507109,-306.235956)-(352.007109,341.735956)`, then relocate delta
    `(+7.081728,+25.324378)`.
- Disabling relocation entirely with `MANATEE_FCOSE_DISABLE_RELOCATE=1` does **not** change the
  focused root residual: the final local SVG remains `823.25 x 775.5` against upstream
  `823.25 x 768.5`.
- Therefore this row is not caused by the final `aux.relocateComponent(...)`-style translation.
  The remaining height drift is produced earlier by the disconnected-component / owner-graph
  layout solution itself.
- A follow-up source check against layout-base `LGraph.updateConnected()` also removed one tempting
  false lead: a single-node owner graph is considered connected upstream because the BFS seed node
  counts as visited immediately. Do not try to "fix" disconnected component height by forcing
  isolated child graphs into the gravity set; that would diverge from upstream semantics.

`stress_architecture_batch5_long_titles_and_punct_076` and
`stress_architecture_batch4_init_small_icons_061` were classified as measurement diagnostics rather
than patched. Browser probes show the batch5 row is driven by Cytoscape canvas label bbox mismatch:
the long `Artifacts Storage retention 30d` label is about `223px` wide in Chromium/Cytoscape, while
the current deterministic scaled estimate is still wider even after the committed long-label
piecewise rule. The batch4 small-icon row is instead icon-floor dominated (`42x56` browser service
bboxes). A single new global label scale would still be self-deceptive; future work should use
generated Architecture canvas-label evidence or a better deterministic canvas measurer.

Later HPD-050 correction: the small-icon service/group sizing diagnosis was still useful, but the
root-width cause was not a service label scale. Re-auditing Mermaid `svgDraw.ts` and
`setupGraphViewbox.js` showed that the rotated Y-axis edge label's non-centered `createText()`
local y-range contributes to `svg.getBBox()`. Merman now transforms that y-range for Architecture
edge-label root bounds and uses `fontSize + 1px` for compound label bottom. As a result,
`stress_architecture_batch4_init_small_icons_061` is root-green without a root override.

Fresh focused 2026-06-02 rechecks also show the three `reasonable_height` fixtures are not
root-green: each still carries the same `+0.380px` width / tiny height rounding tail
(`1859.440px -> 1859.820px` max-width). Treat those rows as part of the honest residual set unless
a source-compatible rounding rule closes the whole family.

Two additional Architecture rows were classified without renderer changes. For
`stress_architecture_html_titles_and_escapes_041`, focused structural parity is green and the
browser probe constraints match Rust; the `+5px` root tail is controlled by the group rect
(`399.926px` upstream vs `404.926px` local), not by edge labels or HTML/entity parsing. Treat it as
another group/service Cytoscape bbox approximation tail. For
`stress_architecture_group_port_edges_017`, browser and Rust share the same alignment/relative
constraints and source-derived same-parent/cross-parent edge force policy. All service labels are
icon-floor dominated (`82x100` browser bboxes), but the final vertical spacing differs by about
`17.845px`, so this row is best classified as source-input-matched manatee vs cytoscape-fcose
solver/compound-bound drift unless a future probe finds a reusable missing FCoSE rule.

Follow-up focused checks classify four more Architecture rows. `stress_architecture_unicode_and_xml_escapes_019`
is another group/service Cytoscape bbox tail: the fixture avoids XML grammar pitfalls, constraints
match, and the local group is about `3px` wider because `Metrics Exporter` is overestimated.
`stress_architecture_edge_label_corner_cases_012` and
`stress_architecture_batch4_init_fontsize_wrap_063` are edge-label `getBBox()` tails: text splitting
and transforms match upstream, but browser text bboxes are about `1.788px` wider than the current
headless estimate. `stress_architecture_nested_groups_002` is a nested compound/layout tail with
matching source inputs; local services shift about `+1.25px` in X and the outer group right edge
lands about `3.75px` farther right.

Later HPD-050 correction: `stress_architecture_edge_label_corner_cases_012` and
`stress_architecture_batch4_init_fontsize_wrap_063` were also closed by the source-backed
`createText()` y-range edge-label bounds fix. Do not reopen them as browser text-scale tails unless
a future Mermaid baseline changes the source behavior.

## Active Task

- Task ID: M15RV-089
- Owner: codex
- Status: IN_PROGRESS
- Goal: Investigate the top Architecture FCoSE/group-port root residuals after the 11.15 baseline
  refresh and stale Architecture root-pin deletion.
- Evidence: start from
  `target/compare/architecture_report_parity_root_hpd050_residual_classification_refresh.md`;
  fresh Architecture root evidence currently reports `25` mismatches.
- Concern: Do not add root pins or browser-dependent layout hacks for the remaining rows. The new
  25-row count comes from Mermaid source rules and later HPD-050 root-bounds seams, not from
  restoring stale baselines or pins.

## Fresh Counts

- Total unaccepted full-root residuals: 134 in the last full all-diagram run; refresh this before
  policy closeout.
- Largest buckets in the current family-level evidence: Flowchart 61, Architecture 25, Sequence
  27, Class 12.
- Smaller buckets: Timeline 3, Journey 2.
- Closed in M15RV-040: C4 15 -> 0.
- Closed in M15RV-050: ER 3 -> 0, Sankey 3 -> 0, Timeline 7 -> 3.
- Closed in M15RV-085: Sequence 67 -> 63 unaccepted residuals in the full all-diagram root policy
  run, with the four largest HTML `<br>` / wrap rows now root-exact.
- Closed in M15RV-087: Sequence 63 -> 27 unaccepted residuals in the full all-diagram root policy
  run, with actor-type height rows and six stale Sequence root pins removed from the residual set.
- Closed in M15RV-088: Architecture fixture corpus is refreshed to Mermaid 11.15, diagram-scoped
  IDs and fallback background paths now come from source rules, all Architecture root pins were
  deleted, and stale groups-within-groups calibration was removed. The Architecture bucket is now
  32 honest rows rather than 30 rows mixed with stale baseline/pin artifacts.
- In progress in M15RV-089: the first source-backed Architecture work reduced the bucket from
  `32` to `29` after deleting the non-source junction group inference and mirroring Mermaid's
  endpoint traversal order for group alignment overwrites. Later HPD-050 root-bounds work reduced
  the active family-level Architecture queue to `25` mismatches. The custom-init group-padding row
  improved from about `-22.5px` to about `-2.5px` after switching group rect sizing from the old
  `iconSize / 2` proxy to source-derived `architecture.padding`. Rust also now preserves Mermaid's
  duplicate queued-position BFS behavior for Architecture relative constraints; this aligns the
  `junction_fork_join` FCoSE input with the browser probe but does not reduce its remaining
  `+13.976px` tail.
  HPD-050 later narrowed this statement: renderer-side pre-layout group bbox center calculation was
  removed because it was not consumed by layout. Manatee owns relocation/element bbox policy; the
  renderer now feeds only per-node `BoundsExtras` into manatee and keeps final SVG group rect
  padding under a separately named helper.
  Batch5, html-title/escape, unicode/xml, nested-group, group-port, and custom-init rows now have
  focused diagnostic evidence; none justify a renderer-side one-off metric or root pin. Batch4
  small-icons, fontsize-wrap, edge-label corner, fan-in/out, deep-nesting, multi-split junctions,
  and disconnected-islands rows are no longer active Architecture root tails in the fresh family
  report. The last full all-diagram root policy run reported 134 unaccepted residuals with a clean
  11.15 baseline and no Architecture root pins; refresh that all-family number before M15RV-090
  policy closeout.

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
- After M15RV-088, the stored Architecture fixture corpus is refreshed to Mermaid 11.15 and the
  renderer emits scoped IDs directly. Do not re-add compare-time ID/path normalization or
  Architecture root pins unless a future fixture-source audit proves the corpus has mixed
  vintages again.
- The Architecture root override table is intentionally empty. A non-zero Architecture override
  count is a regression unless backed by generated fixture-derived evidence and a workstream
  decision.
- Class still has a tempting large pair of residuals:
  `upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037`
  and
  `upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037`.
  Fresh 2026-06-02 focused inspection shows these are not structure/viewBox-rule bugs; they are
  `htmlLabels=true` class title `foreignObject` width underestimates, concentrated in the long
  punctuation and foreign-language labels (`C12` / `C13`). A temporary experiment proved that
  adding 12 rendered-width lookup rows to `class_text_overrides_11_12_2.rs` makes both fixtures
  root-exact, but `report-overrides --check-no-growth` then fails because the text lookup count
  grows from `495` to `507`. Do not hand-add those rows as a local win. The right follow-up is a
  Class text-override stale-table audit and/or a generated Class HTML width evidence path that can
  replace old rows instead of growing the hand-curated table.
- A quick stale-table audit on 2026-06-02 re-confirmed that the obvious short-label candidates are
  not low-hanging fruit. The current fixtures/tests still reference entries such as `Docs`,
  `Cool`, `uses`, `API`, `DB`, and `Server`, and the historical refactor notes already record that
  several seemingly simple deletions (`User`, `test()`, `+handle(...)`, `+query(...)`,
  `+request(...)`) caused focused geometry drift or golden churn. Continue this audit only with
  focused evidence and an explicit delete-one-verify-one loop; do not bulk-prune the table based on
  text search alone.
- The first delete-one-verify-one pass on 2026-06-02 proved one real stale row: removing the
  rendered-width override `(16, true, "User") => Some(33.765625)` reduced the Class text lookup
  budget from `495` to `494` without adding any new Class `parity-root` failures or changing the
  existing 14-row Class residual set. Focused checks for `upstream_pkgtests_classdiagram_spec_003`
  and `upstream_html_demos_classchart_class_diagram_demos_010` stayed at their pre-existing
  `499.75px -> 499.5px` tails, and the full Class root report remained the same 14 fixtures. Keep
  this row deleted. Continue the stale-table audit in the same explicit one-row-at-a-time style for
  the remaining candidates (`test()`, `+handle(...)`, `+query(...)`, `+request(...)`) instead of
  treating the whole historical caution note as a permanent stop sign.
- The second delete-one-verify-one pass on 2026-06-02 proved another stale pair: removing
  `test()` from both `lookup_class_calc_text_width_px` and
  `lookup_class_rendered_width_px` reduced the Class text lookup budget from `494` to `492`
  without changing the focused `upstream_cypress_classdiagram_spec_should_handle_an_empty_class_body_with_empty_braces_029`
  tail (`200.75px -> 201.25px`) and without adding any new Class `parity-root` failures. The full
  Class root report still remained the same 14 fixtures. Keep those `test()` rows deleted. The
  remaining audited-next candidates are now `+handle(...)`, `+query(...)`, and
  `+request(...)`.
- A follow-up grouped deletion experiment on 2026-06-02 showed that the three remaining method
  pairs are not yet pure stale rows. Removing `+handle(req: Request) : Response`,
  `+query(sql: String) : Rows`, and `+request() : Response` did reduce the raw lookup budget to
  `486`, but it swapped one Class residual rather than preserving the current set: the full
  `parity-root` report introduced `stress_class_styles_multiple_classdef_016`
  (`890.25px -> 890.5px`) while dropping another row, so the residual count stayed at 14 but the
  membership changed. That is not good enough for the stale-table lane. Keep those six method rows
  for now, and revisit them only with finer-grained one-row-at-a-time probes or generated evidence
  that explains the style-definition interaction.
- A 2026-06-04 finer-grained pass narrowed that caution note. In the current table, `+query(...)`
  and `+request(...)` are no longer present. The calc-width `+handle(req: Request) : Response`
  row is stale and should stay deleted: removing only
  `(16, "+handle(req: Request) : Response") => Some(221)` preserves the full 14-fixture Class
  root residual membership and keeps `stress_class_styles_multiple_classdef_016` root-green. The
  rendered-width `+handle(...)` row is not stale: removing
  `(16, false, "+handle(req: Request) : Response") => Some(240.375)` reintroduces
  `stress_class_styles_multiple_classdef_016` (`890.25px -> 890.5px`). Keep that rendered row
  unless a generated Class HTML width evidence path replaces it.
- Architecture junction group membership must come from `junction.in` only. Do not infer group
  parents from neighboring services; Mermaid 11.15 does not do that in `addJunctions(...)`.
- Architecture `groupAlignments` must be generated in the same endpoint traversal order as
  `ArchitectureDB.getDataStructures()`: walk model nodes, then each node's incident/service edge
  list, and allow later endpoint traversal to overwrite earlier group-pair alignment values. Do
  not simplify this back to a single global edge pass.
- Architecture group rectangle sizing must use configured `architecture.padding`, not
  `iconSize / 2`, even though those happen to match at the default `iconSize=80`, `padding=40`.
  Keep the remaining 0.5-2.5px Cytoscape/browser bbox lattice as diagnostic unless broad generated
  evidence justifies a reusable headless approximation.
- The same `architecture.padding` rule applies to the pre-layout compound bbox approximation fed
  into FCoSE. Do not use `iconSize / 2` as a padding proxy in either layout input or final SVG group
  rectangles.
- Do not re-add the tested `+1px` Architecture bbox edge adjustment without broad evidence; it
  made two additional root rows fail.
- Architecture relative placement BFS must process duplicate queued current positions on pop, like
  Mermaid `getRelativeConstraints(...)`. Do not simplify it back to a visited-on-pop skip; that
  drops duplicate constraints such as `join -> db` and `join -> cache` in the fork/join fixture.
- `stress_architecture_junction_fork_join_026` matched the old saved Mermaid debug probe at
  service-position level, while the stored upstream SVG is reproducible by the current CLI/Edge
  path. A refreshed HPD-050 probe remains diagnostic-only and still does not reproduce the CLI
  fixture. Do not tune manatee against the debug probe until the probe harness / CLI harness
  disagreement is resolved.
- Do not tune `ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE` against a single residual. Batch5
  long labels and batch4 small-icon labels need different treatment. The current piecewise
  long-label branch (`>= 200px -> 1.01`) is already committed because it improved the targeted
  batch5 row without moving the nearby matrix rows; further changes still need broader generated
  browser-probe evidence or an honestly documented residual class.
- Do not claim `reasonable_height` is root-green. Fresh 2026-06-02 focused parity-root checks still
  show the three `reasonable_height` fixtures at `+0.380px` width / tiny height rounding tails
  (`1859.440px -> 1859.820px` max-width), so they remain part of the honest Architecture residual
  set unless a source-compatible rounding rule closes them for the whole family.
- Do not treat `stress_architecture_html_titles_and_escapes_041` as an HTML/entity or edge-label
  bug. Focused evidence shows the root tail is group-rect / Cytoscape service bbox dominated.
- Do not tune group-edge shifts from `stress_architecture_group_port_edges_017`; its constraints,
  service bboxes, and Mermaid source force policy already match. This row is now root-exact after
  the strict `RectangleD.intersects(...)` follow-up restored source-positive-gap clipping.
- Do not treat `stress_architecture_unicode_and_xml_escapes_019` as an entity-escaping bug; its
  residual is group/service bbox width.
- Do not alter Architecture edge-label wrapping from `stress_architecture_edge_label_corner_cases_012`
  or `stress_architecture_batch4_init_fontsize_wrap_063`; focused SVG text splitting already
  matches upstream. These rows are now root-green after the HPD-050 `createText()` y-range
  root-bounds fix, so further work should not tune their wrapping or text scale.
- Do not tune nested-group padding from `stress_architecture_nested_groups_002`; current evidence
  points to small FCoSE/compound-bound drift after source inputs match.
- Fresh 2026-06-05 Architecture reports supersede the older 29-row and 25-row M15RV-089 queues:
  structural parity is green and Architecture `parity-root` has `20` mismatches after the HPD-050
  multiline-title and strict-intersects fixes. Do not continue from
  `stress_architecture_batch4_init_small_icons_061`,
  `stress_architecture_batch4_init_fontsize_wrap_063`, `stress_architecture_edge_label_corner_cases_012`,
  `stress_architecture_fan_in_out_021`, `stress_architecture_deep_nesting_013`,
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`,
  `stress_architecture_group_port_edges_017`, or
  `stress_architecture_disconnected_islands_046` unless a fresh report regresses. The live larger
  Architecture audit queue is `batch5_long_titles_and_punct_076`,
  `html_titles_and_escapes_041`, `unicode_and_xml_escapes_019`,
  `batch6_init_fontsize_icon_size_wrap_093`, and `nested_groups_002`, with smaller browser/Cytoscape
  bbox lattice rows remaining diagnostic unless a reusable source rule appears.
- For Sequence wrap work, keep the distinction between final emitted SVG text evidence and
  incremental wrap probes. Exact SVG evidence may only short-circuit wrapping when the full string
  demonstrably fits; it should not become a general prefix-width replacement.
- Main closeout recheck on 2026-06-05 passed:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run --no-fail-fast` ran `1857` tests with
  `1857` passed and `5` skipped after the integrated root-report coverage and Class measurement
  no-growth cleanup.
