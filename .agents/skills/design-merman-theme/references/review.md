# Review theme execution and appearance

## Establish what ran

Keep the saved source, recipe, configuration, invocation, font files/measurement mode, pipeline,
and runtime identity with each SVG. For a small experiment, the existing CLI/Node reports and a
short local note are sufficient; use an existing manifest format when one already owns the work.
Temporary reports, probes, and screenshots belong under ignored `target/`.

Read `metadata.theme_execution_evidence` from the SVG operation, or the native CLI
`--theme-report` from the same invocation. Distinguish:

| Observation | What it establishes |
| --- | --- |
| JSON parses or recipe compiles | The request was accepted as authoring input. |
| Catalog/support query | Declared scope or coarse mechanism support, not this scene's applied styling. |
| Theme evidence and diagnostics | Whether the execution accounted for requested styling; retain residual, unknown, and no-consumer outcomes. |
| Target status and reasons | Admission for the named output context. SVG theme verification can coexist with target `unverified`. |
| Inspection of the actual SVG | Observed hierarchy, readability, paint, bounds, and effects in that viewer. |
| Preset availability and qualification | Declared scope and issued qualification; availability alone does not establish admission or one render's theme/target status. |

An exit code of zero, an empty diagnostic array, or theme `verified` does not establish visual fit
or portability. BestEffort can return SVG while its target is rejected. Query unfamiliar facets
using the exact family/target/facet, then test a focused recipe on a scene containing that target.
Local font-stack support does not follow from local font-size or fill support. Current gaps live
beside the [theme guide's examples](../../../../docs/rendering/custom-diagram-themes.md#standalone-explorations-and-example-scenes).

## Collection fit

When extending the Playground collection, compare the candidate and its closest existing designs
on the same ordinary scenes at full size and at gallery thumbnail size. Hide names and material
descriptions for one pass. Describe the visible difference in surfaces, contours, connection
hierarchy, typography, or effects; a material name does not establish a new rendering mechanism.
An attractive palette variation can be useful for a requested reading context, but does not by
itself establish a distinct artistic addition to an already populated collection.

Start with a small representative prototype before rendering every declared family. If the
candidate's differences disappear at the intended scale or depend on sample-specific content,
refine an existing design or keep the candidate under ignored `target/` instead of adding a
catalog entry. When the user permits declining additions, no new theme is a valid design outcome.

## Representative scenes

Use the user's diagram first. For general design work select families that exercise different
consumers. For Playground work, read the current [core scene inventory](../../../../playground/src/themes/families.ts),
[design scope](../../../../playground/src/themes/catalog.ts), and saved
[sources and configurations](../../../../playground/src/themes/samples/). New designs use the core
scenes plus their declared additions; an existing design's extension renders the new families.
Rerender existing scenes when shared tokens, palettes, or compatibility values change their
inputs. Keep sources common across designs. Categorical scope requires the
[palette matrix](#required-categorical-matrix); add other dense or long scenes only when a
concrete mechanism needs them.

| Family | Representative content | Inspect before calling it visually reviewed |
| --- | --- | --- |
| Flowchart | Branches, a cluster, mixed basic shapes, edge labels | Body vs label effects; connector departures and marker tips above body depth; label backings; cluster hierarchy; source style precedence; material at ordinary density when requested. |
| Sequence | Actors, activations, notes, a loop, bottom lifelines | The entire scene lies within viewBox and authored canvas, including lifeline strokes when `mirrorActors: false`; actor depth keeps lifeline junctions visible; note/actor effects keep labels legible. |
| Class | Namespaces, methods, long signatures, multiplicities | Fonts match measured roles, including signature monospace; text boxes fit their own compartments as well as the root viewport. |
| XY Chart | Bar and line series, titles, axes, visible legend | Actual legend text and swatches fit their reserved regions; ticks and endpoints remain clear; effects preserve series distinctions. Apply categorical review to series, not the number of x-axis values. |
| Pie | Several unequal sectors, title, percentages, legend | Apply categorical review to actual sectors, their labels, and the matching legend, including circular adjacency. |

For additional families, inspect their actual semantic roles before selecting rules. Planning
scenes need task states, columns, or phase distinctions; architecture scenes need services and
group boundaries; data scenes need category/legend associations, axes, overlap, and labels on
small regions. Requirement relationships and Tree View highlights have their own consumers.
The [coverage guide](../../../../docs/rendering/diagram-theme-coverage.md) owns current mechanism
limits. A global palette or canvas change alone does not qualify a dedicated family treatment.

Compatibility scalars can affect other families: Kanban and Mindmap may share `cScale` values,
while Git Graph and the Mindmap root share `git0`. Trace the winning consumer before selecting
such a fallback and rerender affected existing examples. Inspect actual group stacking as well:
an Architecture group painted after its services can obscure them when given an opaque fill.

For material effects, inspect a connection entering and leaving the affected body. Depth belongs
behind connections while the actual surface keeps its foreground position. Check a translucent
body as well: the depth filter's final output must omit the surface, which is painted once.
Observe alpha and color in overlapping depth/surface pixels under the current layer semantics;
matching filter stages alone does not prove equivalence to a combined filtered body. Source
class filters must retain ownership without disabling a depth carrier and exposing another face.
For background-dependent label defaults, inspect a supported body-fill change without an explicit
text-color rule; explicit ink in a library recipe can hide a stale family fallback.

For a light/dark pair, compare the same diagram with source/configuration, look, layout, and
category order held fixed. Reuse the required scenes above and the categorical matrix when applicable.
Review at the intended reading scale: preserve semantic roles and multiple useful hues while
adapting luminance and chroma. Compare text with its actual composited background, including
group panels, notes, activations, and label backings. Check connectors and arrowheads against
adjacent surfaces in both modes. Muted text still needs readable contrast. A pair of swatches or
a viewer backing switch does not establish a pair of rendered appearances.

Keep the actual family and expected labels in browser checks. Wait for the changed render to
finish before inspecting it. A family loop or hash change is not proof that the viewer replaced
its previous SVG; strings found in `<style>` are not diagram labels.
For a comparison sheet, isolate each SVG in an image or its own document, or render with unique
diagram IDs. Repeated root/style/definition IDs in one inline document can recolor other samples.

## Color roles and categorical overflow

For shared token/palette edits, category-count changes, categorical content, or an existing-theme
audit, first classify each color role. Start with the requested recipes/families and current
catalog declarations, then follow changed shared colors into affected family consumers.

| Role | Design intent |
| --- | --- |
| Categorical | Distinguish unordered identities; preserve category-to-color and legend matching. |
| Sequential | Encode an ordered magnitude or rank with a ramp; related tones serve that order. |
| Emphasis or status | Highlight selected content or named meanings against quieter context; repeated neutrals can be intentional. |

When shared tokens, palettes, or compatibility colors change, trace their consumers across
families: **fill → label ink/backing** on nodes, sections, and tasks; **series → legend swatch/text**
on charts; **opacity/blend/canvas → actual composite**; **body/stroke/depth → connections and
arrowheads**. Inspect affected normal scenes in each appearance, including text on colored fills,
overlapping flows/surfaces, and connections entering or leaving bodies. For Merman-owned alpha
themes, redesign unsuitable roles and bindings together rather than preserving a weak legacy
palette; keep source/data meaning and exact Mermaid defaults intact.

An accent ramp does not supply distinct unordered categories. Count actual color-domain categories
or series as `N`; use raw mark or axis counts only when they are that domain. Record palette length `k`,
any winning rule cycle, category order, and source/config/rule ownership from the saved inputs,
materialized recipe, and actual family consumer. Repeated entries can make the number of distinct
colors smaller than `k`. Verify these finite contracts in the current sources:

| Consumer | Contract and boundary |
| --- | --- |
| Typed ordinal palette | [OrdinalPalette and OrdinalSelector](../../../../crates/merman-render/src/diagram_theme/semantic.rs): palettes contain 1–256 entries and select one-based index `i` with `(i - 1) % k`. Cyclic rules use `(i - 1) % period == offset`, with `period > 0` and `0 <= offset < period`. Family support and winning rules still govern actual paint. |
| Pie | [Pie paint plan](../../../../crates/merman-render/src/pie/theme.rs) cycles `pie1`–`pie12` by section-label identity and shares that paint with legend swatches; a consumed typed `pie-slice` palette has its own `k`. [Visible-sector selection](../../../../crates/merman-render/src/pie.rs) filters percentages that round to `0%`, while legend entries can remain. Check actual ordering and visible sectors. |
| XY Chart | [Plot palette](../../../../crates/merman-render/src/chart_palette.rs) cycles the ordered nonempty entries of `plotColorPalette`; current named-theme fallbacks have 10 entries. [Series paint](../../../../crates/merman-render/src/xychart/series.rs) assigns colors by plot/series, not by x-axis category; a consumed typed `chart-series` palette uses its own `k`. |
| Radar | [Series adapter](../../../../crates/merman-render/src/radar/theme.rs) has 12 `cScale` slots. Typed palette fallback beyond slot 12, or a changed `THEME_COLOR_LIMIT`, can leave an unsupported-palette residual; generic palette capacity does not extend this adapter. |
| Treemap | [Paint domains](../../../../crates/merman-render/src/treemap/theme/node_visual.rs) prepend a transparent root slot to the [12 body/peer slots](../../../../crates/merman-render/src/treemap/css_binding.rs), giving a 13-slot cycle; labels use an independent 12-slot domain. Typed Node colors have a separate ordinal path. |

For other families, trace the actual status, branch, group, or series consumer before treating
its colors as categories. For exact Mermaid Default parity, use the selected upstream from the
[source lock](../../../../tools/upstreams/REPOS.lock.json) and preserve its palette, ordering,
cycling, and small-value semantics. Record readability residuals separately. A custom Merman
recipe can make an explicit supported design choice without changing that parity baseline.

### Required categorical matrix

Use normal scenes for every affected family; apply count stress only to actual categorical scope.
Cover distinct winning cycles and consumer semantics with a compact set of representative probes,
reusing a probe when it covers multiple rows. Compare themes on common normal scenes. Hold category
identities, order, source/config, look, and layout fixed across themes and appearances; use controlled
additional sources for the counts below. For a pair, run each applicable scene in both light and
dark; keep single-appearance reviews scoped to their declared appearance.

| Scene | Required input | Inspect |
| --- | --- | --- |
| Ordinary | User's actual chart or the current shared family sample | Normal hierarchy, category/legend matching, and intended reading scale. |
| Palette boundary | `N = k - 1` (when positive), `k`, and `k + 1` for each distinct effective palette/cycle | The first repeat, earlier duplicate colors, and actual adjacency. For Pie with all `k + 1` sectors visible under an unoverridden `k`-cycle, last and first sectors reuse the same slot and touch. |
| Overflow | A count above 12 and a representative `N = 20` | Repeated colors, legend disambiguation, crowding, and category identity at larger counts; 20 can also satisfy the above-12 case. |
| Dense or small values | Unequal values, small regions/series, long labels; include zero or filtered values where the family supports them | Every actual label and boundary, fitted/omitted regions, retained legend entries, and their mapping. |

If a required count exceeds a consumer's capacity, retain the actual rejection or residual and
mark that coverage unsupported. Preserve the user's data and runtime limits. A finite cycle
cannot promise unlimited distinct colors. Evaluate any generated or hue-interpolated extension
for actual category separation and text contrast through this matrix; interpolation alone is
not a readability fix. Supported labels, separators, strokes, or markers can supplement color.
Semantic grouping or an `Other` category requires the user's explicit data choice.

Inspect repeats and all neighboring pairs, including Pie's last-to-first boundary. Check the
label/percentage on every actual rendered sector, including late and repeated colors, against
its composited fill, opacity, effects, and receiving background. Pie defaults to `pieOpacity = 0.7`
in its [CSS binding](../../../../crates/merman-render/src/pie/theme/css_binding.rs); use the actual
rendered composite for contrast, not an opaque recipe swatch. Check legend and outside text
against their own composites. One swatch or the first few sectors cannot qualify a shared ink.
Confirm each category's actual paint matches its legend entry and keeps its identity across
appearances, including small/filtered values; do not reorder categories to hide a collision.

## Inspect bounds and fonts

Load the intended fonts before browser measurement (`document.fonts.ready` where available).
Resolve the actual displayed face as well as the computed stack: a browser can select an installed
user font omitted from a closed native context. Chromium's `CSS.getPlatformFontsForNode` reports
the used face; include its checked file when comparing native geometry with that viewer. Computed
`font-family` alone does not prove the same face was measured. Read text boxes from the rendered
elements; check local compartments, legend reservations, and the root canvas in a common coordinate
system. Account for transforms,
strokes, and visible effects rather than comparing unrelated screen and SVG coordinates.
For variable fonts, verify the selected weight/axis instance; loading the same file can still
measure its default instance while the browser paints another weight. Use matching static faces
for a controlled comparison when the native stack cannot select the required instance.
Check HTML text against its `foreignObject` as well as the enclosing node. A root-bounds pass can
miss clipped card labels. Treat text-box overflows as candidates; confirm visible glyph loss or
painted containment before changing geometry, since font boxes can extend beyond painted pixels.
When a measured label is normalized to its box origin, place that box in its reserved compartment;
raw SVG helper offsets apply only to the corresponding raw text recipe. A centered layout box
alone does not establish that the painted text is centered.

For native Class previews, use native measurement with explicit, checked font files and system
discovery off; [runtime.md](runtime.md#native-rendering-and-fonts) has a reproducible example.
Browser previews use the actual host measurement path and loaded viewer fonts; record that
condition separately rather than describing browser measurement as native qualification.
Record any fallback. Library examples include Riso with Avenir + Menlo, Carbon Copy with Menlo +
Avenir, Metro with Helvetica Neue, and Signal with Menlo + Avenir Next Condensed. Resolve file
paths on the current host rather than making these macOS font names a portability requirement.

Deterministic XY legend widths can differ from browser shaping, even when theme evidence is
`verified`. Inspect actual legend bounds at intended text sizes and in a dense scene. If the
renderer reserves too little room, keep a reproduction and name the limitation; arbitrary sample
padding or a smaller font does not establish a general renderer fix.

Review the exact exported SVG independently of viewer backing. For an explicit theme canvas,
check all painted bounds, especially Sequence's lower lifelines and notes. An unthemed renderer
viewport or a page background is not evidence that exported canvas paint covers the scene.
For transparent or translucent output, record the receiving background and inspect that composite
in each intended appearance. Keep exported canvas, family-owned backdrops, root/output background
policy, and viewer backing distinct; a light/dark catalog label does not establish actual output.

## Completion and admission

A theme review is complete when every in-scope recipe, family, and appearance has its required
saved scenes, matching runtime reports, and inspection of the exact output. For categorical
scope, include the source-backed role, `N`, effective `k`/cycles, category/legend mapping, and all
matrix outcomes. For an existing-theme optimization, rerender changed cases and affected normal
scenes from the cross-family trace; identify what improved and any remaining repeat, adjacency,
label, or connection limitation.

Report four conclusions separately: **runtime correctness** (winning consumers and diagnostics),
**visual readability** (observed labels, distinctions, and bounds at the tested counts),
**portability** (actual targets, fonts, viewer/backing conditions and target admission), and
**preset admission** (current catalog scope and the
[qualification owner's requirements](../../../../docs/rendering/preset-qualification.md)). A
correct finite cycle can still be visually ambiguous; a good preview does not qualify a preset
or an export target. Pending, rejected, or unsupported checks remain explicit gaps and prevent
an unqualified completion claim; a completed audit may conclude that a theme needs correction.

## Worked failures: Signal

The initial Signal design requested `canvas.bleed = {top:16,right:10,bottom:16,left:10}`. All five
renders accepted the recipe and exited successfully, but the reports contained canvas `no-consumer`
residuals, unchanged viewBoxes, and rejected target evidence. That was an accepted field without
an active consumer. The final recipe intentionally omitted bleed and kept warning tape inside
the viewport. Preserve the initial recipe/reports locally and explain this compromise; do not
claim bleed worked or that the renderer was fixed.

The final [Signal recipe](../../../../playground/src/themes/recipes/signal.theme.json) uses a
repeating diagonal stripe layer with an 18px period, covered by an opaque center layer. Its
gradient stops at `0.008` and `0.992` reveal rails at 0.8% of viewport height. The stripe period
is fixed in pixels; the revealed rail thickness scales with diagram height. Check a tall scene
before reuse. This composition is not a fixed-thickness, region-anchored border feature.

When evidence and visible output disagree, retain both. In a prior Flowchart run, local
`edge-label.fill` reported verified while the observed ink remained unchanged. Using shared Text
was a design alternative, not proof the local facet was repaired. Recheck the current runtime
before carrying that finding forward, and report mismatches at their owner rather than filtering
them out of diagnostics.
