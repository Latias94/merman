#import "@preview/merman:0.1.0": mermaid-profile, mermaid-svg

#let source = "flowchart LR
  A[Profile font] --> B[Typst wrapper]
"

#let profile = mermaid-profile(
  id: "profile-typography",
  pipeline: "readable",
  background: "#ffffff",
  typography: (
    font: ("Profile Sans", "Arial", "sans-serif"),
    size: "19px",
  ),
)

#let profile-svg = mermaid-svg(source, profile: profile)
#assert(profile-svg.contains("profile-typography"), message: "profile should set diagram id")
#assert(profile-svg.contains("Profile Sans"), message: "profile typography should set font family")
#assert(profile-svg.contains("19px"), message: "profile typography should set font size")

#let direct-svg = mermaid-svg(
  source,
  profile: profile,
  typography: (
    font: "Direct Sans",
    size: "21px",
  ),
)
#assert(direct-svg.contains("Direct Sans"), message: "direct typography should override profile typography")
#assert(direct-svg.contains("21px"), message: "direct typography should override profile font size")
#assert(not direct-svg.contains("Profile Sans"), message: "profile font should not win over direct typography")

#let host-theme-svg = mermaid-svg(
  source,
  profile: profile,
  typography: (font: "Direct Sans", size: "21px"),
  host-theme: (
    font_family: "Host Theme Sans",
    font_size: "23px",
  ),
)
#assert(host-theme-svg.contains("Host Theme Sans"), message: "explicit host-theme should override typography")
#assert(host-theme-svg.contains("23px"), message: "explicit host-theme font size should override typography")
#assert(not host-theme-svg.contains("Direct Sans"), message: "typography should not win over explicit host-theme")

Profile typography fixture passed.
