#import "@preview/merman:0.1.0": mermaid

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  A[Document font] --> B[Context render]
"

#mermaid(
  source,
  document-context: true,
  id: "opt-in-context",
  pipeline: "readable",
  width: 100%,
)

Opt-in context fixture passed.
