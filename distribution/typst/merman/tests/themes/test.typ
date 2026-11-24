#import "@preview/merman:0.6.0": (
  analyze-mermaid,
  export-theme-preset,
  mermaid,
  mermaid-profile,
  mermaid-result,
  mermaid-svg,
  merman-capabilities,
  resolve-theme,
  theme-catalog,
)
#import "../../src/render.typ": render-svg-result-with-config, with-document-render-config

#set text(font: "Libertinus Serif", size: 12pt)

#let source = "flowchart LR\nA[Theme] --> B[Appearance]"
#let svg = mermaid-svg.with(id: "theme-v2", pipeline: "readable")
#let catalog = theme-catalog()
#assert.eq(catalog.schema_version, 2)
#assert.eq(catalog.presets.len(), 13)
#assert(catalog.mermaid_themes.contains("forest"))
#assert(catalog.families.any(family => family.id == "flowchart"))
#assert.eq(merman-capabilities().metadata_ids, ("theme-catalog",))

#let studio = export-theme-preset("studio")
#assert.eq(studio.schema_version, 2)
#assert.eq(studio.id, "studio")
#let dark = resolve-theme(studio, appearance: "dark", auto-fix-contrast: true)
#assert(dark.ok)
#assert.eq(dark.resolved.appearance, "dark")
#assert.eq(type(dark.notices), array)
#let invalid = resolve-theme((schema_version: 2, extends: "studio", typography: (base_size_px: "large")))
#assert(not invalid.ok)
#assert.eq(invalid.path, "typography.base_size_px")
#assert(invalid.line > 0 and invalid.column > 0)

#let dark-profile = mermaid-profile(theme: "studio", appearance: "dark")
#let dark-svg = svg(source, profile: dark-profile)
#let light-svg = svg(source, profile: dark-profile, appearance: "light")
#assert(dark-svg != light-svg)
#assert.eq(dark-svg, svg(source, theme: studio, appearance: "dark"))
#assert.eq(light-svg, svg(source, theme: "studio", appearance: "light"))
#assert.eq(svg(source, theme: "forest"), svg(source, site-config: (theme: "forest")))
#assert.eq(svg(source, profile: dark-profile, theme: "forest"), svg(source, theme: "forest"))

#let opaque = (version: 3, theme: (preset: "studio", appearance: "dark"), svg: (diagram_id: "opaque", pipeline: "readable"))
#assert.eq(svg(source, options: opaque), svg(source, options: opaque, theme: "ignored", appearance: "ignored"))

#let custom = (schema_version: 2, extends: "studio", seed: (accent: "#0052CC"))
#assert(analyze-mermaid(source, theme: custom, typography: (font: "Arial")).valid)
#let background = "#123ABC"
#let opaque-svg = svg(source, theme: custom, background: background)
#let canvas = output => output.split("class=\"merman-theme-canvas\"").at(1).split("</g>").first()
#assert(canvas(opaque-svg).contains("fill=\"" + background + "\""))
#assert(not canvas(svg(source, theme: custom, background: "transparent")).contains("<rect"))
#assert(canvas(svg(source, theme: "forest", background: background)).contains("fill=\"" + background + "\""))

#with-document-render-config(
  config => {
    let result = render-svg-result-with-config(source, config)
    assert(result.ok, message: repr(result))
    assert(lower(result.svg).contains("libertinus serif"))
    assert(result.svg.contains("font-size:16px"))
  },
  theme: custom,
  appearance: "dark",
  pipeline: "readable",
)

#let unknown = mermaid-result(source, theme: "unknown-typst-preset")
#assert(not unknown.ok)
#assert.eq(unknown.operation, "render-svg")
#assert.eq(unknown.svg, none)
#mermaid(source, theme: "unknown-typst-preset", document-context: true, error-mode: "placeholder")

Theme fixture passed.
