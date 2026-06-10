#import "../lib.typ": mermaid, mermaid-svg, validate-mermaid

= merman Typst Options Example

#let source = "flowchart LR
  Start([Start]) --> Parse[Parse]
  Parse --> Render[Render SVG]
  Render --> Done([Done])
"

#let validation = validate-mermaid(source)

Validation result: `#validation.code_name`

#mermaid(
  source,
  width: 95%,
  id: "typst-options-demo",
  scale: 1.05,
  background: "#f8fafc",
  theme-name: "base",
  theme: (
    primaryColor: "#0f172a",
    primaryTextColor: "#f8fafc",
    primaryBorderColor: "#38bdf8",
    lineColor: "#475569",
  ),
)

#let svg = mermaid-svg(source, pipeline: "readable")

SVG starts with:

```text
#svg.slice(0, 80)
```

Validation failures can stay visible in drafts:

#mermaid(
  "flowchart TD\n  A -->",
  width: 95%,
  error-mode: "placeholder",
)
