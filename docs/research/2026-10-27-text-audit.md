# Requirement SVG role measurement follow-up

Date: 2026-10-27. Maintenance PR: #247; main companion: #246.
This branch targets pinned Mermaid 12.1.0.

## Adaptation

The shared SVG normal-word reset also removes Requirement's bold name role from
measurement on this branch. Its existing writer still emits bold name words.
The main CI exposes the same semantic mismatch through its newer source-font
contract; the maintenance implementation needs its own adaptation.

Keep the ordinary-word wrapping probe, then apply the existing bold role only to
the first row of the final node-label bbox. The role adapter delegates
computed-length, row and createText bbox-Y measurements to the same host,
preserving cancellation.
HTML and relationship-label measurement retain their existing paths. Main's
newer source-font/evidence architecture is not imported.

The new face-sensitive test measures `WW WW` as 50px in normal type and 100px in
bold under a 60px wrapping budget. Both cases stay on one row. The final geometry
retains the selected face and the established six-pixel node-label height
padding. Applying bold before wrapping or dropping bold from the final bbox
fails this contract.

Independent review catches the initial all-row override: both pinned
`requirementBox` versions restyle only `text.children[0]`, the first outer tspan.
The writer now limits its bold words to that row. Measurement combines that
row with the ordinary tail at the emitted 1.1em row offset, retaining the host's
bbox Y. Multi-row width/height cases and a rendered-DOM contract verify that
later normal/Markdown words retain their own faces.
Leading empty rows are trimmed before measuring the tail block; their original
indices still determine its Y offset. Native-like tests skip empty-row ink and
cover an omitted code span between visible rows, an empty first row and an empty
tail.

## CI classification

Run `40336772044` initially fails while starting the Linux VS Code Electron
window, before extension assertions run. One rerun passes all 59 selected checks,
including `pr-gate`; four other checks are intentionally skipped. The extension
runner is unchanged because the evidence does not establish a source defect.

This successful run belongs to the preceding head `0a350616b`, not to the new
Requirement adaptation. Final local checks and new-head CI status are recorded
only after they complete.

## Final local verification

The complete renderer run with `all-diagrams,layout-elk,layout-cytoscape,math`
passes 2,458 tests, with two existing skips, after both review corrections.
Strict renderer-library Clippy and the exact release-mode 37-family SVG
structure owner pass. Formatting, indexed path lengths and diff checks pass.
Focused independent review confirms the first-row and empty-row fixes with no
remaining blocker in that reviewed change. Upstream SVGs, comparator policy and
residual receipts are unchanged. Remote CI and its browser paint audit must
qualify the replacement head independently.

## Broader audit

The main source audit finds additional measurement/output divergences in Pie,
GitGraph, Timeline, XYChart, Architecture and the public HTML measurement helper.
Those findings are not backported as unverified bulk changes. The design
recommendation is to share the visible text, final font and measurement stage
within prepared labels, retaining family-specific SVG/HTML semantics and final
output verification. A complete text-system rewrite is not justified.
