#import "@preview/merman:0.3.0": mermaid, mermaid-svg
#import "../../src/options.typ": config-with-context-width, context-text-style, mermaid-profile, render-config
#import "../../src/units.typ": context-width-css-px, typst-length-to-css-px

#set page(width: 12cm, margin: 10mm)
#set text(font: "Arial", size: 13pt)

#assert.eq(typst-length-to-css-px(72pt), 96)
#assert.eq(context-width-css-px(float.inf * 1pt), none)
#assert.eq(context-text-style("Arial", 12pt).font_size_px, 16)
#assert.eq(
  context-text-style((name: "Inria Serif", covers: "latin-in-cjk"), 12pt).font_stack,
  ("Inria Serif",),
)
#let inferred-width-config = config-with-context-width(render-config(), typst-length-to-css-px(72pt))
#assert.eq(inferred-width-config.binding_options.layout.container_width, 96)
#assert.eq(inferred-width-config.binding_options.version, 3)
#assert(not "presentation" in inferred-width-config.binding_options)
#assert(not "host_theme" in inferred-width-config.binding_options)
#let preset-config = render-config(
  theme-preset: "ayu-dark",
  pipeline: "readable",
)
#assert.eq(preset-config.binding_options.theme.preset, "ayu-dark")
#assert.eq(preset-config.binding_options.svg.pipeline, "readable")
#assert(not "presentation" in preset-config.binding_options)
#assert(not "host_theme" in preset-config.binding_options)
#let profile-theme-config = render-config(
  profile: mermaid-profile(
    diagram-theme: (
      typography: (default: (font_weight: 600)),
    ),
    typography: (font: "Profile Diagram Sans", size: "18px"),
    pipeline: "parity",
  ),
)
#assert.eq(
  profile-theme-config.binding_options.theme.spec.typography.default.font_stack,
  ("Profile Diagram Sans",),
)
#assert.eq(profile-theme-config.binding_options.theme.spec.typography.default.font_size_px, 18)
#assert.eq(profile-theme-config.binding_options.theme.spec.typography.default.font_weight, 600)
#assert.eq(profile-theme-config.binding_options.svg.pipeline, "parity")
#assert(not "presentation" in profile-theme-config.binding_options)
#assert(not "host_theme" in profile-theme-config.binding_options)
#let profile-layout-width-config = config-with-context-width(
  render-config(profile: mermaid-profile(layout: (container_width: 333))),
  typst-length-to-css-px(72pt),
)
#assert.eq(
  profile-layout-width-config.binding_options.layout.container_width,
  333,
  message: "profile layout width should take precedence over document context width",
)
#let profile-shorthand-width-config = config-with-context-width(
  render-config(profile: mermaid-profile(container-width: 333)),
  typst-length-to-css-px(72pt),
)
#assert.eq(
  profile-shorthand-width-config.binding_options.layout.container_width,
  333,
  message: "profile container-width should take precedence over document context width",
)
#let partial-typography-config = render-config(
  profile: mermaid-profile(typography: (font: "Profile Sans", size: "16px")),
  typography: (font: "Direct Sans"),
)
#assert.eq(
  partial-typography-config.binding_options.theme.spec.typography.default.font_stack,
  ("Direct Sans",),
)
#assert.eq(
  partial-typography-config.binding_options.theme.spec.typography.default.font_size_px,
  16,
  message: "partial direct typography should preserve the profile size",
)
#let replacement-site-config = render-config(
  profile: mermaid-profile(site-config: (theme: "dark", fontFamily: "Profile Sans")),
  site-config: (theme: "neutral"),
)
#assert.eq(
  replacement-site-config.binding_options.site_config,
  (theme: "neutral"),
  message: "a direct site-config should replace the profile object",
)
#let theme-alias-config = render-config(
  profile: mermaid-profile(base-theme: "dark"),
  theme-name: "forest",
)
#assert.eq(
  theme-alias-config.binding_options.site_config.theme,
  "forest",
  message: "theme-name should override the base-theme compatibility alias",
)
#let full-layout-config = render-config(
  profile: mermaid-profile(container-width: 333, container-height: 222),
  layout: (container_width: 444, container_height: 555),
)
#assert.eq(full-layout-config.binding_options.layout.container_width, 444)
#assert.eq(full-layout-config.binding_options.layout.container_height, 555)
#assert.eq(
  context-text-style(
    ((name: "Inria Serif", covers: "latin-in-cjk"), "Noto Serif CJK SC"),
    12pt,
  ).font_stack,
  ("Inria Serif", "Noto Serif CJK SC"),
)

#let source = "flowchart LR
  A[Document font] --> B[Context render]
"

#let explicit-svg = mermaid-svg(source, id: "context-explicit", pipeline: "readable")
#assert(explicit-svg.contains("context-explicit"), message: "explicit render should use direct options")
#assert(not explicit-svg.contains("Arial"), message: "default render must not inherit Typst document font")
#assert(not explicit-svg.contains("17.333"), message: "default render must not inherit Typst text size")

#let direct-svg = mermaid-svg(
  source,
  id: "context-direct",
  pipeline: "readable",
  typography: (font: "Explicit Sans", size: "18px"),
  container-width: 444,
)
#assert(direct-svg.contains("Explicit Sans"), message: "direct typography should be usable without context")
#assert(not direct-svg.contains("Arial"), message: "SVG export should remain explicit-only")

#let typst-length-svg = mermaid-svg(
  source,
  id: "context-typst-length",
  pipeline: "readable",
  typography: (font: "Typst Length Sans", size: 12pt),
)
#assert(typst-length-svg.contains("16px"), message: "12pt typography must become 16 CSS px")
#assert(not typst-length-svg.contains("12pt"), message: "Typst point units must not leak into SVG layout")

#mermaid(
  source,
  document-context: true,
  id: "context-enabled",
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-direct-width",
  container-width: 444,
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-direct-layout",
  layout: (container_width: 333),
  width: 100%,
)

#mermaid(
  source,
  document-context: true,
  id: "context-profile-layout",
  profile: mermaid-profile(layout: (container_width: 333)),
  width: 100%,
)

Context fixture passed.
