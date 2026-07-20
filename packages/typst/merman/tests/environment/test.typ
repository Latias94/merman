#import "@preview/merman:0.2.0": (
  mermaid-profile,
  mermaid-result,
  merman-capabilities,
)

#let source = "flowchart LR
  Input[Environment options] --> Output[Binding ABI 2]
"

#let default-result = mermaid-result(source)
#assert(default-result.ok, message: "the renderer-owned default environment should remain valid")

#let deterministic-profile = mermaid-profile(
  text-measurement: "deterministic",
  math-renderer: "none",
)
#let profile-result = mermaid-result(source, profile: deterministic-profile)
#assert(profile-result.ok, message: "profile shorthands should construct a valid environment")

#let invalid-profile = mermaid-profile(
  environment: (
    text_measurement: "invalid-profile-text",
    math_renderer: "invalid-profile-math",
  ),
)
#let direct-environment-result = mermaid-result(
  source,
  profile: invalid-profile,
  environment: (
    text_measurement: "deterministic",
    math_renderer: "none",
  ),
)
#assert(
  direct-environment-result.ok,
  message: "a direct environment should override profile environment fields",
)

#let direct-shorthand-result = mermaid-result(
  source,
  environment: (
    text_measurement: "invalid-direct-text",
    math_renderer: "invalid-direct-math",
  ),
  text-measurement: "deterministic",
  math-renderer: "none",
)
#assert(
  direct-shorthand-result.ok,
  message: "direct environment shorthands should override the corresponding direct fields",
)

#let raw-options-result = mermaid-result(
  source,
  profile: invalid-profile,
  environment: "invalid-direct-environment",
  text-measurement: "invalid-direct-text",
  math-renderer: "invalid-direct-math",
  options: (
    environment: (
      text_measurement: "deterministic",
      math_renderer: "none",
    ),
  ),
)
#assert(raw-options-result.ok, message: "raw options should bypass every high-level environment field")

#let invalid-text-result = mermaid-result(source, text-measurement: "typst-font-assets")
#assert(not invalid-text-result.ok, message: "an unknown text measurement profile must fail closed")
#assert.eq(invalid-text-result.code_name, "MERMAN_INVALID_ARGUMENT")
#assert(
  invalid-text-result.message.contains("environment.text_measurement"),
  message: "text measurement errors should identify the ABI 2 environment field",
)

#let invalid-math-result = mermaid-result(source, math-renderer: "katex")
#assert(not invalid-math-result.ok, message: "an unknown math renderer must fail closed")
#assert.eq(invalid-math-result.code_name, "MERMAN_INVALID_ARGUMENT")
#assert(
  invalid-math-result.message.contains("environment.math_renderer"),
  message: "math renderer errors should identify the ABI 2 environment field",
)

#let legacy-layout-result = mermaid-result(
  source,
  options: (layout: (text_measurer: "deterministic")),
)
#assert(not legacy-layout-result.ok, message: "removed layout environment fields must fail closed")
#assert.eq(legacy-layout-result.code_name, "MERMAN_OPTIONS_JSON_ERROR")
#assert(
  legacy-layout-result.message.contains("environment.text_measurement"),
  message: "the migration error should name the canonical environment field",
)

#let capabilities = merman-capabilities()
#assert(not capabilities.ratex_math, message: "the Typst profile must not advertise browser-bound RaTeX")
#let ratex-result = mermaid-result(source, math-renderer: "ratex")
#assert(not ratex-result.ok, message: "ratex must not silently fall back when the feature is absent")
#assert.eq(ratex-result.code_name, "MERMAN_UNSUPPORTED_FORMAT")
#assert(
  ratex-result.message.contains("environment.math_renderer=ratex"),
  message: "the unsupported-feature error should identify the requested environment renderer",
)

Environment fixture passed.
