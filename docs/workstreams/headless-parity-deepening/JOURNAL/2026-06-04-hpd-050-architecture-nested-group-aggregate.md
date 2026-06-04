# HPD-050 - Architecture Nested Group Aggregate Delta Report

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

After adding the local delta batch index, the current top Architecture residual batch was
regenerated without the stale `group_port_edges_017` queue item as the main line. The reports showed
that `nested_groups_002` still had a source-phase blind spot: the existing direct-service group
content table correctly printed `<none>` for the parent group `platform`, because `platform`
contains child groups rather than direct services.

That made the nested parent residual harder to audit from source-backed report output. Browser
`childrenBoundingBoxIncludeLabels` includes child group nodes, while the local report only exposed
direct service contribution bounds.

## Outcome

- Added `architecture_group_parent_map(...)` to read `groups[].in` from the Architecture semantic
  model.
- Added `Group aggregate child attribution` to `debug-architecture-delta --probe-dir`.
- The new table combines local direct service contribution bounds with direct child-group emitted
  rects, then compares that aggregate with browser `childrenBoundingBoxIncludeLabels`.
- Preserved the existing direct-service table because it is still the sharper source seam for
  service child contribution bugs.
- Regenerated the current top Architecture residual batch under
  `target\compare\architecture-delta-current-top-residuals-hpd050`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Focused Snapshot

The generated `nested_groups_002` report now shows:

| group | direct services | child groups | content dw | content dh | expansion dw | expansion dh | emitted dw |
|---|---:|---|---:|---:|---:|---:|---:|
| `platform` | 0 | `data, runtime` | `-0.500000` | `0.000000` | `0.000000` | `0.000000` | `-0.500000` |
| `runtime` | 2 | `<none>` | `-2.000000` | `-2.000000` | `2.000000` | `2.000000` | `0.000000` |
| `data` | 2 | `<none>` | `-2.500000` | `-2.000000` | `2.000000` | `2.000000` | `-0.500000` |

This means the parent `platform` row is no longer opaque. Its local aggregate child content is
`0.5px` narrower than browser, while expansion matches. The remaining root width tail still needs a
source-backed phase model; it is not evidence for a global group padding or final-rect change.

## Verification

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_probe_join` - passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --out target\compare\architecture-delta-current-top-residuals-hpd050` -
  passed and wrote the indexed current top-residual batch.
- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

## Residual Boundary

This is evidence tooling only. It makes nested group residuals reviewable from report output, but it
does not change Architecture residual classification or production layout.
