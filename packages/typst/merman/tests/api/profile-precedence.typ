#import "@preview/merman:0.1.0": mermaid-profile, mermaid-svg

#let source = "flowchart LR
  A[Profile] --> B[Precedence]
"

#let profile = mermaid-profile(
  id: "profile-id",
  pipeline: "readable",
  background: "#ffffff",
  typography: (
    font: "Profile Sans",
    size: "19px",
  ),
  host-theme: (
    font_family: "Profile Host Sans",
    font_size: "20px",
  ),
  layout: (
    text_measurer: "deterministic",
    viewport_width: 321,
  ),
)

#let direct-svg = mermaid-svg(
  source,
  profile: profile,
  id: "direct-id",
  typography: (
    font: "Direct Sans",
    size: "21px",
  ),
  host-theme: (
    font_family: "Direct Host Sans",
    font_size: "22px",
  ),
  viewport-width: 640,
)

#assert(direct-svg.contains("direct-id"), message: "direct id should override profile id")
#assert(not direct-svg.contains("profile-id"), message: "profile id should not win over direct id")
#assert(direct-svg.contains("Direct Host Sans"), message: "direct host-theme should override profile host-theme")
#assert(direct-svg.contains("22px"), message: "direct host-theme size should override profile host-theme size")
#assert(not direct-svg.contains("Profile Host Sans"), message: "profile host-theme should not win over direct host-theme")
#assert(not direct-svg.contains("Direct Sans"), message: "host-theme should win over direct typography")

#let options-svg = mermaid-svg(
  source,
  profile: profile,
  id: "direct-id",
  host-theme: (
    font_family: "Direct Host Sans",
    font_size: "22px",
  ),
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

#assert(options-svg.contains("options-id"), message: "options should override direct and profile id")
#assert(options-svg.contains("Options Sans"), message: "options should bypass direct and profile host-theme")
#assert(options-svg.contains("17px"), message: "options should bypass direct and profile font size")
#assert(not options-svg.contains("direct-id"), message: "direct id should not override options")
#assert(not options-svg.contains("Direct Host Sans"), message: "direct host-theme should not override options")

Profile precedence fixture passed.
