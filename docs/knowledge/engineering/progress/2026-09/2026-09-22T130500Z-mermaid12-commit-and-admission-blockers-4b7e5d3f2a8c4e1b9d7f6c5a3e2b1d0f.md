---
title: "Mermaid 12 implementation commit and remaining admission blockers"
type: progress
status: immutable
created_at: 2026-09-22T13:05:00Z
---

# Mermaid 12 implementation commit and remaining admission blockers

## Verified implementation

Commit `cd6d4895c` contains the Mermaid 12 core, ELK, renderer, theme, Sequence, Class, ER, Flowchart, Requirement, and State implementation batch plus focused regression tests. Workspace `cargo check --workspace` passes. The focused Mermaid 12 renderer suite passes 20/20; the combined Class/ER/Sequence suite passes 108/108 with one intentional skip.

## Remaining strict evidence

State full SVG comparison renders 285 fixtures and still reports a bounded set of path structure differences against the pinned upstream files. The pinned State SVGs use Dagre-shaped routes while the product default is ELK; the mismatch is structural layout evidence and must be resolved through an explicit baseline/product decision rather than a comparator relaxation.

The Sequence participant-types loop fixture has one text wrapping boundary where deterministic widths produce 142px for a 143px limit and Chromium wraps the same label. This is a browser text measurement residual; the loop width implementation now follows Mermaid's `loopWidth - 2 * wrapPadding` source semantics.

Class label candidates now show eight stable edges with route and text-width differences after the shared ELK terminal pass. They remain review-required because the evidence still includes route-shape differences; they are not admitted as measurement residuals.

The label residual catalog therefore remains intentionally uncommitted until Class/ER/State evidence is reviewed and signed. WASM size and final alpha.7 release checks remain outstanding.
