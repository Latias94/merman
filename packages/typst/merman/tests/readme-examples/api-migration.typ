#import "@preview/merman:0.1.0": mermaid, show-mermaid-blocks

#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  Old[Old context wrapper] --> New[document-context parameter]
"

#mermaid(
  source,
  document-context: true,
  width: 100%,
  alt: "Migrated context render",
)

#show raw.where(lang: "mermaid"): show-mermaid-blocks(
  document-context: true,
  width: 100%,
  error-mode: "panic",
)

```mermaid
sequenceDiagram
  participant Old
  participant New
  Old->>New: show-mermaid-blocks-context(...)
  New-->>Old: show-mermaid-blocks(document-context: true, ...)
```

API migration fixture passed.
