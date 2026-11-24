#import "../../src/options.typ": mermaid-profile, render-config

#set page(width: 10in, margin: 0pt)
#set text(font: "Arial", size: 15pt)

#let default-options = render-config().binding_options
#assert(
  not "scoped_css" in default-options.svg,
  message: "default options JSON must omit removed scoped CSS controls",
)
#assert(
  not "css_override_policy" in default-options.svg,
  message: "default options JSON must omit removed CSS override controls",
)
#let default-options-json = json.encode(default-options)
#assert(
  not default-options-json.contains("scoped_css"),
  message: "encoded default options must omit removed scoped CSS controls",
)
#assert(
  not default-options-json.contains("css_override_policy"),
  message: "encoded default options must omit removed CSS override controls",
)

#assert(not "theme" in default-options)
#assert.eq(render-config(theme: "studio").binding_options.theme, "studio")

#let dark-profile = mermaid-profile(theme: "studio", appearance: "dark")
#assert.eq(render-config(profile: dark-profile).binding_options.theme, (preset: "studio", appearance: "dark"))
#assert.eq(
  render-config(profile: dark-profile, appearance: "light").binding_options.theme,
  (preset: "studio", appearance: "light"),
)
#assert.eq(render-config(profile: dark-profile, theme: "paper-cut").binding_options.theme, "paper-cut")
#assert(not "theme" in render-config(profile: dark-profile, theme: "none").binding_options)

#let document = (schema_version: 2, extends: "studio", seed: (accent: "#0052CC"))
#assert.eq(render-config(theme: document).binding_options.theme, document)
#assert.eq(
  render-config(profile: dark-profile, theme: document, appearance: "dark").binding_options.theme,
  (document: document, appearance: "dark"),
)

#let classic = render-config(
  profile: dark-profile,
  theme: "base",
  site-config: (theme: "neutral", themeVariables: (primaryColor: "#111827")),
  background: "transparent",
).binding_options
#assert(not "theme" in classic)
#assert.eq(classic.site_config, (theme: "base", themeVariables: (primaryColor: "#111827",)))
#assert.eq(classic.background, "transparent")

#let opaque = (version: 3, theme: (preset: "studio", appearance: "dark"))
#for config in (
  render-config(options: opaque, theme: "ignored", appearance: "ignored", background: "ignored"),
  render-config(profile: mermaid-profile(options: opaque), theme: "ignored", appearance: "ignored"),
) {
  assert.eq(config.binding_options, opaque, message: "raw options must bypass all theme shorthands")
}

Options contract fixture passed.
