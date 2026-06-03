# HPD-050 Architecture FCoSE Compound Bounds Output

Date: 2026-06-04

## Summary

Added a local evidence seam for Architecture compound bounds without changing SVG output.
`manatee::algo::fcose::IndexedLayoutResult` now carries final layout-base compound rectangles in
`compound_bounds`, and `ArchitectureDiagramLayout` maps them to group ids as
`fcose_compound_bounds`. The Architecture renderer does not consume this field.

`xtask debug-architecture-delta` now writes a `Local FCoSE compound bounds vs emitted group rects`
table so local FCoSE final compound rects can be compared directly with the local emitted SVG group
rects rebuilt by `GroupRectComputer`.

## Findings

- The new field confirms the local layout engine had final compound rectangles, but the width/height
  phase was not exposed outside `manatee`.
- The focused `batch5`, `html_titles`, and `unicode` reports show FCoSE layout-base compound rects
  are not a direct substitute for emitted Architecture group rects:
  - `pipeline`: emitted group rect is `dx=-35`, `dy=+37.5`, `dw=+107`, `dh=+22` versus the FCoSE
    compound rect.
  - `ui`: emitted group rect is `dx=+13`, `dy=+37.5`, `dw=+44`, `dh=+22`.
  - `i`: emitted group rect is `dx=+15`, `dy=+37.5`, `dw=+32`, `dh=+22`.
- The same reports still show the upstream/local emitted group-width tails as `+5px`, `+5px`, and
  `+3px`, so the exposed FCoSE rects explain phase separation rather than closing the residual.

## Evidence

- `target\compare\architecture-delta-fcose-compound-bounds-hpd050`
- `stress_architecture_batch5_long_titles_and_punct_076.md`
- `stress_architecture_html_titles_and_escapes_041.md`
- `stress_architecture_unicode_and_xml_escapes_019.md`

## Verification

- `cargo nextest run -p manatee indexed_layout_matches_string_graph_layout_for_compound_constraints`
- `cargo nextest run -p merman-render architecture_layout_exposes_fcose_compound_bounds_by_group_id`
- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-fcose-compound-bounds-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-fcose-compound-bounds-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --out target\compare\architecture-delta-fcose-compound-bounds-hpd050`
- `cargo nextest run -p manatee`
- `cargo nextest run -p merman-render --test architecture_layout_test`
- `cargo nextest run -p xtask`
- `cargo fmt --check -p manatee -p merman-render -p xtask`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual Boundary

Do not wire `fcose_compound_bounds` into SVG group rect rendering as a shortcut. It is a local
layout-base phase probe, not Mermaid/Cytoscape `node.boundingBox()` parity. The active direct
group-width rows still point at the child service-label/content union feeding `GroupRectComputer`.
