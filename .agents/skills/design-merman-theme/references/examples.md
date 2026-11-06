# Theme mechanisms and delegated design

## Reuse real inputs

The [Playground theme library](../../../../playground/src/themes/) owns editable recipes, shared
English sample sources/configurations, and saved SVGs. Open a relevant recipe and its exact preview
before adapting it. These are standalone designs, not additional built-in preset IDs. Read current
files rather than caching the library's size or copying every design into this skill.

| Example | Distinct mechanism | Cross-family adaptation to study |
| --- | --- | --- |
| [Riso](../../../../playground/src/themes/recipes/riso.theme.json) · [Flowchart SVG](../../../../playground/src/themes/previews/riso-flowchart.svg) | Opposing independent hard offsets suggest blue/red registration passes; clear text remains single-pass. | Flowchart and Sequence bodies carry offsets; XY uses smaller bar offsets; Class and Pie keep identity through ink, type, and suitable tints. This is vector separation, not physical ink mixing. |
| [Pop Art](../../../../playground/src/themes/recipes/pop-art.theme.json) · [Sequence SVG](../../../../playground/src/themes/previews/pop-art-sequence.svg) | Bold contours, exposed magenta/black offsets, poster faces, and a tiled dot canvas. | Use a separate Pie palette for readable percentage labels; retain family-specific effect scope instead of attaching filters to every shape. |
| [Paper Cut](../../../../playground/src/themes/recipes/paper-cut.theme.json) · [Flowchart SVG](../../../../playground/src/themes/previews/paper-cut-flowchart.svg) | Independent hard backing sheets plus a softer contact shadow express paper thickness. | Actors and notes use different stacks; XY bars use reduced offsets; Class and Pie retain paint/type treatments without requesting the same body effect. |
| [Aurora](../../../../playground/src/themes/recipes/aurora.theme.json) · [XY SVG](../../../../playground/src/themes/previews/aurora-xychart.svg) | Gradient canvas, radial light ribbons, restrained panel halos, and sharp foreground lettering. | Keep unsupported node/sector gradients in the canvas; Class uses solid surfaces; narrow chart type gives legends room. Halos and transparency do not implement backdrop blur. |
| [Signal](../../../../playground/src/themes/recipes/signal.theme.json) · [Class SVG](../../../../playground/src/themes/previews/signal-class.svg) | Mounted sign plates, straight yellow lower flanges, condensed lettering, and perimeter warning tape. | Dense Class signatures use a measured monospace role; XY accents stay functional; [review.md](review.md#worked-failures-signal) explains the rejected bleed request and height-scaled rails. |

For a minimal complete example, use [starter.theme.json](../assets/starter.theme.json) with the
bundled [Flowchart source](../assets/flowchart.mmd). It intentionally has only three color tokens,
one independent hard offset, separate body/label effects, and basic-shape Flowchart rules. The accent
edits the backing card, while ink edits contours and text. It is a seed to extend, not a complete
cross-family design or an output-target qualification. [runtime.md](runtime.md) renders it as saved.

## A concrete parallel-design prompt

Use this when several independent designs are requested. Replace names and paths together if
they already exist. The coordinator owns integration; workers own disjoint design files. Reuse a
prepared runtime so parallel preview work does not start competing builds.

**Coordinator prompt:**

```text
Use $design-merman-theme to create two standalone examples: Stencil and Night Transit.
Prepare one matching CLI and share its absolute path/identity. Keep the five common sources
and configs in playground/src/themes/samples/ unchanged so the comparison is meaningful.

Spawn two workers with the worker prompt below:
- Worker A: id stencil; stencil-*.svg; target/theme-design/stencil/.
  Brief: utilitarian stencilled crates, hard backing edges, warm stock, clear annotations.
- Worker B: id night-transit; night-transit-*.svg; target/theme-design/night-transit/.
  Brief: luminous transport signage, dark panels, condensed names, precise route accents.

Each worker may write only its recipe, its five previews, and its temporary output directory.
I own catalog/locales/UI/docs integration. I will inspect the five-family comparisons, review
the reports against the final inputs, and rerender any inputs changed during integration.
Then independently review visual bounds and generated Rust/CLI/Web adoption. Register no new
preset IDs. Keep font and target limitations visible in the handoff.
```

**Worker prompt (instantiate once per worker):**

```text
Use $design-merman-theme for ID and BRIEF assigned above. You share this checkout with others.
Your only tracked write paths are:
  playground/src/themes/recipes/ID.theme.json
  playground/src/themes/previews/ID-{flowchart,sequence,class,xychart,pie}.svg
Temporary output: target/theme-design/ID/. Do not change shared sources/configs or other files.
Do not build, commit, or integrate UI. Use the supplied existing CLI and its fixed identity.

Start with a short material/structure/typography brief. Author an editable recipe with useful
shared relationships. Render all five exact saved samples with their configs and this recipe.
Use explicit checked native fonts for Class; record font files, order, and measurement mode.
Keep each SVG and report from the same execution. Inspect those exact SVGs and distinguish
runtime evidence from visual findings. Check Class compartments, XY legends, Sequence full
canvas, label contrast, and effect scope. Preserve unsupported or no-consumer probes under
your temporary directory; describe any design compromise rather than hiding a failure.

Return: changed paths; final recipe/input/runtime identity and exact invocations; reports;
five-family screenshots and visual findings; fonts; actual limits; preview colors and light/
dark classification for integration. Keep theme JSON, optional scene configuration, and
Rust/CLI/Web adoption responsibilities distinct. Stop after handing off your assigned files.
```

## Design-skill sources and adaptation

These primary sources informed the practices listed below. Same-render reports, screenshots,
font checks, and diagram bounds review are Merman's own execution practices. Revisit an external
source when extending its borrowed practice, not on every theme invocation.

| Primary source | Adopted | Boundary for diagrams |
| --- | --- | --- |
| Anthropic [frontend-design](https://github.com/anthropics/skills/blob/main/skills/frontend-design/SKILL.md) | Subject-specific visual direction, deliberate type roles, and one dominant visual expression. | Page heroes, CSS layout, motion, and taste-based bans on particular colors/fonts do not establish diagram quality. Preserve source meaning and host font constraints. |
| Anthropic [theme-factory](https://github.com/anthropics/skills/blob/main/skills/theme-factory/SKILL.md) | Let users see real examples and retain an editable design they can apply consistently. | A palette/font pair is insufficient for material effects and family coverage. Continue an authorized design brief without introducing a mandatory choice/confirmation loop. |
| Vercel [web-design-guidelines](https://github.com/vercel-labs/agent-skills/blob/main/skills/web-design-guidelines/SKILL.md) and its [source rules](https://github.com/vercel-labs/web-interface-guidelines/blob/main/command.md) | Review long content and report a concrete issue at its owning file/location. | Form controls, CSS truncation, routing, and DOM interaction rules belong to UI work. Diagram labels need full content and measured bounds, not hidden overflow or ellipses as a fit fix. |
