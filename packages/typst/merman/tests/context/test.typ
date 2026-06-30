#import "@preview/merman:0.1.0": mermaid, mermaid-svg

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  A[Document font] --> B[Context render]
"

#let explicit-svg = mermaid-svg(source, id: "context-explicit", pipeline: "readable")
#assert(explicit-svg.contains("context-explicit"), message: "explicit render should use direct options")
#assert(not explicit-svg.contains("Arial"), message: "default render must not inherit Typst document font")
#assert(not explicit-svg.contains("13pt"), message: "default render must not inherit Typst text size")

#let direct-svg = mermaid-svg(
  source,
  id: "context-direct",
  pipeline: "readable",
  typography: (font: "Explicit Sans", size: "18px"),
  viewport-width: 444,
)
#assert(direct-svg.contains("Explicit Sans"), message: "direct typography should be usable without context")
#assert(not direct-svg.contains("Arial"), message: "SVG export should remain explicit-only")

#mermaid(
  source,
  document-context: true,
  id: "context-enabled",
  pipeline: "readable",
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-direct-width",
  pipeline: "readable",
  viewport-width: 444,
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-direct-layout",
  pipeline: "readable",
  layout: (viewport_width: 333),
  width: 100%,
)

Context fixture passed.
