#import "@preview/merman:0.6.0": (
  analyze-mermaid,
  mermaid,
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

#let layout-work-source = {
  let lines = ("mindmap", "  root[Root]")
  for branch in range(8) {
    lines.push("    branch" + str(branch) + "[Branch " + str(branch) + "]")
    for leaf in range(8) {
      lines.push("      leaf" + str(branch) + "_" + str(leaf) + "[Leaf " + str(leaf) + "]")
    }
  }
  lines.join("\n") + "\n"
}
#let default-layout-result = mermaid-result(layout-work-source, id: "api-layout-work")
#assert(default-layout-result.ok, message: "Mindmap should fit the default interactive layout allowance")

#let stricter-layout-result = mermaid-result(
  layout-work-source,
  options: (resources: (limits: (max_layout_work_units: 125000))),
)
#assert(not stricter-layout-result.ok, message: "an explicit stricter layout allowance should still apply")
#assert.eq(stricter-layout-result.code_name, "MERMAN_RESOURCE_LIMIT_EXCEEDED")
#assert.eq(stricter-layout-result.details.resource.limit_id, "max_layout_work_units")
#assert.eq(stricter-layout-result.details.resource.max, 125000)
#assert.eq(stricter-layout-result.details.resource.cause, "ceiling")

#let constrained-layout-result = mermaid-result(
  layout-work-source,
  options: (resources: (profile: "constrained")),
)
#assert(not constrained-layout-result.ok, message: "an explicit constrained profile should retain its own allowance")
#assert.eq(constrained-layout-result.code_name, "MERMAN_RESOURCE_LIMIT_EXCEEDED")
#assert.eq(constrained-layout-result.details.resource.limit_id, "max_layout_work_units")
#assert.eq(constrained-layout-result.details.resource.max, 125000)

#let layout-work-options = (
  resources: (limits: (max_layout_work_units: 20000000)),
  svg: (diagram_id: "api-layout-work", pipeline: "resvg-safe"),
)
#let direct-layout-result = mermaid-result(layout-work-source, options: layout-work-options)
#assert(direct-layout-result.ok, message: "direct options should allow an explicit finite layout budget")
#assert(direct-layout-result.svg.contains("api-layout-work"))
#assert.eq(default-layout-result.svg, direct-layout-result.svg)
#let layout-work-profile = mermaid-profile(options: layout-work-options)
#let profiled-layout-result = mermaid-result(layout-work-source, profile: layout-work-profile)
#assert(profiled-layout-result.ok, message: "a raw-options profile should reuse the finite layout budget")
#assert.eq(profiled-layout-result.svg, direct-layout-result.svg)

#let overflowing-layout-result = mermaid-result(
  source,
  options: (resources: (limits: (max_layout_work_units: 4294967296))),
)
#assert(not overflowing-layout-result.ok, message: "layout limits must fit the WASM32 resource counter")
#assert.eq(overflowing-layout-result.code_name, "MERMAN_OPTIONS_JSON_ERROR")

#let options-svg = mermaid-svg(
  source,
  profile: svg-profile,
  id: "api-direct",
  options: (
    version: 3,
    theme: (
      schema_version: 2,
      extends: "studio",
      typography: (sans: ("API Options Sans", "sans-serif")),
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
  theme: "forest",
)
#let profile-theme-svg = mermaid-svg(
  source,
  id: "api-theme-layer",
  pipeline: "readable",
  profile: mermaid-profile(
    site-config: (theme: "dark"),
    theme: "forest",
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
  theme: "forest",
)
#assert.eq(
  direct-theme-svg,
  forest-svg,
  message: "direct theme shorthand should override profile and direct site-config theme fields",
)

#let capabilities = merman-capabilities()
#assert.eq(
  capabilities.package_version,
  sys.inputs.at("merman-version", default: "0.11.0"),
  message: "Typst package must bundle the expected Merman runtime",
)
#assert.eq(capabilities.schema_version, 1)
#assert.eq(capabilities.transport_api_version, 3)
#assert(
  capabilities.capabilities.capability_ids.contains("svg"),
  message: "capabilities should stay exported",
)
#assert(
  capabilities.capabilities.capability_ids.contains("layout-dagre"),
  message: "the packaged runtime should expose Dagre",
)
#let dagre-result = mermaid-result(source, site-config: (layout: "dagre"))
#assert(dagre-result.ok, message: "the packaged runtime should render an explicit Dagre request")
#assert.eq(
  capabilities.capabilities.operation_ids,
  (
    "analysis-json",
    "export-theme-preset",
    "resolve-theme",
    "svg",
  ),
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
