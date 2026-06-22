#import "@preview/merman:0.1.0": mermaid-result, mermaid-svg

#let source = "flowchart TD
  A[Write Mermaid] --> B[Render with merman]
"

#let result = mermaid-result(source, id: "typst-basic-smoke", pipeline: "readable")
#assert(result.ok, message: "expected basic render to succeed")
#assert.eq(result.code_name, "MERMAN_OK")
#assert(result.svg.contains("<svg"), message: "expected SVG output")
#assert(result.svg.contains("Write Mermaid"), message: "expected source label in SVG")

#let svg = mermaid-svg(source, id: "typst-basic-smoke-svg", pipeline: "readable")
#assert(svg.contains("typst-basic-smoke-svg"), message: "expected stable diagram id")

Basic smoke fixture passed.
