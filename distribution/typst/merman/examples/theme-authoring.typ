#import "@preview/merman:0.7.0": export-theme-preset, mermaid, resolve-theme

#let studio = export-theme-preset("studio")
#let dark = resolve-theme(studio, appearance: "dark")
#assert(dark.ok)
#mermaid(
  "flowchart LR\nA[Studio] --> B[Dark appearance]",
  theme: studio,
  appearance: "dark",
  document-context: true,
  width: 90%,
)

#let custom = (
  schema_version: 2,
  extends: "studio",
  seed: (accent: "#0052CC"),
  typography: (sans: ("Source Sans 3", "Arial", "sans-serif")),
)
#let custom-json = json.encode(custom, pretty: true)
// Save the JSON as custom.theme.json and read it with json("custom.theme.json").
#let imported = json(bytes(custom-json))
#let check = resolve-theme(imported, auto-fix-contrast: true)
#assert(check.ok)
#mermaid(
  "flowchart LR\nA[Editable JSON] --> B[Brand accent]",
  theme: imported,
  width: 90%,
)

#raw(custom-json, lang: "json", block: true)
