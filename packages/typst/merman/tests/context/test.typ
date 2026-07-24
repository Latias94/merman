#import "@preview/merman:0.2.0": mermaid, mermaid-svg
#import "../../src/options.typ": config-with-context-width, context-host-theme, mermaid-profile, render-config
#import "../../src/units.typ": context-width-css-px, typst-length-to-css-px

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#assert.eq(typst-length-to-css-px(72pt), 96)
#assert.eq(context-width-css-px(float.inf * 1pt), none)
#assert.eq(context-host-theme("Arial", 12pt).font_size, "16px")
#assert.eq(
  context-host-theme((name: "Inria Serif", covers: "latin-in-cjk"), 12pt).font_family,
  "Inria Serif",
)
#let inferred-width-config = config-with-context-width(render-config(), typst-length-to-css-px(72pt))
#assert.eq(inferred-width-config.binding_options.layout.container_width, 96)
#let profile-layout-width-config = config-with-context-width(
  render-config(profile: mermaid-profile(layout: (container_width: 333))),
  typst-length-to-css-px(72pt),
)
#assert.eq(
  profile-layout-width-config.binding_options.layout.container_width,
  333,
  message: "profile layout width should take precedence over document context width",
)
#assert.eq(
  context-host-theme(
    ((name: "Inria Serif", covers: "latin-in-cjk"), "Noto Serif CJK SC"),
    12pt,
  ).font_family,
  "Inria Serif, Noto Serif CJK SC",
)

#let source = "flowchart LR
  A[Document font] --> B[Context render]
"

#let explicit-svg = mermaid-svg(source, id: "context-explicit", pipeline: "readable")
#assert(explicit-svg.contains("context-explicit"), message: "explicit render should use direct options")
#assert(not explicit-svg.contains("Arial"), message: "default render must not inherit Typst document font")
#assert(not explicit-svg.contains("17.333"), message: "default render must not inherit Typst text size")

#let direct-svg = mermaid-svg(
  source,
  id: "context-direct",
  pipeline: "readable",
  typography: (font: "Explicit Sans", size: "18px"),
  container-width: 444,
)
#assert(direct-svg.contains("Explicit Sans"), message: "direct typography should be usable without context")
#assert(not direct-svg.contains("Arial"), message: "SVG export should remain explicit-only")

#let typst-length-svg = mermaid-svg(
  source,
  id: "context-typst-length",
  pipeline: "readable",
  typography: (font: "Typst Length Sans", size: 12pt),
)
#assert(typst-length-svg.contains("16px"), message: "12pt typography must become 16 CSS px")
#assert(not typst-length-svg.contains("12pt"), message: "Typst point units must not leak into SVG layout")

#mermaid(
  source,
  document-context: true,
  id: "context-enabled",
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-direct-width",
  container-width: 444,
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-direct-layout",
  layout: (container_width: 333),
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-profile-layout",
  profile: mermaid-profile(layout: (container_width: 333)),
  width: 100%,
)

Context fixture passed.
