#import "@preview/merman:0.1.0": show-mermaid-blocks-context

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#show raw.where(lang: "mermaid"): show-mermaid-blocks-context(
  width: 100%,
  pipeline: "readable",
  error-mode: "panic",
)

```mermaid
sequenceDiagram
  participant Typst
  participant merman
  Typst->>merman: Render this raw block
  merman-->>Typst: Return SVG bytes
```

Raw block context fixture passed.
