#import "@preview/merman:0.1.0": (
  mermaid,
  mermaid-figure,
  mermaid-profile,
  mermaid-result,
  mermaid-svg,
  merman-capabilities,
  show-mermaid-blocks,
  validate-mermaid,
)

#let source = "flowchart TD
  A[Canonical API] --> B[Shared renderer path]
"

#let result = mermaid-result(source, id: "api-result", pipeline: "readable")
#assert(result.ok, message: "structured result should render successfully")
#assert.eq(result.code_name, "MERMAN_OK")
#assert(result.svg.contains("api-result"), message: "structured result should use renderer options")

#let validation = validate-mermaid(source)
#assert.eq(validation.code_name, "MERMAN_OK")

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

#let options-svg = mermaid-svg(
  source,
  profile: svg-profile,
  id: "api-direct",
  options: (
    svg: (diagram_id: "api-options", pipeline: "readable"),
    host_theme: (font_family: "API Options Sans", font_size: "17px"),
  ),
)
#assert(options-svg.contains("api-options"), message: "options should override direct and profile id")
#assert(options-svg.contains("API Options Sans"), message: "options should bypass high-level fields")
#assert(not options-svg.contains("api-direct"), message: "direct id should not override options")

#let capabilities = merman-capabilities()
#assert(capabilities.render, message: "capabilities should stay exported")
#assert(capabilities.text_measurement.vendored, message: "capabilities should keep text measurement boundary")
#assert(not capabilities.text_measurement.host_callback, message: "Typst host callback measurement is not supported")

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
