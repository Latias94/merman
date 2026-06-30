#import "@preview/merman:0.1.0": (
  mermaid,
  mermaid-figure,
  mermaid-profile,
  mermaid-result,
  mermaid-svg,
  show-mermaid-blocks,
  validate-mermaid,
)

#let source = "flowchart TD
  A[Current API] --> B[Surviving behavior]
"

#let result = mermaid-result(source, id: "current-surface-result", pipeline: "readable")
#assert(result.ok, message: "structured result should render successfully")
#assert.eq(result.code_name, "MERMAN_OK")
#assert(result.svg.contains("current-surface-result"), message: "structured result should use renderer options")

#let svg = mermaid-svg(source, id: "current-surface-svg", pipeline: "readable")
#assert(svg.contains("<svg"), message: "SVG export should return SVG text")
#assert(svg.contains("current-surface-svg"), message: "SVG export should use renderer options")

#let validation = validate-mermaid(source)
#assert.eq(validation.code_name, "MERMAN_OK")

#let profile = mermaid-profile(
  id: "current-surface-profile",
  pipeline: "readable",
  typography: (font: "Current Surface Sans", size: "18px"),
)

#let profiled-svg = mermaid-svg(source, profile: profile)
#assert(profiled-svg.contains("current-surface-profile"), message: "profile should apply to SVG export")
#assert(profiled-svg.contains("Current Surface Sans"), message: "profile typography should apply to SVG export")

#mermaid(source, profile: profile, width: 80%, alt: "Current API diagram")

#mermaid-figure(
  source,
  profile: profile,
  caption: [Current API figure],
  width: 80%,
)

#show raw.where(lang: "mermaid"): show-mermaid-blocks(
  profile: profile,
  width: 80%,
  error-mode: "panic",
)

```mermaid
flowchart LR
  Raw[Raw block] --> Handler[Show handler]
```

Current surface fixture passed.
