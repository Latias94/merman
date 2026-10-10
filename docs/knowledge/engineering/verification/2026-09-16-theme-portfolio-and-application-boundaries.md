# Theme Portfolio and Application Boundaries

Date: 2026-09-16. Repository source baseline: `8f17d2c4f`.
Reference: `repo-ref/modern_mermaid` at `a021cbce37fc0b07a9f4791c28e983101ea06f2d`.
This analysis extends the [product boundary plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md) beyond its first Cyberpunk sample.
It does not claim that Merman implements all reference themes or that any theme is suitable for every diagram.

## Findings That Change the Design

1. A theme has a shared visual identity, but its useful realization is family-specific. The reference has neutral palettes, detailed family styling, data-series palettes, paper or grid canvases, hard shadows, neon effects and simulated materials. A universal palette-only recipe cannot reproduce all of them.
2. Reference family coverage is uneven. Spotless has substantial Flowchart/Sequence/XY rules but only selected State/Gantt text styling. Hand Drawn's Class/ER rules are largely typography; Grafana concentrates on Flowchart/Sequence/XY. A listed theme is not a full family adaptation.
3. Diagram semantics constrain decoration. A reply line, cardinality marker, Gantt task state and numeric series are not interchangeable decoration slots. Source CSS sometimes changes these indiscriminately; Merman must preserve their distinctions while applying style.
4. Technical support, visual suitability and measured qualification answer different questions. Do not use `available`, a supported matrix entry or a successful render as a recommendation or a complete-style guarantee.
5. Broad reference review and selective product delivery are compatible. Analyze all 24 themes, but implement and qualify deliberate theme/family/target combinations. Do not generate 24 incomplete catalog entries or require every combination to pass before a useful preset can exist.

## Reference Portfolio

All line references below refer to the pinned `src/utils/themes.ts`.
“Dedicated scope” means source declarations explicitly address that family; it is not a claim that every selector matched or that all mechanisms were implemented.
Generic `.node`/`.edge` styling does not establish support for all families.
Suitability and cautions are design assessments to guide sample selection, not accessibility or usability certification.

