#import "@preview/merman:0.1.0": merman-capabilities

#let capabilities = merman-capabilities()

#assert(capabilities.render, message: "Typst package should expose SVG rendering capability")
#assert(capabilities.text_measurement.vendored, message: "Typst package should report vendored text measurement")
#assert(capabilities.text_measurement.deterministic, message: "Typst package should report deterministic text measurement")
#assert(not capabilities.text_measurement.host_callback, message: "Typst plugin should not claim browser-style host measurement")
#assert(not capabilities.text_measurement.font_assets, message: "Typst plugin should not claim font asset measurement")

Capabilities fixture passed.
