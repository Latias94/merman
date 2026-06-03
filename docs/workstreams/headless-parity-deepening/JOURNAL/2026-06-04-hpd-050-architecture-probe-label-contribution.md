# HPD-050 - Architecture Probe Label Contribution Summary

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The Architecture FCoSE probe Markdown already exposed final group bbox expansion over
`childrenBoundingBoxIncludeLabels`. That closed one manual subtraction, but the active residual rows
still required reviewers to subtract `childrenBoundingBoxBodyOnly` from
`childrenBoundingBoxIncludeLabels` to see the label contribution phase.

The next source-backed formula discussion needs the full phase chain in one table:
child body contribution, child label contribution, and final compound group expansion.

## Outcome

- Extended the `Final Node Bounds` Markdown table with `children labels over body`.
- The new column reports `childrenBoundingBoxIncludeLabels` expansion over
  `childrenBoundingBoxBodyOnly` as left, right, top, bottom, `dw`, and `dh`.
- Kept the existing `bb over children labels` column unchanged, so each group row now reads as
  `children body -> children labels -> final node.boundingBox()`.
- Regenerated the seven-fixture active Architecture residual probe batch into
  `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`.
- Representative group rows now show the label phase directly:
  - `batch5_long_titles` `pipeline`: `l=69.500 r=27.500 t=0.000 b=17.000 dw=97.000 dh=17.000`.
  - `html_titles` `ui`: `l=22.500 r=11.500 t=0.000 b=17.000 dw=34.000 dh=17.000`.
  - `group_port_edges` `outer`: zero label expansion over body, while `inner` contributes
    `b=17.000 dh=17.000`.
  - `batch6` custom-init `left` / `right`: label contribution is asymmetric horizontally but the
    final group expansion remains `31.5px` per side.
- No Architecture layout, renderer, SVG, probe JSON, fixture, or baseline behavior changed.

## Verification

- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed,
  `1` test run after the expected red assertion was made green.
- `cargo nextest run -p xtask` - passed, `95` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed.
- The batch wrote one index plus `7` JSON files and `7` Markdown summaries.
- `rg -n "children labels over body" target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 -g "*.md"` -
  found the new column in all `7` per-fixture Markdown summaries.

## Residual Boundary

This is evidence tooling only. The new label-contribution column should be cited beside local delta
reports before any production group-bbox formula change; it does not close Architecture root
residuals or justify tuning final group padding by itself.
