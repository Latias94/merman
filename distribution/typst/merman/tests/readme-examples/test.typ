#import "@preview/merman:0.6.0": (
  export-theme-preset,
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
  theme-preset: "editor",
  theme-appearance: "light",
)
#let diagram = mermaid.with(
  profile: diagram-style,
  document-context: true,
  width: 100%,
)
#diagram("flowchart LR\n  Source --> Document")
#diagram("flowchart LR\nA --> B", theme-preset: "paper-cut")

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
  theme-preset: "editor",
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

#let recipe-source = "flowchart LR\n  Recipe --> YourDocument"
#let original = export-theme-preset("paper-cut")
#let recipe = original
#let tokens = recipe.design.tokens
#let paper = tokens.at("paper")
#paper.insert("value", "#fff4df")
#tokens.insert("paper", paper)
#let body-font = tokens.at("body-font")
#body-font.insert("value", ("Arial", "sans-serif"))
#tokens.insert("body-font", body-font)
#let design = recipe.design
#design.insert("tokens", tokens)
#recipe.insert("design", design)

#let imported = json(bytes(json.encode(recipe, pretty: true)))
#assert.eq(imported, recipe, message: "the complete edited recipe must survive a JSON round trip")
#assert.eq(imported.design.tokens.at("paper").value, "#fff4df")
#assert.eq(imported.design.tokens.at("body-font").value, ("Arial", "sans-serif"))
#let original-svg = mermaid-svg(recipe-source, theme-recipe: original, id: "recipe-check", pipeline: "readable")
#let edited-svg = mermaid-svg(recipe-source, theme-recipe: imported, id: "recipe-check", pipeline: "readable")
#assert(edited-svg != original-svg, message: "editing the shared recipe tokens must change rendered SVG")
#assert(lower(edited-svg).contains("#fff4df"), message: "the edited paper color must reach SVG output")
#mermaid(recipe-source, theme-recipe: imported, width: 100%)

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
