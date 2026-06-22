#import "@preview/merman:0.1.0": mermaid-profile, mermaid-svg

#let source = "flowchart LR
  A[Options] --> B[Escape hatch]
"

#let profile = mermaid-profile(
  id: "profile-id",
  pipeline: "readable",
  typography: (font: "Profile Sans", size: "19px"),
)

#let svg = mermaid-svg(
  source,
  profile: profile,
  id: "direct-id",
  typography: (font: "Direct Sans", size: "21px"),
  options: (
    svg: (
      diagram_id: "options-id",
      pipeline: "readable",
    ),
    host_theme: (
      font_family: "Options Sans",
      font_size: "17px",
    ),
  ),
)

#assert(svg.contains("options-id"), message: "options should set the final diagram id")
#assert(svg.contains("Options Sans"), message: "options should bypass profile and direct typography")
#assert(svg.contains("17px"), message: "options should bypass profile and direct font size")
#assert(not svg.contains("direct-id"), message: "direct id should not override options")
#assert(not svg.contains("Direct Sans"), message: "direct typography should not override options")
#assert(not svg.contains("Profile Sans"), message: "profile typography should not override options")

Options escape hatch fixture passed.