| Reference theme | Identity and important mechanisms | Dedicated source scope; partial styling | Candidate use and caution |
| --- | --- | --- | --- |
| linearLight (18–61) | Neutral light surfaces, fine lines, dotted canvas, muted XY palette | Generic node/edge/cluster; explicit XY | Documents and restrained charts; verify many-category distinction |
| linearDark (62–106) | Dark zinc surfaces, dotted canvas, cool muted XY palette | Generic node/edge; explicit XY | Dark applications; requires its dark backdrop, not a print preset |
| notion (107–208) | Slate surfaces, modest corners, yellow notes, system typography | Flowchart, Sequence, XY | Technical documents and protocols; not a strong decorative identity |
| cyberpunk (209–395) | Cyan/magenta neon, chained glow, navy grid/radial canvas | Flowchart, Sequence, XY | Sparse technical presentations; glow can interfere with dense labels and print |
| monochrome (396–439) | Black/white, gray series and distinct line dashes | Generic node/edge/cluster; explicit XY | Print and grayscale candidates; not universal accessibility proof |
| ghibli (440–625) | Cream paper, brown lines, white borderless cards, soft shadows | Flowchart, Sequence, XY; fixed-node-ID accent | Narrative and teaching; fine boundaries and fixed-ID assumptions need correction |
| spotless (626–912) | Manual-like ink, uppercase text, tight corners, paper grid | Flowchart, Sequence, XY; State/Gantt selected text | Instructions and process diagrams; preserve case-sensitive identifiers and long labels |
| brutalist (913–1326) | Heavy outlines, hard offset shadows, strong accents, ordinal color | Flowchart, Sequence, Class, State, ER, Gantt, Pie, XY | Posters and small explanations; dense entity tables need restraint |
| glassmorphism (1327–1724) | Translucent cards, colored shadows, gradient backdrop | Flowchart, Sequence, Class, State, ER, Gantt, Pie, XY | Controlled presentation backgrounds; simulated glass, not true backdrop sampling |
| softPop (1725–1955) | Soft colored cards, shape-dependent colors, shadows, monospace | Flowchart, Sequence, XY; Gantt/Pie/Git/ER/State/Class text patches | Teaching and light flows; shape coloring and blanket message dashes must not alter meaning |
| darkMinimal (1956–2058) | Dark surfaces, restrained palette, bold/dashed edges | Generic node/edge; explicit XY; Preview also rewrites paths | Sparse dark diagrams; do not import the host's blanket dash replacement |
| wireframe (2059–2257) | Gray outlines, grid, restrained corners and gray/dashed series | Generic node/edge/cluster; Sequence, XY | Drafts and structural discussion; category distinction remains necessary |
| handDrawn (2258–2559) | Paper/ink, handwriting, host-injected irregular-line filter | Flowchart, Sequence, XY; Class/State/ER/Gantt/Pie/Git mainly text | Sketches and teaching; font and effect requirements are separate capabilities |
| grafana (2560–2841) | Dark monitoring panels, blue/orange/green series, fine grid | Flowchart, Sequence, XY; State panel/text, Gantt/Pie text | Monitoring and system diagrams; no complete Class/ER design implied |
| memphis (2842–3262) | Heavy black lines, hard shadows, vivid ordinal colors, geometric texture | Flowchart, Sequence, State, Class, ER, Gantt, Pie, XY | Posters and small explainers; texture and thickness can overwhelm dense information |
| noir (3263–3772) | Black/white, white glow, monospace, dark spotlight canvas | Flowchart, Sequence, Class, State, ER, Gantt, Pie, XY; limited Journey | Dark storytelling; limited hue encoding and bright halos constrain data density |
| material (3773–4287) | White surfaces, elevation shadows, few borders, restrained accents | Flowchart, Sequence, Class, State, ER, Gantt, Pie, Journey, XY | Product documentation and cards; borderless separation depends on actual shadow output |
| aurora (4288–4829) | Translucency, white text, soft light, colorful gradients, backdrop declarations | Flowchart, Sequence, Class, State, ER, Gantt, Pie, Journey, XY | Presentation overview; backdrop sampling remains a named residual and background matters |
| win95 (4830–5415) | Gray rectangular controls, bevel-like hard shadows, teal desktop | Flowchart, Sequence, Class, State, ER, Gantt, Pie, Journey, XY | Retro software explanation; decoration and desktop color are not general report defaults |
| doodle (5416–6095) | Pastels, thick outlines, large corners, hard shadows and ordinal color | Flowchart, Sequence, State, Class, ER, Gantt, Pie, Journey, XY, Git, Timeline | Teaching and informal planning; preserve task states and distinguish data from decorative cycling |
| organic (6096–6751) | Sage greens, soft corners/shadows, related hues, green gradient | Flowchart, Sequence, State, Class, ER, Gantt, Pie, Journey, XY, Git, Timeline | Calm narratives and planning; many series or warning levels need stronger discrimination |
| hightech (6752–7305) | Green/cyan glow, dark radial canvas, monospace, container halo | Flowchart, Sequence, Class, State, Gantt, Pie, XY, Git, Timeline, ER, Journey, Mindmap | Technical presentations; dense text and multi-category charts require scrutiny |
| kawaii (7306–7848) | Pink surfaces, thick rounded borders, soft shadows and gradients | Flowchart, Sequence, Class, State, Gantt, Pie, XY, ER, Journey, Mindmap, Git, Timeline | Informal teaching and personal plans; not an alarm or precision-data default |
| geometricCollage (7849–8473) | Shape-dependent and ordinal accents, flat geometry, subtle crosshatch | Flowchart, Sequence, State, Class, ER, Gantt, Pie, Git, XY, Timeline | Editorial diagrams and overviews; DOM ordering must not become business-category identity |

