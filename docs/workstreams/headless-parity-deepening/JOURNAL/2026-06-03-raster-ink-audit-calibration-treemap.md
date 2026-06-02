# HPD-080 Raster Ink Audit Calibration And Single-Leaf Treemap

Date: 2026-06-03
Task: HPD-080 visible rendering defect triage

## Context

The PNG ink gate was deliberately stronger than "resvg produced bytes", but the first broad
supported-family raster pass exposed two different classes:

- source-detector false positives for fixtures that parse but intentionally have no visible diagram
  marks;
- a real headless renderability failure where a contentful Treemap rendered as an all-background PNG.

The gate should catch the second class without pretending the first class is broken output.

## Source Checks

Pinned Mermaid source commit:

- `41646dfd43ac83f001b03c70605feb036afae46d`

Checked source and fixture families:

- `packages/mermaid/src/diagrams/treemap/renderer.ts`
- `packages/mermaid/src/diagrams/treemap/styles.ts`
- `fixtures/journey/upstream_acctitle_only.mmd`
- `fixtures/packet/upstream_packet_beta_header_spec.mmd`
- `fixtures/radar/upstream_pkgtests_radar_spec_006.mmd`
- `fixtures/treemap/upstream_pkgtests_treemap_test_014.mmd`
- `fixtures/treemap/upstream_pkgtests_treemap_test_032.mmd`

Important findings:

- Journey `section ...` lines without any task rows do not produce visible Journey marks.
- `packet-beta` header-only input is parser/header coverage, not visible content.
- Radar option-only inputs such as `ticks`, `showLegend`, `graticule`, `min`, and `max` can produce
  no visible axes or curves.
- Treemap root-only/classDef-only inputs have no value-bearing leaf and should not require ink.
- A single top-level Treemap value does have visible semantic content, but Mermaid 11.15 assigns the
  first leaf fill from a color scale whose first range entry is `transparent`, while the first label
  color comes from `cScaleLabel0`. In the default theme that combination becomes white text on a
  transparent cell over the white root background.

## Outcome

- The source-content detector now tracks diagram kind and treats Journey section-only, Packet
  `packet-beta` header-only, Radar option-only, and Treemap no-value/classDef-only inputs as
  non-visual for the PNG ink requirement.
- Treemap keeps Mermaid's transparent first leaf fill, but when a leaf has transparent fill, no
  explicit class/style fill override, and a white/near-white generated label color, the renderer uses
  `themeVariables.textColor` for the leaf label/value inline fill. This follows Mermaid's own
  Treemap CSS provider default for `.treemapLabel` / `.treemapValue` and avoids preserving an
  unreadable headless output only for byte parity.
- Explicit Treemap classDef fill/color styles remain respected; the readability fallback is narrowly
  scoped to the transparent-cell/white-label combination.

## Verification

- `cargo fmt -p merman-render -p merman`
- `cargo fmt -p merman-render -p merman --check`
- `cargo nextest run -p merman-render --test treemap_svg_test treemap_single_leaf_label_uses_readable_fill_over_transparent_cell`
- `cargo nextest run -p merman-render --test treemap_svg_test`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke source_content_gate_distinguishes_accessibility_only_from_visible_content`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='treemap'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='gitgraph,kanban,timeline,journey'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='treemap,pie,quadrantchart,xychart'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='radar,requirement,packet,sankey,c4'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_pkgtests_treemap_test_032`

## Residual

The unfiltered multi-family raster command can exceed a five-minute tool timeout, so broad PNG-level
triage should stay split by `MERMAN_RESVG_SAFE_AUDIT_FAMILY`. This slice does not claim visual diff
parity; it closes one gross readability failure and tightens the source-content gate so future
failures are actionable.
