#import "@preview/merman:0.1.0": mermaid-figure, mermaid-profile

#let source = "flowchart TD
  A[Figure wrapper] --> B[Caption]
"

#let figure-profile = mermaid-profile(
  id: "figure-wrapper",
  typography: (
    font: "Figure Sans",
    size: "18px",
  ),
  figure: (
    placement: bottom,
    scope: "parent",
    outlined: false,
    gap: 1em,
    caption-position: top,
    caption-separator: [ -- ],
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
  document-context: true,
  profile: figure-profile,
  id: "document-context-figure",
  placement: top,
  caption-position: bottom,
  width: 80%,
)

#context {
  let figures = query(figure)
  assert(figures.len() == 2, message: "fixture should render two figures")

  let profiled = figures.at(0)
  assert(profiled.body.width == 80%, message: "figure image width should be forwarded")
  assert(profiled.placement == bottom, message: "profile figure placement should be forwarded")
  assert(profiled.scope == "parent", message: "profile figure scope should be forwarded")
  assert(profiled.outlined == false, message: "profile figure outlined should be forwarded")
  assert(profiled.gap == 1em, message: "profile figure gap should be forwarded")
  assert(profiled.caption.position == top, message: "profile caption position should be forwarded")

  let document-context-figure = figures.at(1)
  assert(document-context-figure.placement == top, message: "direct figure placement should override profile")
  assert(document-context-figure.caption.position == bottom, message: "direct caption position should override profile")
}

Figure wrapper fixture passed.
