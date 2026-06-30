#import "@preview/merman:0.1.0": mermaid, mermaid-svg

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  A[Context] --> B[Precedence]
"

#let explicit-svg = mermaid-svg(
  source,
  id: "context-precedence-explicit",
  pipeline: "readable",
  typography: (font: "Explicit Sans", size: "18px"),
  viewport-width: 444,
)

#assert(explicit-svg.contains("Explicit Sans"), message: "direct typography should be usable without context")
#assert(not explicit-svg.contains("Arial"), message: "SVG export should remain explicit-only")

#mermaid(
  source,
  document-context: true,
  id: "context-precedence-direct-width",
  pipeline: "readable",
  viewport-width: 444,
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-precedence-direct-layout",
  pipeline: "readable",
  layout: (viewport_width: 333),
  width: 100%,
)

Context precedence fixture passed.
