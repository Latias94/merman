#import "@preview/merman:0.2.0": (
  describe-theme-support,
  export-theme-preset,
  materialize-theme,
  mermaid-theme-definition,
)

#let definition = (
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: (
    canvas: "#0f172a",
    surface: "#1e293b",
    text: "#e5e7eb",
    accent: "#38bdf8",
  ),
)

#let support = describe-theme-support((
  schema_version: 2,
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
