#import "@preview/merman:0.1.0": mermaid-figure, mermaid-profile

#let source = "flowchart TD
  A[Figure wrapper] --> B[Caption]
"

#let figure-profile = mermaid-profile(
  id: "figure-wrapper",
  pipeline: "readable",
  typography: (
    font: "Figure Sans",
    size: "18px",
  ),
)

#mermaid-figure(
  source,
  caption: [Figure wrapper caption],
  profile: figure-profile,
  width: 80%,
  alt: "A merman figure wrapper diagram",
)

#set text(font: "Arial", size: 13pt)

#mermaid-figure(
  source,
  caption: [Context-aware figure caption],
  context-aware: true,
  id: "context-aware-figure",
  pipeline: "readable",
  width: 80%,
)

Figure wrapper fixture passed.
