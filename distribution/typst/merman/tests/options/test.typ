#import "@preview/merman:0.3.0": mermaid-svg
#import "../../src/options.typ": config-with-theme-spec, context-text-style, mermaid-profile, render-config
#import "../../src/render.typ": render-svg-result-with-config, with-theme-definition-render-config

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

#let definition-context-config = config-with-theme-spec(
  render-config(context-text-style: context-text-style("Definition Context Sans", 12pt)),
  (typography: (default: (font_weight: 600))),
)
#assert.eq(
  definition-context-config.binding_options.theme.spec.typography.default,
  (
    font_weight: 600,
    font_stack: ("Definition Context Sans",),
    font_size_px: 16,
  ),
  message: "materialized specs should retain resolved document typography",
)
#let definition-context-svg = mermaid-svg(
  "flowchart LR\nA[Definition context] --> B[Rendered]",
  options: definition-context-config.binding_options,
)
#assert(
  definition-context-svg.contains("Definition Context Sans"),
  message: "materialized theme configuration should emit the document font",
)
#assert(
  definition-context-svg.contains("16px"),
  message: "materialized theme configuration should emit the document font size",
)

#let facade-definition = (
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: (text: "#123456", accent: "#abcdef"),
)

#context {
  with-theme-definition-render-config(
    facade-definition,
    config => {
      let result = render-svg-result-with-config(
        "flowchart LR\nA[Definition context] --> B[Rendered]",
        config,
      )
      assert(result.ok, message: "context-aware definition rendering should succeed")
      assert.eq(
        config.binding_options.theme.spec.typography.default.font_stack,
        ("arial",),
        message: "the public definition render path should retain the Typst font",
      )
      assert.eq(
        config.binding_options.theme.spec.typography.default.font_size_px,
        20,
        message: "the public definition render path should retain the Typst font size",
      )
      assert.eq(
        config.binding_options.layout.container_width,
        960,
        message: "the public definition render path should retain the available Typst width",
      )
      assert(
        result.svg.contains("arial"),
        message: "the final definition SVG should emit the Typst font",
      )
      assert(
        result.svg.contains("20px"),
        message: "the final definition SVG should emit the Typst font size",
      )
    },
    result => panic("context-aware definition materialization failed: " + result.code_name),
    document-context: true,
  )

  with-theme-definition-render-config(
    facade-definition,
    config => {
      let result = render-svg-result-with-config(
        "flowchart LR\nA[Direct options] --> B[Rendered]",
        config,
      )
      assert(result.ok, message: "opaque direct options should render a definition")
      assert.eq(
        config.binding_options.layout.container_width,
        321,
        message: "opaque direct options should keep their layout",
      )
      assert(
        result.svg.contains("definition-opaque-direct"),
        message: "opaque direct options should keep their SVG settings",
      )
      assert(
        not result.svg.contains("Arial"),
        message: "document context must not be injected into opaque direct options",
      )
      assert(
        not result.svg.contains("Raw Direct Sans"),
        message: "a definition must replace an opaque options theme selection",
      )
    },
    result => panic("opaque direct definition materialization failed: " + result.code_name),
    document-context: true,
    options: (
      version: 3,
      theme: (spec: (typography: (default: (font_stack: ("Raw Direct Sans",))))),
      layout: (container_width: 321),
      svg: (diagram_id: "definition-opaque-direct", pipeline: "readable"),
    ),
  )

  with-theme-definition-render-config(
    facade-definition,
    config => {
      let result = render-svg-result-with-config(
        "flowchart LR\nA[Profile options] --> B[Rendered]",
        config,
      )
      assert(result.ok, message: "opaque profile options should render a definition")
      assert.eq(
        config.binding_options.layout.container_width,
        322,
        message: "opaque profile options should keep their layout",
      )
      assert(
        result.svg.contains("definition-opaque-profile"),
        message: "opaque profile options should keep their SVG settings",
      )
      assert(
        not result.svg.contains("Arial"),
        message: "document context must not be injected into opaque profile options",
      )
      assert(
        not result.svg.contains("Raw Profile Sans"),
        message: "a definition must replace an opaque profile theme selection",
      )
    },
    result => panic("opaque profile definition materialization failed: " + result.code_name),
    document-context: true,
    profile: mermaid-profile(options: (
      version: 3,
      theme: (spec: (typography: (default: (font_stack: ("Raw Profile Sans",))))),
      layout: (container_width: 322),
      svg: (diagram_id: "definition-opaque-profile", pipeline: "readable"),
    )),
  )

  with-theme-definition-render-config(
    facade-definition,
    config => panic("resource-limited definition unexpectedly materialized"),
    result => {
      assert.eq(
        result.code_name,
        "MERMAN_RESOURCE_LIMIT_EXCEEDED",
        message: "definition materialization should honor the caller resource limit",
      )
      assert.eq(
        result.details.resource.limit_id,
        "max_theme_encoded_bytes",
        message: "definition materialization should expose the caller resource limit",
      )
    },
    document-context: true,
    options: (
      version: 3,
      resources: (
        profile: "constrained",
        limits: (max_theme_encoded_bytes: 1),
      ),
    ),
  )
}

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
