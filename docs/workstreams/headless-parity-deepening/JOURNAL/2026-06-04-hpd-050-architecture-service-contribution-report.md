# HPD-050 Architecture Service Contribution Report

Date: 2026-06-04

## Summary

Added a stable local evidence surface for Architecture service child contribution bounds. The
layout now exposes `cytoscape_service_bounds` with each service's body, label, and union bounds,
and `xtask debug-architecture-delta` writes those rows in Markdown.

This replaces the previous need to read group content inputs from
`MERMAN_ARCH_DEBUG_GROUP_RECT` stderr when auditing the direct group-width residual rows.

## Findings

- The focused `batch5`, `html_titles`, and `unicode` delta reports now show local service child
  union rows directly beside the FCoSE compound-vs-emitted table and upstream/local element deltas.
- Representative local child contribution rows:
  - `batch5` / `pipeline` / `storage`: union `225x97`.
  - `html_titles` / `ui` / `web`: union `129x97`.
  - `unicode` / `i` / `metrics`: union `125x97`.
- The root/group residuals remain unchanged in those reports: emitted local-vs-upstream group
  widths are still `+5px`, `+5px`, and `+3px`.

## Evidence

- `target\compare\architecture-delta-service-contribution-hpd050`
- `stress_architecture_batch5_long_titles_and_punct_076.md`
- `stress_architecture_html_titles_and_escapes_041.md`
- `stress_architecture_unicode_and_xml_escapes_019.md`

## Verification

- `cargo nextest run -p merman-render architecture_layout_exposes_cytoscape_service_child_bounds_by_service_id`
- `cargo nextest run -p merman-render --test architecture_layout_test`
- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-service-contribution-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-service-contribution-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --out target\compare\architecture-delta-service-contribution-hpd050`
- `cargo nextest run -p merman-render --test architecture_svg_test`
- `cargo nextest run -p xtask`
- `cargo fmt --check -p merman-render -p xtask`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual Boundary

Do not treat `cytoscape_service_bounds` as a generic root-bounds source. It is a local Architecture
child contribution phase probe that makes `GroupRectComputer` inputs auditable; SVG root bounds and
final group rects still need phase-specific rules.
