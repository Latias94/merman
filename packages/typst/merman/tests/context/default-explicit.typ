#import "@preview/merman:0.1.0": mermaid-svg

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  A[Document font] --> B[Default render]
"

#let svg = mermaid-svg(source, id: "default-explicit", pipeline: "readable")
#assert(svg.contains("default-explicit"), message: "explicit render should use direct options")
#assert(not svg.contains("Arial"), message: "default render must not inherit Typst document font")
#assert(not svg.contains("13pt"), message: "default render must not inherit Typst document text size")

Default explicit fixture passed.
