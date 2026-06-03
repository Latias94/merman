# HPD-050 Architecture Probe Phase Join

Date: 2026-06-04

## Summary

Added an optional `--probe-dir` input to `xtask debug-architecture-delta`, allowing local
Architecture delta reports to join directly with existing browser/Cytoscape FCoSE probe JSON.

This turns the previous manual phase join into repeatable Markdown output. It remains evidence
tooling only: no Architecture layout formula, SVG renderer, fixture, or baseline behavior changed.

## Report Additions

When `--probe-dir` points at a directory containing
`<fixture>.fcose-browser-probe.json`, each delta report now includes:

- `Group content decomposition`: browser `childrenBoundingBoxIncludeLabels`, local direct-service
  content union, local final emitted expansion, and emitted group `dw` / `dh`.
- `Service bbox join`: local service body/label/union contribution phases beside browser final
  service `bodyBounds`, `labelBounds.all`, `node.boundingBox()`, and position drift.

The group table is explicit that local content is currently direct-service contribution only; nested
group or junction content still needs separate source-backed audit before driving formulas.

## Focused Readings

The new automated output reproduces the earlier hand join for the active direct group-width rows:

| fixture / group | content dw | content dh | expansion dw | expansion dh | emitted dw | emitted dh |
|---|---:|---:|---:|---:|---:|---:|
| `batch5` / `pipeline` | `+3.000` | `-2.000` | `+2.000` | `+2.000` | `+5.000` | `0.000` |
| `html_titles` / `ui` | `+3.000` | `-2.000` | `+2.000` | `+2.000` | `+5.000` | `0.000` |
| `unicode` / `i` | `+1.000` | `-2.000` | `+2.000` | `+2.000` | `+3.000` | `0.000` |

Representative service rows now show the next seam without manual subtraction:

- `batch5` / `storage`: local contribution label width is `+4px` over browser label width; local
  union is `+2px` wide and `-3px` tall versus browser final service bbox; position `dx=-0.5`.
- `html_titles` / `web`: local label width is `+2px`; local union width matches browser final
  service bbox and is `-3px` tall; position `dx=+0.5`.
- `unicode` / `metrics`: local label width is `+4px`; local union is `+2px` wide and `-3px` tall;
  position `dx=-1.5`.

## Evidence

- `target\compare\architecture-delta-probe-phase-join-hpd050`
- `target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050`

## Verification

- `cargo nextest run -p xtask architecture_delta_args_accept_probe_dir architecture_probe_join_decomposes_group_and_service_bounds fcose_probe_markdown_summarizes_stage_and_node_bounds`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-probe-phase-join-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-probe-phase-join-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --out target\compare\architecture-delta-probe-phase-join-hpd050`
- `cargo nextest run -p xtask`
- `cargo fmt --check -p xtask`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

## Residual Boundary

The automated join reinforces the previous boundary. Standalone group padding, root padding,
group-title bounds, final rect emission, and direct FCoSE compound rect substitution remain rejected
for these rows. The next viable source-backed seam is individual service label/content contribution
width, service position drift, and how those feed final group expansion.
