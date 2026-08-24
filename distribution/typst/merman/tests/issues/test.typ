#import "@preview/merman:0.2.0": mermaid, mermaid-svg, show-mermaid-blocks

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
#let issue-89-fallback = issue-89-class
  .split("data-merman-foreignobject=\"fallback\"")
  .find(fragment => fragment.contains(">User</text>"))
#assert(issue-89-fallback != none, message: "issue #89 should emit a tagged fallback text node")
#let issue-89-user-text-tag = issue-89-fallback
  .split(">User</text>")
  .first()
  .split("<" + "text")
  .last()
#assert(
  issue-89-user-text-tag.contains("font-size:16px") or issue-89-user-text-tag.contains("font-size: 16px"),
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
