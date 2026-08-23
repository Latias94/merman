#import "@preview/merman:0.2.0": mermaid, show-mermaid-blocks

#show raw.where(lang: "mermaid-issue"): show-mermaid-blocks(
  width: 100%,
  error-mode: "placeholder",
)

#mermaid("", error-mode: "placeholder", width: 80%)

#let issue-89-class = mermaid-svg(
  "classDiagram\n  class User {\n    +String name\n  }",
  id: "issue-89-class-typography",
)
#assert(issue-89-class.contains("User"), message: "issue #89 class label should survive Typst export")
#assert(not issue-89-class.contains("<foreignObject"), message: "Typst default export should be resvg-safe")
#assert(
  issue-89-class.contains("font-size:16px") or issue-89-class.contains("font-size: 16px"),
  message: "Typst export should keep the source 16px class fallback metric",
)

```mermaid-issue
flowchart LR
  First[First raw block] --> Shared[No fixed id]
```

```mermaid-issue
flowchart LR
  Second[Second raw block] --> Shared[No fixed id]
```

Issue fixture passed.
