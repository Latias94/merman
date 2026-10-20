#import "@preview/merman:0.5.0": (
  describe-theme-support,
  export-theme-preset,
  materialize-theme,
  mermaid-theme-definition,
)

#let definition = (
  tokens: (
    canvas: (type: "color", value: "#0f172a"),
    surface: (type: "color", value: "#1e293b"),
    text: (type: "color", value: "#e5e7eb"),
    accent: (type: "color", value: "#38bdf8"),
  ),
  canvas: (base: (kind: "solid", color: (ref: "canvas"))),
  styles: (
    text: (fill: (kind: "solid", color: (ref: "text"))),
    actor: (
      fill: (kind: "solid", color: (ref: "surface")),
      stroke: (paint: (kind: "solid", color: (ref: "accent"))),
    ),
    message: (stroke: (paint: (kind: "solid", color: (ref: "accent")))),
  ),
)

#let support = describe-theme-support((
  schema_version: 1,
  family: "sequence",
  output: "standalone-svg",
  subject: (kind: "base-typography", property: "font-stack"),
))
#let preset = export-theme-preset("editor-dark")

#mermaid-theme-definition(
  "sequenceDiagram\n  Alice->>Bob: Shared theme definition",
  definition,
  width: 90%,
)

Support: #support.state

Preset export kind: #preset.kind
