#import "../lib.typ": mermaid-raw

#show raw.where(lang: "mermaid"): block => mermaid-raw(
  block,
  width: 100%,
  pipeline: "readable",
  error-mode: "placeholder",
  alt: "A Mermaid diagram rendered from a raw block",
)

= merman Typst Raw Block Example

```mermaid
sequenceDiagram
  participant User
  participant Typst
  participant merman
  User->>Typst: Write a mermaid raw block
  Typst->>merman: Call the wasm plugin
  merman-->>Typst: Return SVG bytes
```
