# M15C-070 Flowchart Styled Text Root Pin

Date: 2026-06-01

## Scope

Refresh the stale root viewport override for the styled text/color Flowchart strict-root row.

## Diagnosis

`upstream_cypress_flowchart_spec_27_set_text_color_of_nodes_and_links_according_to_styles_when_ht_027`
failed strict-root with the existing root override enabled:

- Upstream Mermaid 11.15 root: `viewBox="0 0 370.53125 373.40625"`,
  `max-width: 370.531px`.
- Existing local root pin: `viewBox="0 0 376.296875 373.40625"`,
  `max-width: 376.297px`.

With root overrides disabled, the renderer was much closer but still had a small root residual
(`370.75px` local versus `370.53125px` upstream). This is kept as a narrow root pin rather than a
broad renderer change in this slice.

## Changes

- Refreshed the existing Flowchart root override for the styled text/color fixture to the Mermaid
  11.15 root.
- No new root override entry was added.

## Validation

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_spec_27_set_text_color_of_nodes_and_links_according_to_styles_when_ht_027 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  still fails with only the small unpinned root residual (`370.75px` local versus `370.53125px`
  upstream).
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_spec_27_set_text_color_of_nodes_and_links_according_to_styles_when_ht_027 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all`:
  passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`:
  passed; root viewport overrides remain at `282` total entries and text lookup entries remain
  capped at `495`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still fails as expected with 65 Flowchart strict root-only mismatches, down from 66.

## Follow-Up

Continue M15C-070 with `upstream_docs_flowchart_parameters_136`, which remains a real unpinned
root/text residual, then sample shape-mix and demo flowchart 010/049.
