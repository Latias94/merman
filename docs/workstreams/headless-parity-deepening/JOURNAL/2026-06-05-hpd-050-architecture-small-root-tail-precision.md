# HPD-050 - Architecture Small Root Tail Precision

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

After the edge curve-style relocation fix and root-edge attribution diagnostics, the remaining
focused Architecture rows `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` and
`stress_architecture_nested_groups_002` are both `2.5px` root-width tails.

This pass decides whether those two rows justify another production formula change. It does not
change renderer code, layout code, stored SVGs, root overrides, or baselines.

## Evidence

- `target/compare/architecture-render-path-source-frame-002-093-main-hpd050`
- `target/compare/architecture-delta-root-tail-attribution-002-093-main-hpd050`
- Rejected no-output-change experiment:
  `target/compare/architecture-delta-cy-node-default-family-experiment-hpd050`

Key current facts:

- The render-path probe reports `rendered/stored facts match: true` for both `002` and `093`, so
  the browser-side root facts used by the focused reports match the stored upstream SVGs.
- `093` remains `-2.5px` wide:
  - root left is owned by `group-left`, with local-minus-render delta `+2.730461px`;
  - root right is owned by `group-right`, with local-minus-render delta `+0.230461px`;
  - the owner-span delta is therefore `-2.500000px`.
- `093` group SVG facts explain the owner span:
  - `left` group is `-3px` narrower locally;
  - `right` group is `-1px` narrower locally;
  - service positions are uniformly shifted by about `+1.230469px` in X.
- `002` remains `+2.5px` wide:
  - root left is owned by top-level `service-ingress`, with delta `+1.250000px`;
  - root right is owned by `group-platform`, with delta `+3.750000px`;
  - the owner-span delta is therefore `+2.500000px`.
- `002` group SVG facts are not a simple group-width expansion:
  - `platform` is `-0.5px` narrower locally, but shifted `+4.25px` in X;
  - `data` is also `-0.5px` narrower locally and shifted `+4.25px`;
  - child-group parent-input rows show `data` input width `-2.5px` and `runtime` input width
    `-2px`, so the parent edge tail mixes nested child-group consumption with service placement.
- Root padding is stable in both rows:
  - `093` uses about `30px` on both X edges;
  - `002` uses about `40px` on both X edges.

## Negative Experiment

A temporary experiment changed Architecture Cytoscape node-label measurement to stop explicitly
using the Mermaid SVG Trebuchet font stack, matching the Mermaid Cytoscape stylesheet's
`node[label]` rule more literally: the upstream stylesheet sets `font-size` but not
`font-family`.

The focused deltas did not move:

| fixture | current delta | experiment delta |
|---|---:|---:|
| `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` | `-2.500` | `-2.500` |
| `stress_architecture_nested_groups_002` | `+2.500` | `+2.500` |

The experiment was reverted. Do not pursue the Cytoscape node-label font-family switch as a
production fix for these root tails.

## Classification

The current `2.5px` tails are now precise enough for engineering triage, but not root-cause closed
at pixel parity:

1. `093` is a final group-edge owner tail whose visible root span is already reduced from the old
   large relocation error to a small group/service label contribution lattice.
2. `002` is a mixed top-level service plus parent-group owner tail, with nested child-group
   parent-input evidence and a uniform service/group X-position offset.
3. Both rows reject root padding, final group padding, global label scale, exact labelWidth lookup,
   and Cytoscape font-family switching as standalone fixes.

## Outcome

No production behavior changed.

Treat `002` and `093` as diagnostic small-root tails unless a reusable generated measurement or
phase-specific final-bbox model emerges from broader Architecture evidence. The next useful
HPD-050 target remains the larger direct service label/content rows
`stress_architecture_batch5_long_titles_and_punct_076`,
`stress_architecture_html_titles_and_escapes_041`, and
`stress_architecture_unicode_and_xml_escapes_019`; any candidate fix must also keep `002` and
`093` stable and preserve family-level Architecture `parity-root`.

## Verification

- `git diff --check` - passed.
- `cargo fmt --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed across the implemented matrix.
