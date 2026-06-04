# HPD-050 - Architecture Root Revalidation After HPD-090

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

HPD-090 closed the Mermaid 11.15 baseline preparation queue and confirmed full implemented-matrix
DOM parity. Before resuming Architecture source-backed residual work, the Architecture
`parity-root` diagnostic needed a fresh current-HEAD report so the next audit does not continue
from stale pre-baseline artifacts.

## Outcome

- Regenerated Architecture structural DOM parity and root diagnostic reports after HPD-090 closeout.
- Architecture structural DOM parity is still green.
- Architecture `parity-root` remains an expected diagnostic failure with `25` root/style width
  mismatch rows.
- The leading active root queue is unchanged:
  - `stress_architecture_junction_fork_join_026`: `+13.976px`
  - `stress_architecture_batch5_long_titles_and_punct_076`: `+5.000px`
  - `stress_architecture_html_titles_and_escapes_041`: `+5.000px`
  - `stress_architecture_unicode_and_xml_escapes_019`: `+3.000px`
  - `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`: `-2.500px`
  - `stress_architecture_nested_groups_002`: `+2.500px`
- `stress_architecture_group_port_edges_017` is zero-delta in the fresh all-row report and should
  not be reopened from older pre-Procrustes diagnostics.
- No renderer, layout, fixture, baseline, or source code changed.

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd090_closeout_revalidation.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd090_closeout.md` -
  expected diagnostic failure with `25` root/style mismatch rows.

## Next

- Continue Architecture work from `junction_fork_join_026` or the direct group-width rows only with
  source-backed FCoSE/Cytoscape phase evidence.
- Do not tune group padding, root padding, font family, exact label-width lookup, or one-off root
  pins to shrink this diagnostic count.
