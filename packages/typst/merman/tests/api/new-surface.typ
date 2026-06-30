#import "@preview/merman:0.1.0": (
  mermaid,
  mermaid-figure,
  mermaid-profile,
  mermaid-result,
  mermaid-svg,
  merman-capabilities,
  show-mermaid-blocks,
  validate-mermaid,
)

#let source = "flowchart TD
  A[Canonical API] --> B[Shared renderer path]
"

#let profile = mermaid-profile(
  id: "new-surface-profile",
  pipeline: "readable",
  typography: (font: "New Surface Sans", size: "17px"),
  figure: (placement: bottom, outlined: false),
)

#let result = mermaid-result(source, profile: profile)
#assert(result.ok, message: "structured result should render through canonical surface")
#assert(result.svg.contains("new-surface-profile"), message: "profile should reach structured result")

#let svg = mermaid-svg(source, profile: profile)
#assert(svg.contains("New Surface Sans"), message: "profile should reach SVG export")

#let validation = validate-mermaid(source, profile: profile)
#assert.eq(validation.code_name, "MERMAN_OK")

#let capabilities = merman-capabilities()
#assert(capabilities.render, message: "capabilities should stay exported")
#assert(capabilities.text_measurement.vendored, message: "capabilities should keep measurement boundary")

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#mermaid(
  source,
  document-context: true,
  profile: profile,
  width: 100%,
  alt: "Canonical context image",
)

#mermaid-figure(
  source,
  document-context: true,
  profile: profile,
  caption: [Canonical context figure],
  width: 100%,
)

#show raw.where(lang: "mermaid"): show-mermaid-blocks(
  document-context: true,
  profile: profile,
  width: 100%,
  error-mode: "panic",
)

```mermaid
flowchart LR
  Raw[Raw block] --> Canonical[Canonical show rule]
```

New surface fixture passed.
