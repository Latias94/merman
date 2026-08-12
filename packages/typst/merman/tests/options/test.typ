#import "../../src/options.typ": context-text-style, mermaid-profile, render-config

#assert.eq(context-text-style("Arial", 12pt).font_stack, ("Arial",))
#assert.eq(context-text-style("Arial", 12pt).font_size_px, 16)

#let preset-config = render-config(theme-preset: "ayu-dark")
#assert.eq(preset-config.binding_options.theme, (preset: "ayu-dark"))
#assert(not "presentation" in preset-config.binding_options)
#assert(not "host_theme" in preset-config.binding_options)

#let typography-config = render-config(
  typography: (font: "Document Sans", size: 12pt),
)
#assert.eq(
  typography-config.binding_options.theme,
  (
    spec: (
      typography: (
        default: (font_stack: ("Document Sans",), font_size_px: 16),
      ),
    ),
  ),
)

#let spec-config = render-config(
  diagram-theme: (
    typography: (default: (font_weight: 600)),
  ),
  typography: (font: ("Inter", "Arial"), size: "18px"),
)
#assert.eq(
  spec-config.binding_options.theme.spec.typography.default,
  (
    font_weight: 600,
    font_stack: ("Inter", "Arial"),
    font_size_px: 18,
  ),
)
#assert(not "presentation" in spec-config.binding_options)
#assert(not "host_theme" in spec-config.binding_options)

#let null-typography-config = render-config(
  diagram-theme: (typography: none),
  typography: (font: "Fallback Sans"),
)
#assert.eq(
  null-typography-config.binding_options.theme.spec.typography.default.font_stack,
  ("Fallback Sans",),
)

#let mermaid-theme-config = render-config(
  theme-name: "base",
  theme-variables: (primaryColor: "#111827"),
)
#assert(not "theme" in mermaid-theme-config.binding_options)
#assert.eq(mermaid-theme-config.binding_options.site_config.theme, "base")
#assert.eq(
  mermaid-theme-config.binding_options.site_config.themeVariables,
  (primaryColor: "#111827"),
)

#let profile-config = render-config(
  profile: mermaid-profile(
    theme-preset: "editor-dark",
    theme-name: "base",
    theme-variables: (primaryColor: "#111827"),
  ),
)
#assert.eq(profile-config.binding_options.theme, (preset: "editor-dark"))
#assert.eq(profile-config.binding_options.site_config.theme, "base")
#assert.eq(profile-config.binding_options.site_config.themeVariables.primaryColor, "#111827")
#assert(not "presentation" in profile-config.binding_options)
#assert(not "host_theme" in profile-config.binding_options)

Options contract fixture passed.
