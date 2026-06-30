#import "@preview/merman:0.1.0": show-mermaid-blocks

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#show raw.where(lang: "mermaid-explicit"): show-mermaid-blocks(
  width: 100%,
  pipeline: "readable",
  error-mode: "panic",
)

#show raw.where(lang: "mermaid-docctx"): show-mermaid-blocks(
  document-context: true,
  width: 100%,
  pipeline: "readable",
  error-mode: "panic",
)

```mermaid-explicit
flowchart TD
  Explicit[Explicit raw block] --> Rendered[Rendered image]
```

```mermaid-docctx
sequenceDiagram
  participant Typst
  participant merman
  Typst->>merman: Render this raw block with document context
  merman-->>Typst: Return SVG bytes
```

Raw block fixture passed.
