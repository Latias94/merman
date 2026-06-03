# HPD-050 - Architecture Probe Expansion Active-Residual Batch

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The previous slice added `bb over children labels` to Architecture FCoSE probe Markdown. One
focused `batch5_long_titles` run proved the column worked, but the active residual queue needs a
shared batch artifact so future formula work can compare the same phase across the representative
rows.

## Outcome

- Regenerated the seven-fixture active Architecture residual probe batch into
  `target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050`.
- The batch index is
  `target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050\architecture-fcose-probe-batch.md`.
- All seven per-fixture summaries include `bb over children labels`.
- Standard-padding group rows now show the final group expansion directly:
  `l=41.500 r=41.500 t=41.500 b=41.500 dw=83.000 dh=83.000`.
- The custom-init `batch6_init_fontsize_icon_size_wrap_093` groups show
  `l=31.500 r=31.500 t=31.500 b=31.500 dw=63.000 dh=63.000`.
- No code or renderer output changed in this slice; it is evidence collection after the probe
  summary enhancement.

## Verification

- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-expansion-active-residuals-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed.
- Batch output wrote one index plus `7` JSON files and `7` Markdown summaries.
- `Select-String` found `bb over children labels` in all `7` Markdown summaries.
- Focused row extraction confirmed readable expansion rows for `pipeline`, `ui`, `i`, `platform`,
  `data`, `left`, `right`, `inner`, `outer`, and the junction groups.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`567` records) and `WORKSTREAM.json`.

## Residual Boundary

This batch is evidence, not a formula change. It makes the browser/Cytoscape final group expansion
phase easy to cite, but the remaining Architecture residuals still require local delta joins and a
family-level verification gate before any production root-bounds or group-bbox adjustment.
