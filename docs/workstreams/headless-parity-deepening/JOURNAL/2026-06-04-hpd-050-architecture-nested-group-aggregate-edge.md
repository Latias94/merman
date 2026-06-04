# HPD-050 - Architecture Nested Group Aggregate Edge Attribution

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The nested group aggregate table made `nested_groups_002/platform` visible by combining direct
service contribution bounds with direct child-group emitted rects. It still stopped at aggregate
`content dw=-0.5`, so reviewers had to manually identify which child group owned the left, right,
top, and bottom boundaries.

## Outcome

- Added `Group aggregate edge attribution` to `debug-architecture-delta --probe-dir`.
- The new table uses the same aggregate inputs as the nested content table:
  - browser direct service child unions and child-group `node.boundingBox()` values
  - local direct service contribution bounds and child-group emitted rects
- Reused the existing edge-owner logic already used by direct-service attribution.
- Regenerated the current top Architecture delta batch under
  `target\compare\architecture-delta-current-top-aggregate-edge-hpd050`.
- Kept renderer output, layout formulas, SVG fixtures, and baselines unchanged.

## Focused Snapshot

For `nested_groups_002/platform`, the generated report now shows:

| group | child groups | left owner | left dx | right owner | right dx | edge dw | top owner | top dy | bottom owner | bottom dy | edge dh |
|---|---|---|---:|---|---:|---:|---|---:|---|---:|---:|
| `platform` | `data, runtime` | `data` | `44.250000` | `data` | `43.750000` | `-0.500000` | `runtime` | `40.000000` | `data` | `40.000000` | `0.000000` |

This means the parent aggregate width tail is now attributable to child-group boundary drift. The
final group expansion still matches, and there are no direct services under `platform`.

## Verification

- `cargo fmt -p xtask` - passed.
- `cargo nextest run -p xtask architecture_probe_join_reports_nested_group_aggregate_content architecture_probe_join_decomposes_group_and_service_bounds` -
  passed, `2` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --fixture stress_architecture_nested_groups_002 --probe-dir target\compare\architecture-fcose-probe-active-residuals-hpd050 --out target\compare\architecture-delta-current-top-aggregate-edge-hpd050` -
  passed and wrote the aggregate-edge batch.
- `cargo fmt --check -p xtask` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p xtask` - passed, `100` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; Architecture structural parity stayed green.

## Residual Boundary

This is evidence tooling only. It makes nested child-group boundary ownership explicit, but it does
not change Architecture residual classification or justify a production layout tweak.
