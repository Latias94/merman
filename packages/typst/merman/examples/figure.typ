#import "@preview/merman:0.1.0": mermaid-figure, mermaid-profile

= Mermaid figure

#let diagram-profile = mermaid-profile(
  pipeline: "resvg-safe",
  background: "#ffffff",
  typography: (
    font: ("Source Sans 3", "Arial", "sans-serif"),
    size: "16px",
  ),
)

#mermaid-figure(
  "flowchart TD
    Source[Mermaid source] --> Diagram[SVG image]
    Diagram --> Figure[Typst figure]
  ",
  caption: [A Mermaid diagram rendered as a Typst figure.],
  profile: diagram-profile,
  width: 90%,
  alt: "A Mermaid diagram rendered as a Typst figure",
)
