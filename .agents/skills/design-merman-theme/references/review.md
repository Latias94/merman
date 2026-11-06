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

An exit code of zero, an empty diagnostic array, or theme `verified` does not establish visual fit
or portability. BestEffort can return SVG while its target is rejected. Query unfamiliar facets
using the exact family/target/facet, then test a focused recipe on a scene containing that target.
Local font-stack support does not follow from local font-size or fill support. Current gaps live
beside the [theme guide's examples](../../../../docs/rendering/custom-diagram-themes.md#standalone-explorations-and-example-scenes).

## Representative scenes

Use the user's diagram first. For general design work select three to five families that exercise
different consumers; a Playground library addition uses all five saved
[sources and configurations](../../../../playground/src/themes/samples/). Keep those inputs common
across competing designs. Add a targeted dense or long scene only when its mechanism needs it.

| Family | Representative content | Inspect before calling it visually reviewed |
| --- | --- | --- |
| Flowchart | Branches, a cluster, mixed basic shapes, edge labels | Body vs label effects; label backings; marker tips; cluster hierarchy; source style precedence; material at ordinary density when requested. |
| Sequence | Actors, activations, notes, a loop, bottom lifelines | The entire scene lies within viewBox and authored canvas, including lifeline strokes when `mirrorActors: false`; note/actor effects keep labels legible. |
| Class | Namespaces, methods, long signatures, multiplicities | Fonts match measured roles, including signature monospace; text boxes fit their own compartments as well as the root viewport. |
| XY Chart | Bar and line series, titles, axes, visible legend | Actual legend text and swatches fit their reserved regions; series order and legend associations survive appearance changes; ticks and endpoints remain clear; effects preserve series distinctions. |
| Pie | Several unequal sectors, title, percentages, legend | Neighboring sectors remain distinct; sector order and category/legend associations survive appearance changes; labels contrast against every actual sector and canvas; small sectors and legends remain readable. |

For a light/dark pair, compare the same diagram with source/configuration, look, layout, and
category order held fixed. Reuse the scenes above; add a dense case only for a concrete concern.
Review at the intended reading scale: preserve semantic roles and multiple useful hues while
adapting luminance and chroma. Compare text with its actual composited background, including
group panels, notes, activations, and label backings. Check connectors and arrowheads against
adjacent surfaces in both modes. Muted text still needs readable contrast; a coordinated accent
ramp alone does not establish a categorical palette. A pair of swatches or a viewer backing
switch does not establish a pair of rendered appearances.

Keep the actual family and expected labels in browser checks. Wait for the changed render to
finish before inspecting it. A family loop or hash change is not proof that the viewer replaced
its previous SVG; strings found in `<style>` are not diagram labels.

## Inspect bounds and fonts

Load the intended fonts before browser measurement (`document.fonts.ready` where available).
Read computed font families and text boxes from the rendered elements; check local compartments,
legend reservations, and the root canvas in a common coordinate system. Account for transforms,
strokes, and visible effects rather than comparing unrelated screen and SVG coordinates.

For font-sensitive Class scenes, use native measurement with explicit, checked font files and
system discovery off; [runtime.md](runtime.md#native-rendering-and-fonts) has a reproducible example.
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
