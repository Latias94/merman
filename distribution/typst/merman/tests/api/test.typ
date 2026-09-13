#import "@preview/merman:0.3.0": (
  analyze-mermaid,
  describe-theme-support,
  export-theme-preset,
  materialize-theme,
  theme-catalog,
  mermaid,
  mermaid-theme-definition,
  mermaid-figure,
  mermaid-profile,
  mermaid-result,
  mermaid-svg,
  merman-capabilities,
  show-mermaid-blocks,
)

#let source = "flowchart TD
  A[Canonical API] --> B[Shared renderer path]
"

#let result = mermaid-result(source, id: "api-result", pipeline: "readable")
#assert(result.ok, message: "structured result should render successfully")
#assert.eq(result.operation, "render-svg")
#assert.eq(result.code_name, "MERMAN_OK")
#assert.eq(result.kind, none)
#assert.eq(result.capability_id, none)
#assert(result.svg.contains("api-result"), message: "structured result should use renderer options")

#let missing-math = mermaid-result("flowchart TD\nA[\"$$x^2$$\"] --> B")
#assert(not missing-math.ok, message: "uncompiled math should be a structured capability error")
#assert.eq(missing-math.operation, "render-svg")
#assert.eq(missing-math.kind, "missing-capability")
#assert.eq(missing-math.capability_id, "math")
#assert.eq(missing-math.svg, none)

#let analysis = analyze-mermaid(source)
#assert.eq(analysis.version, 1)
#assert(analysis.valid, message: "valid input should produce a valid canonical analysis payload")
#assert.eq(analysis.summary.errors, 0)
#assert.eq(analysis.source.kind, "diagram")
#assert.eq(analysis.diagnostics.len(), 0)

#let failed-analysis = analyze-mermaid("")
#assert.eq(failed-analysis.version, 1)
#assert(not failed-analysis.valid, message: "invalid input should stay inside the analysis schema")
#assert.eq(failed-analysis.summary.errors, 1)
#assert.eq(failed-analysis.diagnostics.at(0).code_name, "MERMAN_NO_DIAGRAM")

#let svg-profile = mermaid-profile(
  id: "api-profile",
  pipeline: "readable",
  typography: (font: "API Profile Sans", size: "18px"),
  figure: (placement: bottom, outlined: false),
)

#let profiled-svg = mermaid-svg(source, profile: svg-profile)
#assert(profiled-svg.contains("api-profile"), message: "profile should apply to SVG export")
#assert(profiled-svg.contains("API Profile Sans"), message: "profile typography should apply")

#let direct-svg = mermaid-svg(
  source,
  profile: svg-profile,
  id: "api-direct",
  typography: (font: "API Direct Sans", size: "19px"),
)
#assert(direct-svg.contains("api-direct"), message: "direct id should override profile id")
#assert(direct-svg.contains("API Direct Sans"), message: "direct typography should override profile typography")
#assert(not direct-svg.contains("API Profile Sans"), message: "profile typography should not override direct typography")

#let low-level-id-svg = mermaid-svg(
  source,
  id: "api-high-level-id",
  diagram-id: "api-low-level-id",
  pipeline: "readable",
)
#assert(
  low-level-id-svg.contains("api-low-level-id"),
  message: "diagram-id should override id at the same call layer",
)
#assert(
  not low-level-id-svg.contains("api-high-level-id"),
  message: "id should not override diagram-id at the same call layer",
)

#let profile-id-svg = mermaid-svg(
  source,
  profile: mermaid-profile(diagram-id: "api-profile-low-level-id"),
  id: "api-direct-id",
  pipeline: "readable",
)
#assert(
  profile-id-svg.contains("api-direct-id"),
  message: "direct id should override a profile diagram-id",
)
#assert(
  not profile-id-svg.contains("api-profile-low-level-id"),
  message: "profile diagram-id should not override direct id",
)

#let profile-both-ids-svg = mermaid-svg(
  source,
  profile: mermaid-profile(
    id: "api-profile-id",
    diagram-id: "api-profile-diagram-id",
  ),
  pipeline: "readable",
)
#assert(
  profile-both-ids-svg.contains("api-profile-diagram-id"),
  message: "profile diagram-id should override profile id",
)
#assert(
  not profile-both-ids-svg.contains("api-profile-id"),
  message: "profile id should not override profile diagram-id",
)

#let direct-low-level-id-svg = mermaid-svg(
  source,
  profile: mermaid-profile(id: "api-profile-id"),
  diagram-id: "api-direct-low-level-id",
  pipeline: "readable",
)
#assert(
  direct-low-level-id-svg.contains("api-direct-low-level-id"),
  message: "direct diagram-id should override profile id",
)