The reference fonts are requests, not embedded resources. Typography may contribute strongly to Hand Drawn, Win95 or monospace styles, but users must supply the chosen font or accept a documented fallback.
`annotationColors` belongs to the reference editing overlay, not the diagram's native Note role.

## Browser Probe and Its Limits

The three committed inputs under `crates/merman-theme-fixtures/fixtures/public-cyberpunk/` were rendered with every pinned reference recipe: **72/72 rendered without an exception**.
These inputs are derived/new diagnostic scenes, not falsely labeled verbatim reference examples.
The probe used Mermaid **11.17.2**, HeadlessChrome **151.0.7922.77**, a 1280×960 viewport at DPR 1, and capture-only Arial overrides.
External network requests were blocked; fonts were not downloaded or bundled.
This holds typography variation out of the style/selector comparison; it does not verify the reference's original font identity or native metrics.

The probe composes each recipe's Mermaid configuration with its `bgClass`/`bgStyle` canvas.
It deliberately does **not** reproduce the full Preview component's additional path rewriting, injected hand-drawn filter definitions, editing state or annotation overlay.
Consequently this is a recipe/DOM compatibility probe, not a complete preview-app reproduction or a Merman comparison.
Class, ER, Gantt, Pie, other families and PNG/PDF are not covered by this browser batch.

Raw SVGs, individual captures, three contact sheets, extracted recipe values and computed styles are in `target/bench/experiments/theme-portfolio-8f17d2c4f/`.
The capture script is retained there as `capture.cjs`; these are ignored experiment artifacts, not distributed product assets.
The source recipe SHA-256 is `9f1baf22457de1fbc5217437dc103840a14f15f2213eeaba45f91e306f6db92d`.
The capture script SHA-256 is `0f94a7d4344d63dcddfccc55b396be4b1402295438888cbbb5db0dab19ebd8c5`.
The result JSON SHA-256 is `89f0d4763f2863c41bf66c8f002d10f5586c688e649264ddb40a05ceb4b4817f`.

Specific observations change implementation expectations:

- Cyberpunk Flowchart's `.edgePath .path` and `.arrowheadPath` selectors matched zero terminals in this Mermaid version. Their authored intent and actual reference output must be recorded separately; implementing readable cyan edges/markers is a deliberate typed recipe decision, not pixel-copying an upstream failure.
- Cyberpunk Sequence's `.actor text` matched zero terminals, while four `text.actor` elements inherited the geometry-oriented `.actor` rule. Their computed weight was 400 with no text-shadow, and they received a 3px stroke and the actor filter. Merman should bind actor geometry and actor text separately.
- The interleaved XY scene emitted bar-0, line-1, bar-2, line-3. Cyberpunk's first three source suffix rules styled the first three series, but line-3 had 2px default stroke and no filter. This is a global declaration ordinal, not numbering independently per plot kind. The typed recipe must state and test cycling beyond its palette length rather than silently lose styling after the third series.

No visual completion percentage is derived from these 72 outputs. Small contact sheets also cannot establish text contrast at native size.

## Application Model

### Separate Three Questions

| Question | Authority | Meaning |
| --- | --- | --- |
| Can this recipe be compiled here? | Existing catalog availability and feature/resource policy | Compilation only; no diagram or export guarantee |
| Was this family deliberately styled, and for what use? | The preset's curated family design declaration and documentation | Design intent and recommendation; not target certification |
| Did this recipe produce the promised result on this target? | Existing family application report and artifact-bound qualification | Actual consumed facets and observed result, scoped to recipe/family/profile/target |

Do not add a generic scoring engine, automatic aesthetic classifier or second capability matrix.
Keep curated design scope with the existing preset descriptor; keep recommendations in documentation and consumer presentation.
Retain technical support and qualification in their existing owners.
A missing/unknown design declaration means unreviewed, not unsupported and not recommended by default.

### Shared Identity With Family Recipes

