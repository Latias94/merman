# HPD-050 - Architecture Active Residual Probe Batch

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The Architecture FCoSE probe command already supports repeated fixtures and a batch index. The next
useful step was to use that machinery on the active residual queue instead of extending command
shape again.

## Outcome

- Ran a fresh Architecture `parity-root` diagnostic report:
  `target\compare\architecture_report_parity_root_hpd050_active_residual_probe_prep.md`.
- The report remains an expected failure with `25` Architecture root-only mismatch rows.
- Captured seven representative active residual samples in one batch:
  - `stress_architecture_junction_fork_join_026`
  - `stress_architecture_batch5_long_titles_and_punct_076`
  - `stress_architecture_html_titles_and_escapes_041`
  - `stress_architecture_unicode_and_xml_escapes_019`
  - `stress_architecture_nested_groups_002`
  - `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`
  - `stress_architecture_group_port_edges_017`
- The batch wrote `7` raw JSON artifacts, `7` Markdown summaries, and the index:
  `target\compare\architecture-fcose-probe-active-residuals-hpd050\architecture-fcose-probe-batch.md`.
- The sampled summaries all show `bbBeforeRun2` equal to `bbAfterSegments`, so the captured data is
  ready for final node/edge/child-bounds phase comparison.
- No renderer, layout, measurement constant, probe command shape, or SVG output behavior changed.

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_active_residual_probe_prep.md` -
  expected failure with the current `25` Architecture root-only mismatch rows.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_group_port_edges_017 --out-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote the batch artifacts.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- Line-by-line JSON parse for `docs\workstreams\headless-parity-deepening\CONTEXT.jsonl` - passed,
  `506` JSONL records parsed.

## Residual Boundary

This slice closes an evidence-collection gap only. It does not claim root closure or justify a
production formula change. The next safe move is to compare these browser/Cytoscape final
node/edge/child phases against local Rust measurement/root-bounds phases.
