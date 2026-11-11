#import "@preview/merman:0.5.0": (
  analyze-mermaid,
  export-theme-preset,
  materialize-theme,
  mermaid,
  mermaid-figure,
  mermaid-profile,
  mermaid-result,
  mermaid-svg,
  mermaid-theme-definition,
  show-mermaid-blocks,
)
#import "../../src/options.typ": render-config
#import "../../src/render.typ": resolve-theme-render-config, render-svg-result-with-config, with-document-render-config

#set page(width: 10in, margin: 0pt)
#set text(font: "Arial", size: 15pt)

#let source = "flowchart LR\nA[Theme selection] --> B[Rendered]"
#let dark-profile = mermaid-profile(theme-preset: "editor", theme-appearance: "dark")
#let dark-svg = mermaid-svg(source, profile: dark-profile, id: "paired", pipeline: "readable")
#assert.eq(
  dark-svg,
  mermaid-svg(source, theme-preset: "editor-dark", id: "paired", pipeline: "readable"),
  message: "a grouped preset appearance should render the corresponding concrete preset",
)
#let light-svg = mermaid-svg(
  source, profile: dark-profile, theme-appearance: "light", id: "paired", pipeline: "readable",
)
#assert(light-svg != dark-svg, message: "changing only appearance should change the rendered palette")
#assert.eq(
  light-svg,
  mermaid-svg(source, theme-preset: "editor-light", id: "paired", pipeline: "readable"),
)

#let typography-svg = mermaid-svg(
  source,
  profile: dark-profile,
  typography: (font: "Preset Typography Sans", size: 18),
)
#assert(typography-svg.contains("Preset Typography Sans"))
#assert(typography-svg.contains("18px"))

#let themed-analysis = analyze-mermaid(
  source, profile: dark-profile, typography: (font: "Analysis Sans", size: 18),
)
#assert.eq(themed-analysis.version, 1)
#assert(themed-analysis.valid, message: "analysis should accept preset appearance plus typography")
#assert.eq(themed-analysis.summary.errors, 0)

#let paired-recipe = export-theme-preset("editor")
#assert.eq(
  mermaid-svg(
    source, theme-recipe: paired-recipe, theme-appearance: "dark", id: "paired", pipeline: "readable",
  ),
  dark-svg,
  message: "an exported recipe should render through the ordinary selection path",
)
#let recipe-typography-svg = mermaid-svg(
  source,
  theme-recipe: paired-recipe,
  theme-appearance: "dark",
  typography: (font: "Recipe Typography Sans", size: 19),
)
#assert(recipe-typography-svg.contains("Recipe Typography Sans"))
#assert(recipe-typography-svg.contains("19px"))

#let complete-recipe = (
  schema_version: 1,
  kind: "complete_spec",
  complete_spec: (typography: (default: (font_weight: 600))),
)
#let complete-result = mermaid-result(
  source,
  theme-recipe: complete-recipe,
  typography: (font: "Complete Recipe Sans", size: 21),
)
#assert(complete-result.ok, message: "complete_spec recipes should compose with typography")
#assert(complete-result.svg.contains("Complete Recipe Sans"))
#assert(complete-result.svg.contains("21px"))

#context {
  with-document-render-config(
    config => {
      let result = render-svg-result-with-config(source, config)
      assert(result.ok, message: "a preset should render with real Typst document context")
      assert(result.svg.contains("arial"), message: "document font should reach the preset SVG")
      assert(result.svg.contains("20px"), message: "15pt should become 20 CSS px")
      assert.eq(config.binding_options.theme.appearance, "dark")
      assert.eq(config.binding_options.layout.container_width, 960)
    },
    profile: dark-profile,
  )
}
#mermaid(source, profile: dark-profile, document-context: true, width: 90%)
#mermaid-figure(
  source,
  profile: dark-profile,
  theme-recipe: paired-recipe,
  theme-appearance: "light",
  document-context: true,
  caption: [Recipe-selected figure],
  width: 90%,
)

#show raw.where(lang: "mermaid-themed"): show-mermaid-blocks(
  profile: mermaid-profile(theme-recipe: paired-recipe, theme-appearance: "light"),
  theme-appearance: "dark",
  document-context: true,
  error-mode: "panic",
)
```mermaid-themed
sequenceDiagram
  Document->>Theme: Select the dark appearance
  Theme-->>Document: Preserve document typography
```