#let snake-profile-id-svg = mermaid-svg(
  source,
  profile: (diagram_id: "api-snake-profile-id"),
  pipeline: "readable",
)
#assert(
  snake-profile-id-svg.contains("api-snake-profile-id"),
  message: "profile diagram_id should remain accepted as a binding alias",
)

#let resource-limited-result = mermaid-result(
  source,
  options: (
    version: 3,
    resources: (limits: (max_source_bytes: 1)),
  ),
)
#assert(
  not resource-limited-result.ok,
  message: "mermaid-result should preserve structured resource failures",
)
#assert.eq(resource-limited-result.code_name, "MERMAN_RESOURCE_LIMIT_EXCEEDED")
#assert.eq(
  resource-limited-result.details.resource.limit_id,
  "max_source_bytes",
  message: "resource failure details should identify the exceeded limit",
)

#let options-svg = mermaid-svg(
  source,
  profile: svg-profile,
  id: "api-direct",
  options: (
    version: 3,
    theme: (
      spec: (
        typography: (
          default: (font_stack: ("API Options Sans",), font_size_px: 17),
        ),
      ),
    ),
    svg: (diagram_id: "api-options", pipeline: "readable"),
  ),
)
#assert(options-svg.contains("api-options"), message: "options should override direct and profile id")
#assert(options-svg.contains("API Options Sans"), message: "options should bypass high-level fields")
#assert(not options-svg.contains("api-direct"), message: "direct id should not override options")

#let raw-scoped-css-result = mermaid-result(
  source,
  options: (version: 3, svg: (scoped_css: ".node rect { fill: red; }")),
)
#assert(
  not raw-scoped-css-result.ok,
  message: "raw scoped CSS must remain rejected by the general binding boundary",
)
#assert.eq(raw-scoped-css-result.code_name, "MERMAN_OPTIONS_JSON_ERROR")
#assert(
  raw-scoped-css-result.message.contains("svg.scoped_css"),
  message: "raw scoped CSS errors should identify the rejected binding field",
)

#let raw-css-override-policy-result = mermaid-result(
  source,
  options: (version: 3, svg: (css_override_policy: "preserve")),
)
#assert(
  not raw-css-override-policy-result.ok,
  message: "raw CSS override policy must remain rejected by the general binding boundary",
)
#assert.eq(raw-css-override-policy-result.code_name, "MERMAN_OPTIONS_JSON_ERROR")
#assert(
  raw-css-override-policy-result.message.contains("svg.css_override_policy"),
  message: "raw CSS override errors should identify the rejected binding field",
)

#let forest-svg = mermaid-svg(
  source,
  id: "api-theme-layer",
  pipeline: "readable",
  theme-name: "forest",
)
#let profile-theme-svg = mermaid-svg(
  source,
  id: "api-theme-layer",
  pipeline: "readable",
  profile: mermaid-profile(
    site-config: (theme: "dark"),
    theme-name: "forest",
  ),
)
#assert.eq(
  profile-theme-svg,
  forest-svg,
  message: "profile theme shorthand should override profile site-config theme fields",
)
#let direct-theme-svg = mermaid-svg(
  source,
  id: "api-theme-layer",
  pipeline: "readable",
  profile: mermaid-profile(site-config: (theme: "dark")),
  site-config: (theme: "neutral"),
  theme-name: "forest",
)
#assert.eq(
  direct-theme-svg,
  forest-svg,
  message: "direct theme shorthand should override profile and direct site-config theme fields",
)

#let capabilities = merman-capabilities()
#assert.eq(capabilities.schema_version, 1)
#assert.eq(capabilities.transport_api_version, 4)
#assert(
  capabilities.capabilities.capability_ids.contains("svg"),
  message: "capabilities should stay exported",
)
#assert.eq(
  capabilities.capabilities.operation_ids,
  (
    "analysis-json",
    "describe-theme-support-json",
    "export-theme-preset-json",
    "materialize-theme-json",
    "svg",
  ),
)

#let theme-definition = (
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: (text: "#123456", accent: "#abcdef"),
)
#let materialized-theme = materialize-theme(theme-definition)
#assert.eq(materialized-theme.schema_version, 1)
#assert.eq(materialized-theme.authoring_schema_version, 1)
#assert.eq(materialized-theme.expansion_version, 1)
#assert.eq(materialized-theme.spec_schema_version, 1)
#assert(materialized-theme.spec.styles.len() > 0)

#let materialization-limit = materialize-theme(
  theme-definition,
  options: (
    version: 3,
    resources: (
      profile: "constrained",
      limits: (max_theme_encoded_bytes: 1),
    ),
  ),
)
#assert(
  not ("spec" in materialization-limit),
  message: "materialization must enforce caller resource limits before decoding",
)

