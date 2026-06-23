#import "@preview/merman:0.1.0": mermaid-context, mermaid-svg

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  A[Document font] --> B[Context wrapper]
"

#let explicit-svg = mermaid-svg(source, id: "typst-explicit-font", pipeline: "readable")
#assert(
  not explicit-svg.contains("Arial"),
  message: "plain mermaid-svg must stay explicit-only and not inherit document font",
)

#let context-image = mermaid-context(
  source,
  id: "typst-context-font",
  width: 100%,
)

#context-image

Context bridge fixture passed.
