#import "@preview/merman:0.7.0": (
  mermaid,
  mermaid-figure,
  mermaid-profile,
  mermaid-svg,
  show-mermaid-blocks,
)

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

#let diagram-style = mermaid-profile(
  theme: "studio",
  appearance: "light",
)
#let diagram = mermaid.with(
  profile: diagram-style,
  document-context: true,
  width: 100%,
)
#diagram("flowchart LR\n  Source --> Document")
#diagram("flowchart LR\nA --> B", theme: "paper-cut")

#{
  show raw.where(lang: "mermaid"): show-mermaid-blocks(
    profile: diagram-style,
    document-context: true,
    width: 100%,
    error-mode: "panic",
  )
  raw("flowchart LR\n  Source --> Document", lang: "mermaid", block: true)
}

#let workflow = "flowchart LR\n  Draft --> Review\n  Review --> Publish"
#let diagrams = mermaid-profile(
  theme: "studio",
  background: "#ffffff",
  typography: (font: ("Arial", "sans-serif"), size: "16px"),
  figure: (caption-position: top, gap: 1em, outlined: false),
)
#mermaid-figure(
  workflow,
  profile: diagrams,
  caption: [The publishing workflow.],
  width: 100%,
  alt: "Draft, review, and publish",
) <publishing>

See @publishing for the workflow.

#let theme-source = "flowchart LR\n  Theme --> YourDocument"
#let custom = (schema_version: 2, extends: "paper-cut", seed: (canvas: "#fff4df"))
#let imported = json(bytes(json.encode(custom, pretty: true)))
#assert.eq(imported, custom)
#let custom-svg = mermaid-svg(theme-source, theme: imported, id: "theme-check", pipeline: "readable")
#assert(lower(custom-svg).contains("#fff4df"), message: "the edited paper color must reach SVG output")
#mermaid(theme-source, theme: imported, width: 100%)

#{
  show raw.where(lang: "mermaid"): show-mermaid-blocks(profile: diagrams, error-mode: "panic")
  raw(workflow, lang: "mermaid", block: true)
}

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

README workflow and migration fixture passed.