#for preset-id in ("paper-cut", "carbon-copy", "pop-art") {
  let recipe = export-theme-preset(preset-id)
  assert.eq(recipe.schema_version, 1)
  assert.eq(recipe.kind, "design")
  let inspection = materialize-theme((schema_version: 1, action: "inspect", recipe: recipe))
  assert.eq(inspection.supported_appearances, ("light",))
  let result = mermaid-result(source, theme-preset: preset-id, theme-appearance: "light")
  assert(result.ok, message: preset-id + " should render under the Typst resource ceiling")
  let unavailable = mermaid-result(source, theme-preset: preset-id, theme-appearance: "dark")
  assert(not unavailable.ok, message: "Light-only art presets must reject Dark")
  mermaid(source, theme-preset: preset-id, document-context: true, width: 90%)
}

// Editing one public shared value updates every retained font reference.
#let custom = export-theme-preset("paper-cut")
#let tokens = custom.design.tokens
#let body-font = tokens.at("body-font")
#body-font.insert("value", ("Curated Test Sans", "Arial", "sans-serif"))
#tokens.insert("body-font", body-font)
#let design = custom.design
#design.insert("tokens", tokens)
#custom.insert("design", design)
#let saved-json = json.encode(custom, pretty: true)
#let imported = json(bytes(saved-json))
#assert.eq(imported, custom, message: "the full editable configuration should round-trip as JSON")
#let custom-svg = mermaid-svg(
  source, theme-recipe: imported, typography: (font: "Default Only Sans", size: 22),
)
#assert(custom-svg.contains("Curated Test Sans"), message: "family font references should use the edited token")
#let resolved = resolve-theme-render-config(render-config(
  theme-recipe: imported,
  typography: (font: "Default Only Sans", size: 22),
))
#assert.eq(resolved.binding_options.theme.spec.typography.default.font_stack, ("Default Only Sans",))
#assert.eq(
  resolved.binding_options.theme.spec.typography.families.flowchart.font_stack,
  ("Curated Test Sans", "Arial", "sans-serif"),
  message: "document/default typography must not erase authored family fonts",
)
#mermaid(source, theme-recipe: imported, document-context: true, width: 90%)
#mermaid-theme-definition(
  source,
  (schema_version: 1, action: "materialize", recipe: imported, appearance: "light"),
  document-context: true,
  width: 90%,
)

#let invalid-recipe = (
  schema_version: 1,
  kind: "design",
  design: (styles: (node: (typography: (font_stack: ())))),
)
#let expected-error = materialize-theme((schema_version: 1, action: "materialize", recipe: invalid-recipe))
#let failed = mermaid-result(source, theme-recipe: invalid-recipe, typography: (font: "Error Sans"))
#assert(not failed.ok)
#assert.eq(failed.code_name, expected-error.code_name)
#assert.eq(failed.operation, expected-error.operation)
#assert.eq(failed.kind, expected-error.kind)
#assert.eq(failed.details, expected-error.details, message: "authoring diagnostic details must survive composition")
#assert.eq(failed.svg, none)
#let failed-analysis = analyze-mermaid(
  source, theme-recipe: invalid-recipe, typography: (font: "Analysis Error Sans"),
)
#assert(not failed-analysis.ok)
#assert.eq(failed-analysis.operation, expected-error.operation)
#assert.eq(failed-analysis.code_name, expected-error.code_name)
#assert.eq(failed-analysis.details, expected-error.details, message: "analysis must retain theme materialization failures")
#mermaid(source, theme-recipe: invalid-recipe, document-context: true, error-mode: "placeholder")

#let incompatible = mermaid-result(
  source, theme-preset: "editor-dark", theme-appearance: "light", typography: (font: "Mismatch Sans"),
)
#assert(not incompatible.ok, message: "materialization must not turn a concrete dark preset into Light")
#let unknown = mermaid-result(source, theme-preset: "unknown-typst-preset", typography: (font: "Error Sans"))
#assert(not unknown.ok, message: "preset export failures should remain structured render results")
#assert.eq(unknown.operation, "export-theme-preset-json")
#assert("details" in unknown, message: "mermaid-result should keep its optional details field")

#let opaque = (version: 3, theme: (preset: "editor-dark"), svg: (diagram_id: "opaque", pipeline: "readable"))
#assert.eq(
  mermaid-svg(source, options: opaque),
  mermaid-svg(
    source,
    options: opaque,
    theme-preset: "ignored",
    theme-recipe: imported,
    theme-appearance: "ignored",
    typography: (font: "Ignored Sans"),
  ),
  message: "opaque options must bypass both selection and typography materialization",
)
#context {
  with-document-render-config(
    config => assert.eq(
      render-svg-result-with-config(source, config).svg,
      mermaid-svg(source, options: opaque),
      message: "opaque raw options must also bypass inferred document typography",
    ),
    options: opaque,
    theme-preset: "ignored",
    theme-appearance: "ignored",
  )
}

Theme selection fixture passed.
