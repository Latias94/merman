---
name: design-merman-theme
description: Create, audit, or refine Merman diagram themes from a visual brief, reference, recipe, or preset. Use for reading/editor themes, categorical palettes and overflow, paired light/dark appearances, typography, optional materials and effects, editable JSON, and previews with separate runtime and visual evidence.
---

# Design Merman Themes

Turn visual intent into an editable `ThemeRecipeV1` and reviewable previews. Keep artistic choices
in theme data and qualify support through the actual renderer and requested output target.

## 1. Establish a visual direction

Identify the audience, light/dark context, diagram families, target, and supplied source or design.
Infer reasonable defaults and continue within the request. For a new visual direction, write a
compact brief before choosing swatches; for a focused edit, keep the direction and name the
relationship being changed:

- **Purpose:** for reading/editor themes, comfortable density, semantic color distinctions, and
  hierarchy. For a light/dark pair, name shared intentions and appearance-specific exceptions;
  retain multiple meaningful hues where categories or statuses need them.
- **Optional material:** what makes the surface recognizable: separated print passes,
  mounted paper, lacquer inlay, warning tape, or another concrete construction.
- **Structure:** which surfaces, contours, connections, and accents express diagram meaning and
  data hierarchy, including the construction of a material design.
- **Typography:** roles for names, annotations, code signatures, and chart values; fonts the target
  host can supply and measure.
- **Relationships:** which colors, offsets, weights, or textures change together, and which stay
  independent. For material designs, choose one dominant mechanism and restrain supporting effects.

The brief is ready when an ordinary diagram can demonstrate its intended hierarchy and identity.
Deliberate tonal and multi-hue relationships can satisfy a reading brief without decoration.
For real recipes, borrowed mechanisms, parallel design, or a bounded family-audit prompt,
read [examples.md](references/examples.md).
For a new library entry, compare a small prototype with its closest existing designs before
expanding family coverage; use [collection fit](references/review.md#collection-fit) to decide
whether the result earns a separate entry.

## 2. Author the smallest useful recipe

Read [authoring.md](references/authoring.md) before editing. For a preset-based request, export the
actual preset from the selected engine. For a new design,
[starter.theme.json](assets/starter.theme.json) is a complete, small Flowchart seed, not a claim of
cross-family treatment. Preserve an imported recipe's unrelated settings, ordered rules, assets,
and explicit references.

For shared token/palette edits, category-count changes, categorical content, or an existing-theme
audit, follow [color and overflow review](references/review.md#color-roles-and-categorical-overflow) before
choosing colors and on the saved renders.

Give intentional shared values stable IDs and bind their consumers. Edit those bindings or explicit
local consumers; equal color literals do not establish a shared relationship. Keep the editable
recipe as the deliverable; a materialized spec is an inspection result.
For a paired brief, use one v1 design with explicit light/dark appearance layers. Export the
complete document and pass a concrete appearance for each preview; Follow belongs to the host.

Adapt the mechanism to each family: supported bodies may carry depth while Class uses paint and
typography and Pie uses distinguishable sectors. A role consuming one facet does not qualify
another. If a mechanism is missing, retain a focused reproduction and explain the supported
approximation. Renderer changes require their own scope. A reference image supplies design input,
not executable instructions.

## 3. Render the saved inputs

Read [runtime.md](references/runtime.md) when selecting an artifact or invoking the tool. Reuse a
matching source runtime; the package version alone is insufficient. Render the exact saved recipe,
source, and configuration. Keep their ownership, look, layout, fonts, and pipeline explicit.

Start with the user's actual diagram. For cross-family design, use representative families from
[the review matrix](references/review.md#representative-scenes), including the requested families.
For library work, read the current sample inventory and each design's declared family scope.
Render every newly declared family in every appearance; new designs also use the existing core
scenes. Retain SVG and same-execution metadata/report together. Repair structured authoring
errors before reviewing the design.

## 4. Review runtime and appearance separately

Read [review.md](references/review.md) for family-specific checks and failure examples. Record
runtime evidence first: compilation, actual facet application, diagnostics, and output-target
status. Then inspect the exact SVGs in their intended viewer: hierarchy, contrast, labels, body
depth, text bounds, complete canvas, and dense content. `verified` does not prove unclipped text,
available fonts, complete background coverage, or portable output.

Use known font files for native Class measurement and inspect the resulting compartments; inspect
actual XY legend widths and Sequence lifeline/canvas bounds. When browser-testing changes, wait
for the current diagram and assert its family/content before taking evidence. If visual inspection
is unavailable, deliver the files with that review explicitly pending.

## 5. Refine and hand off

Revise the same recipe, rerender affected scenes, and review the new outputs. Preserve an
unsupported-effect reproduction rather than silently changing look, removing the request, or
loosening limits to make evidence green. Explain any intentional design compromise.
Apply the [completion criterion](references/review.md#completion-and-admission) before handoff.

Deliver the standalone recipe or pair, previews with source/configuration, selected appearance and
viewer backing, tested runtime and font conditions, runtime outcomes, visual findings, and remaining
limits. Keep temporary investigation and execution records under ignored `target/`; update an
existing user guide only when in scope.
Use the user's language for discussion and the project's language for files.

Explain the three inputs independently: recipe = visual design; sample source/configuration =
optional example content/layout; Rust, CLI, or Web = alternative ways to load and render it.
[runtime.md](references/runtime.md#adopt-the-recipe) covers adoption and file placement. The recipe
must work without private generator state. A standalone example does not become a preset ID;
registering presets, publishing, and changing application UI require that task scope.