A recipe contains a shared base (appearance, neutral surface/text/line colors, typography defaults and canvas) plus explicit family-scoped styling.
Flowchart may use node surfaces and edge-label backgrounds; Sequence requires actors, message types, lifelines, notes and activation distinctions; Class/ER require readable rows and relationship markers; charts require data-series, axes and legend treatment; Gantt requires task-state distinctions.
There is no requirement for all these recipes to exist under every preset name.

Use the existing `family` selector on `ThemeRuleSetWireV1::Rule` for specialization.
An effect graph is reusable data; its application belongs to the winning scoped style rather than an unconditional global Node binding.
The current top-level effect binding and OrdinalPalette entries do not carry family scope. Do not broaden their use to simulate scoped application: use scoped rules with effect references and ordinal selectors, or simplify the unpublished representation if those paths reveal a real model gap.
This does not require a second family-dispatch subsystem inside the compiler.

Semantic invariants take priority over decoration: replies stay distinguishable from requests, cardinality/marker geometry is preserved, warning/task-state colors are not flattened into one surface color, category assignment is deterministic, and widths/corners do not erase diagram shapes.
Default categorical cycling follows the family's documented semantic series order, not DOM sibling order. Author-specified data colors retain ownership.
Sequential/diverging quantitative color scales, if not implemented, remain unsupported rather than being approximated by the preset's categorical list.

### Suitability Within a Family

Follow-up source and artifact review: 2026-09-17, Merman `405593f27`. The pinned reference revision is unchanged. This review reused the recorded reference captures; it did not rerun the browser batch or certify current Merman output.

Family scope is necessary but insufficient for a recommendation. A six-node Flowchart and a densely nested Flowchart share a renderer while imposing different legibility demands. A Class illustration with three short cards does not qualify a schema containing many members and relationship labels. Record these conditions in curated documentation and representative scenes, not in a new runtime suitability evaluator.

| Condition | What the review must establish | Deliberate application policy |
| --- | --- | --- |
| Dense labels and relationships | Member rows, edge-label backgrounds and cardinalities remain legible at the intended output size | Prefer restrained documented recipes; do not silently reduce effects based on node count |
| Multiple data series | Series remain identifiable beyond the first three, with matching legends and stable semantic order | Scope a categorical palette to its data roles; decorative node cycling is not a quantitative scale |
| Semantic line and state distinctions | Replies, relationship types, task states and source-owned warnings remain distinguishable | Preserve those distinctions even when the reference CSS flattens them |
| Dark, transparent or textured canvas | Text, surfaces and markers work against the actual composed background | A host-background override produces a modified recipe; do not reuse an opaque-canvas readability claim |
| Screen, print and native export | Effects, clipping, grayscale distinctions and fonts meet the declared target conditions | Recommend by intended medium; a browser screenshot is not a PNG/PDF qualification |

A deliberately supported family recipe must work for its declared scenes; weak visual suitability must not excuse a missing requested facet. Conversely, successful mechanism consumption does not establish readability. Review both structural correctness and the final image, keeping source intent, observed reference behavior and Merman's chosen behavior distinct.

The product should therefore offer a manageable portfolio of useful styles rather than maximize preset count. Retain the existing ten IDs, describe their actual scope, and use the non-Cyberpunk review slices below to decide which recipes deserve further investment. New public presets require a distinct useful role and representative evidence; matching a reference name is insufficient. Do not automatically forbid an unreviewed family, change a user's selected theme, or introduce a separate preset ID for every theme/family pair.

### Predictable Selection and Overrides

1. Parse the actual family and select the requested preset's shared base plus only that family's scoped rules.
2. Preserve source-owned facets and explicit caller modifications under the existing precedence contract. Clearing an effect does not clear unrelated fill/stroke; changing fill does not replace the whole style.
3. Render using the existing BestEffort/strict policy. Actual requested-but-unimplemented facets produce residuals or rejection as today; a deliberately absent family override is not invented as a request.
4. Report design scope separately. A family with only base styling is labeled as such; one with curated styling is described as designed for that family. Unknown scope remains unreviewed.
5. On a family switch, preserve the explicit preset selection and update its scope/explanation. Do not silently substitute a different preset or retain another family's effects. Users can still deliberately try an unreviewed combination.