#let theme-support = describe-theme-support((
  schema_version: 2,
  family: "sequence",
  output: "standalone-svg",
  subject: (kind: "base-typography", property: "font-stack"),
))
#assert.eq(theme-support.schema_version, 2)
#assert.eq(theme-support.query.family, "sequence")

#let preset-export = export-theme-preset("editor-light")
#assert.eq(preset-export.kind, "complete_spec")
#assert(type(preset-export.complete_spec) == dictionary)

#let light-definition = (
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: (
    canvas: "#f8fafc",
    surface: "#ffffff",
    text: "#0f172a",
    border: "#cbd5e1",
    line: "#64748b",
    accent: "#2563eb",
  ),
  styles: (
    (
      kind: "rule",
      family: "flowchart",
      target: "node",
      style: (fill: "#dbeafe"),
    ),
  ),
)
#let dark-definition = (
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: (
    canvas: "#0f172a",
    surface: "#1e293b",
    text: "#e2e8f0",
    border: "#475569",
    line: "#94a3b8",
    accent: "#38bdf8",
  ),
  styles: (
    (
      kind: "rule",
      family: "flowchart",
      target: "node",
      style: (fill: "#1e3a8a"),
    ),
  ),
)
#let light-json = json.encode(light-definition)
#let light-from-json = materialize-theme(light-json)
#let light-from-typed = materialize-theme(light-definition)
#let dark-json = json.encode(dark-definition)
#let dark-from-json = materialize-theme(dark-json)
#let dark-from-typed = materialize-theme(dark-definition)
#assert.eq(
  light-from-json.spec,
  light-from-typed.spec,
  message: "JSON import and typed construction must materialize identically",
)
#assert.eq(
  dark-from-json.spec,
  dark-from-typed.spec,
  message: "dark JSON import and typed construction must materialize identically",
)
#assert(light-json != "", message: "the authoring definition must have a readable JSON export")
#assert(dark-json != "", message: "the dark authoring definition must have a readable JSON export")

#let unknown-support = describe-theme-support((
  schema_version: 1,
  family: "future-family",
  output: "standalone-svg",
  target: "node",
  facet: "fill",
))
#assert.eq(unknown-support.state, "unverified")
#assert(unknown-support.reason_ids.contains("theme-support.unknown-family"))

#set text(font: "Definition Context Sans", size: 12pt)
#mermaid-theme-definition(
  "flowchart LR\nA[Light] --> B[Authored]",
  light-json,
  document-context: true,
  width: 80%,
)
#mermaid-theme-definition(
  "flowchart LR\nA[Dark] --> B[Authored]",
  dark-definition,
  width: 80%,
)
#mermaid-theme-definition(
  "flowchart LR\nA[Invalid] --> B[Theme]",
  (:),
  document-context: true,
  error-mode: "text",
)
#mermaid-theme-definition(
  "flowchart LR\nA[Invalid] --> B[Theme]",
  (:),
  document-context: true,
  error-mode: "placeholder",
  width: 80%,
)
#assert(
  capabilities.capabilities.text_measurement.provider_ids.contains("deterministic"),
  message: "capabilities should keep text measurement boundary",
)
#assert(
  not capabilities.capabilities.text_measurement.provider_ids.contains("host-callback"),
  message: "Typst host callback measurement is not supported",
)
#assert(
  capabilities.resources.profiles.any(profile => profile.id == "constrained"),
  message: "the runtime catalog should expose the constrained resource profile",
)

#let image-profile = mermaid-profile(
  id: "api-image",
  typography: (font: "API Image Sans", size: "18px"),
  figure: (placement: bottom, outlined: false),
)

#mermaid(source, profile: image-profile, width: 80%, alt: "Canonical API image")

#mermaid-figure(
  source,
  profile: image-profile,
  caption: [Canonical API figure],
  width: 80%,
)

#show raw.where(lang: "mermaid-api"): show-mermaid-blocks(
  profile: image-profile,
  width: 80%,
  error-mode: "panic",
)

```mermaid-api
flowchart LR
  Raw[Raw block] --> Handler[Show handler]
```

API fixture passed.

#let catalog = theme-catalog()
#assert.eq(catalog.schema_version, 3)
#assert(catalog.structured_spec_available)
#assert.eq(catalog.supported_output_ids, ("svg",))
#assert.eq(catalog.presets.len(), 10)
#assert.eq(catalog.presets.at(0).id, "editor-light")
#assert.eq(catalog.presets.at(0).qualified_cells, ())
#assert.eq(merman-capabilities().metadata_ids, ("theme-catalog",))
#let catalog-preset = export-theme-preset(catalog.presets.at(0).id)
#assert.eq(catalog-preset.kind, catalog.presets.at(0).export_kind)
#assert(type(catalog-preset.complete_spec) == dictionary)
