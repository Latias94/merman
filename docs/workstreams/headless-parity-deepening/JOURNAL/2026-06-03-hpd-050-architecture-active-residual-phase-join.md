# HPD-050 - Architecture Active Residual Phase Join

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The browser/Cytoscape probe batch and the local delta reports now cover the same seven active
Architecture residual samples. The missing step was a source-backed join between browser final
group bbox phases and local upstream-vs-rendered group `dw` / `dh` rows.

## Outcome

- Joined existing probe JSON artifacts under
  `target\compare\architecture-fcose-probe-active-residuals-hpd050` with existing local delta
  Markdown reports under `target\compare\architecture-delta-active-residuals-hpd050-group-size`.
- Confirmed browser final group `node.boundingBox().w/h` matches the stored upstream SVG group rect
  `w/h` for the non-junction focused group rows.
- Classified `batch5_long_titles_and_punct_076`, `html_titles_and_escapes_041`, and
  `unicode_and_xml_escapes_019` as direct local group-width tails: local group `dw` equals the root
  width delta.
- Classified `nested_groups_002` and `batch6_init_fontsize_icon_size_wrap_093` as mixed
  nested/position aggregation rows, not one global group-width formula.
- Identified `group_port_edges_017` as the clearest phase seam: local outer group height
  `444.603px` equals browser `bbAfterSegments.h`, while the upstream final outer group bbox height
  is `462.448px`.
- Re-ran `junction_fork_join_026` with explicit Edge after the default Puppeteer Chrome cache was
  missing. The rerun reproduced the earlier probe geometry, confirming this row should be treated
  as probe-vs-stored-baseline divergence before it drives production formula work.
- No production code, xtask command, renderer behavior, or root residual status changed.

## Key Table

| fixture | group | browser final group w/h | upstream group w/h | local group dw/dh | root dw/dh |
|---|---|---:|---:|---:|---:|
| `batch5_long_titles_and_punct_076` | `pipeline` | `462.926 / 382.926` | `462.926 / 382.926` | `+5.000 / +0.000` | `+5.000 / +0.000` |
| `html_titles_and_escapes_041` | `ui` | `399.926 / 382.926` | `399.926 / 382.926` | `+5.000 / +0.000` | `+5.000 / +0.000` |
| `unicode_and_xml_escapes_019` | `i` | `389.822 / 383.593` | `389.822 / 383.593` | `+3.000 / -0.000` | `+3.000 / +0.000` |
| `group_port_edges_017` | `outer` | `447.995 / 462.448` | `447.995 / 462.448` | `+0.030 / -17.845` | `+1.468 / -17.845` |
| `junction_fork_join_026` | `left` | `1809.785 / 1626.571` | `1788.557 / 1649.154` | `+17.331 / -18.609` | `+13.976 / -12.502` |

## Verification

- Read-only PowerShell JSON/Markdown join over the seven probe JSON files and seven local delta
  Markdown reports - passed.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture junction_fork_join_026 --out target\compare\architecture-fcose-probe-junction-rerun-hpd050` -
  expected-failed because the local Puppeteer Chrome cache is absent.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture junction_fork_join_026 --out target\compare\architecture-fcose-probe-junction-rerun-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed.

## Residual Boundary

The next implementation candidate should not be a broad root constant. Start with the
`group_port_edges_017` phase seam and prove whether local root/group bounds are using a layout
stage bbox where Mermaid SVG emission uses final compound `node.boundingBox()`. Keep
`junction_fork_join_026` out of formula tuning until the probe-vs-stored-baseline split is
explained.