`RequirePortable` keeps its target portability meaning. It must not be overloaded to mean “recommended visual style” or “identical to the reference.”
A successfully compiled preset or a base-only rendering therefore does not become a complete reference reproduction claim.

### Canvas and Host Overrides

The complete preset includes its intended canvas, but a user may explicitly choose transparency or another background using existing canvas configuration.
That creates a modified effective recipe and cannot reuse qualification for the original opaque recipe.
Check actual text/surface contrast against the chosen composition; where the host backdrop is unknown, report the limitation rather than certifying universal readability.
Do not auto-recolor every terminal as an undocumented response to a background override.
Preview UI chrome, interactive annotation and a container's outer decoration remain host-owned; diagram background layers belong in the exported canvas only when explicitly included in the recipe.

## Application to the Current Ten Presets

The seven editor palettes (`editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`, `gruvbox-dark`, `ayu-light`, `ayu-dark`) should be presented as broad base styles with the family refinements actually provided, not as reproductions of Modern Mermaid.
Brutalist, Spotless and Cyberpunk need separately declared designed-family scopes and visible recipe acceptance.
The current shared palette builder already contains family-specific exceptions (including Mindmap fill precedence and Kanban task-label contrast), which is evidence that a uniform all-family patch is not a valid general policy.

Do not remove a preset ID simply because it is not suitable for every family.
Do not add all 24 reference names to imply a completed portfolio.
Before expanding the catalog, use current presets and reference-only probes to cover different design demands:

| Review slice | Candidate representative | What it tests |
| --- | --- | --- |
| General document style | Editor Light/Dark and reference Notion | Dense labels, relationships, light/dark ownership |
| Instructions and structure | Public Spotless on Sequence/Flowchart | Text hierarchy, case-sensitive labels, line distinctions |
| Strong outlined cards | Public Brutalist on Class/ER probes | Rows, cardinality, hard-shadow bounds and dense readability |
| Data-focused styling | Reference Grafana/Monochrome on XY/Pie probes | Series identity beyond three colors, grayscale distinction, quantitative honesty |
| Neon presentation | Public Cyberpunk on Flowchart/Sequence/XY | Composed effects, text, markers, canvas and export |
| Font/material dependence | Reference Hand Drawn/Glassmorphism/Win95 | Font ownership, backdrop dependence and honest residuals |

These are deliberately selected review/probe pairs, not a promise to publish new presets or fully implement every reference mechanism in this tranche.
The three public named recipes require explicit conclusions: complete for a declared scene, bounded base styling, or unresolved product work. A successful file export alone cannot close them.
The resulting portfolio decisions feed recipe implementation and catalog presentation before final C7a candidate promotion.

## Existing Owners and Required Plan Changes

- `crates/merman-theme-contract/src/preset_catalog.rs`: `available` currently means compilation, and qualification is already family/output/profile scoped. Extend this existing descriptor only enough to convey curated family design scope; do not reuse qualification as a recommendation table.
- `crates/merman-render/src/diagram_theme/presets/catalog.rs`: the shared builder already has family-specific rules. Keep this owner for family recipes and complete-spec composition.
- `crates/merman-theme-contract/src/wire.rs`: ordinary rules already carry family/variant/ordinal scope. Prefer these over new selector syntax.
- `playground/src/components/ToolbarControls.tsx`: the picker currently maps only preset IDs. Present current-family scope and preserve an explicit selection when the family changes; keep unknown IDs observable.
- The product boundary plan gains a portfolio/application work unit ahead of scene-specific implementation, with non-Cyberpunk checks in the product verification tranche.

This research does not complete U1's Merman/native failure baseline, nor implement the optional font or runtime-certification boundaries.
