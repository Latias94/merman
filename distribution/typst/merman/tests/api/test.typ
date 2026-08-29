#import "@preview/merman:0.3.0": (
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
  host-theme: (font_family: "API Host Sans", font_size: "20px"),
)
#assert(direct-svg.contains("api-direct"), message: "direct id should override profile id")
#assert(direct-svg.contains("API Host Sans"), message: "host-theme should override typography")
#assert(not direct-svg.contains("API Direct Sans"), message: "typography should not override host-theme")

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
    version: 2,
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
    version: 2,
    presentation: (
      theme: (font_family: "API Options Sans", font_size: "17px"),
    ),
    svg: (diagram_id: "api-options", pipeline: "readable"),
  ),
)
#assert(options-svg.contains("api-options"), message: "options should override direct and profile id")
#assert(options-svg.contains("API Options Sans"), message: "options should bypass high-level fields")
#assert(not options-svg.contains("api-direct"), message: "direct id should not override options")

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
#assert.eq(capabilities.transport_api_version, 2)
#assert(
  capabilities.capabilities.capability_ids.contains("svg"),
  message: "capabilities should stay exported",
)
#assert.eq(
  capabilities.capabilities.operation_ids,
  ("analysis-json", "svg"),
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
