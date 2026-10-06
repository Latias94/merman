#import "@preview/merman:0.4.0": mermaid, mermaid-profile, mermaid-svg, show-mermaid-blocks

#set text(font: "Arial", size: 13pt)

#let source = "flowchart LR
  Old[Old context wrapper] --> New[document-context parameter]
"

#mermaid(
  source,
  document-context: true,
  width: 100%,
  alt: "Migrated context render",
)

#let layout-source = "flowchart LR\n  Source --> Layout\n  Layout --> SVG"
#let dagre = mermaid-profile(site-config: (layout: "dagre"))
#let default-svg = mermaid-svg(layout-source, id: "layout-check", pipeline: "readable")
#let direct-dagre-svg = mermaid-svg(
  layout-source,
  id: "layout-check",
  pipeline: "readable",
  site-config: (layout: "dagre"),
)
#let profile-dagre-svg = mermaid-svg(
  layout-source,
  id: "layout-check",
  pipeline: "readable",
  profile: dagre,
)
#assert(default-svg != direct-dagre-svg, message: "Dagre must change the default flowchart layout")
#assert.eq(profile-dagre-svg, direct-dagre-svg, message: "profile must select Dagre just like site-config")

#mermaid(layout-source, profile: dagre)

#show raw.where(lang: "mermaid"): show-mermaid-blocks(
  document-context: true,
  width: 100%,
  error-mode: "panic",
)

```mermaid
sequenceDiagram
  participant Old
  participant New
  Old->>New: show-mermaid-blocks-context(...)
  New-->>Old: show-mermaid-blocks(document-context: true, ...)
```

API migration fixture passed.
