---
type: Verification Record
title: Alpha.7 preset usability and dense export review
timestamp: 2026-09-20
source_commit: 226ef06512249d044003b35c3774c7bc28bad897
---

# Scope

This is a bounded usability review for the ten existing preset IDs. It uses the current release
CLI recipe, native text configuration (`htmlLabels=false`), and four scenes: one pinned Modern
Mermaid Flowchart example, one pinned Mindmap example, the existing dense Class scene, and the
existing four-series XY scene from the Apple smoke fixture. It is visual evidence for product
guidance; it does not qualify catalog cells or close C7a.

# Reproducible run

The CLI was rebuilt with the descriptor-owned `cli-release` profile using `--build-host --locked`
on the current source. The build completed in the `dist` profile with Rust `1.95.0` on macOS ARM64.
The run recipe, source files, outputs, hashes, and the rejected first-pass argument log are in
`target/bench/experiments/preset-usability-20260920/`. The pinned Modern Mermaid revision is
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`.

The matrix contains 120 cells (4 scenes × 10 presets × SVG/PNG/PDF). 116 cells produced valid
outputs. The four rejected cells are Cyberpunk Flowchart PNG/PDF and Cyberpunk four-series XY PNG/PDF;
each is the same explicit default resource boundary:

```
resource limit `max_total_svg_conversion_filter_primitives` exceeded during svg_conversion:
actual=132 maximum=128 cause=ceiling
```

The SVG outputs for those cells succeeded. The default budget was not relaxed. PDFKit independently
opened and rasterized all 38 successful PDF files as one-page documents with non-zero media boxes.

# Visual observations

The generated PNG and PDF contact sheets were inspected at enlarged resolution. Dense Class output
keeps the namespace, six entities, members, relation labels, and notes visible in all ten presets;
dark presets retain light text and light presets retain dark text. The Cyberpunk Class remains
visually legible, but its neon treatment is visually dense and is better suited to presentation
slides than document body copy.

The Mindmap sample retains readable Chinese and Latin labels in all ten presets. Branch groups remain
visually separated by color; the dark presets keep the central white label and do not reproduce the
earlier suspected low-contrast failure. This observation is limited to this source, native text path,
and host font environment.

The four-series XY output preserves two bar series and two line series in every successful preset.
The bars and lines remain distinguishable in grayscale by geometry and placement, although this is not
a WCAG or color-vision certification. PDF previews preserve the same axes, labels, bars, and lines.

The pinned Flowchart is readable across the nine successful export groups. Cyberpunk's SVG remains
available for browser inspection, while its PNG/PDF rejection shows that its effect composition can
exceed the default native conversion budget on a moderately dense flow. This is an export suitability
boundary, not evidence that the preset is malformed.

# Product guidance and limits

Editor Light/Dark, One Dark, Gruvbox Light/Dark, Ayu Light/Dark are reasonable broad base themes for
documentation and interactive diagrams in this bounded sample. Brutalist is strongest for outlined
documents and posters; Spotless is restrained for instruction material. Cyberpunk is suitable for
selected presentation scenes and should retain an explicit fallback or budget-aware export path for
dense PNG/PDF work.

The review does not measure controlled-font variation, browser HTML-label rendering, memory, cold
start, throughput, package deltas, accessibility conformance, or all diagram families. Those remain
open U10/C7a evidence. The public catalog remains unqualified for these observations.
