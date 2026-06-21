#import "@preview/merman:0.1.0": mermaid-context, show-mermaid-blocks-context

#set page(width: 16cm, margin: 18mm)
#set text(font: "Arial", size: 13pt)

#show raw.where(lang: "mermaid"): show-mermaid-blocks-context(
  width: 100%,
  pipeline: "readable",
)

= Document context example

#mermaid-context(
  "flowchart LR
    A[Document font] --> B[Context-aware wrapper]
    B --> C[Respects container width]
  ",
  width: 100%,
)

```mermaid
sequenceDiagram
  participant Typst
  participant merman
  Typst->>merman: Use the show-block wrapper
  merman-->>Typst: Render with the current document context
```
