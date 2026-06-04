# HPD-050 - Architecture Delta Summary Residual Ordering

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

After the service label final-frame report, the next safe move was to refresh the current
Architecture residual queue before selecting another production seam. A fresh `parity-root` run
shows the current queue is `24` root-only mismatches, and `group_port_edges_017` is no longer an
active root mismatch on current HEAD.

The existing `summarize-architecture-deltas` output contained upstream/local max-width values, but
it was sorted by fixture name and lacked an explicit delta column. That made the active residual
queue hard to read from the diagnostic summary and encouraged stale artifact reuse.

## Outcome

- Added `max-width delta` to `xtask summarize-architecture-deltas`.
- Sorted summary rows by absolute max-width delta descending, then fixture name for deterministic
  ties.
- Added a focused unit test for the sorting policy.
- Regenerated the current ordered summary under
  `target\compare\architecture-delta-summary-hpd050-current`.
- Refreshed the current Architecture root report under
  `target\compare\architecture_report_parity_root_hpd050_current.md`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Current Snapshot

The fresh `parity-root` report expected-fails with `24` root-only mismatches. The ordered summary
now starts with:

| fixture | max-width delta | notable local delta |
|---|---:|---|
| `junction_fork_join_026` | `+13.976` | `group max dw=+17.331`, `group max dh=-18.609` |
| `batch5_long_titles_and_punct_076` | `+5.000` | `group max dw=+5.000` |
| `html_titles_and_escapes_041` | `+5.000` | `group max dw=+5.000` |
| `unicode_and_xml_escapes_019` | `+3.000` | `group max dw=+3.000` |
| `batch6_init_fontsize_icon_size_wrap_093` | `-2.500` | `group max dw=-3.000` |
| `nested_groups_002` | `+2.500` | `group max dw=-0.500` |

`group_port_edges_017` reports zero max-width and group deltas in the current summary, so it should
not drive more Architecture root work unless a fresh report regresses.

## Verification

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask architecture_delta_summary_order_sorts_by_abs_max_width_delta_then_stem architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `2` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_current.md` -
  expected-failed with `24` root-only mismatches.
- `cargo run -p xtask -- summarize-architecture-deltas --out target\compare\architecture-delta-summary-hpd050-current` -
  passed and wrote the sorted summary.

## Residual Boundary

This is evidence tooling only. It makes the current Architecture queue easier to read and prevents
stale residual selection, but it does not claim root residual closure or justify a production
layout change.
