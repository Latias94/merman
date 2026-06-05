# HPD-050 - Architecture Direct Service Tail Render-Path Revalidation

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

After classifying the small `002` / `093` root tails as diagnostic owner-edge residuals, the next
Architecture residuals with production potential are the direct service label/content rows:

- `stress_architecture_batch5_long_titles_and_punct_076`
- `stress_architecture_html_titles_and_escapes_041`
- `stress_architecture_unicode_and_xml_escapes_019`

This pass regenerated actual `mermaid.render(...)` render-path probes on current `main` and joined
them with the existing browser FCoSE label-contribution probe plus local SVG delta reports.

## Evidence

- `target/compare/architecture-render-path-direct-service-tails-main-hpd050`
- `target/compare/architecture-delta-direct-service-tail-render-path-main-hpd050`
- Existing label-contribution probe batch:
  `target/compare/architecture-fcose-probe-label-contribution-active-residuals-hpd050`

Commands:

- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-direct-service-tails-main-hpd050`
- `MANATEE_FCOSE_DEBUG_TRACE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-direct-service-tails-main-hpd050 --out target\compare\architecture-delta-direct-service-tail-render-path-main-hpd050`

## Findings

All three render-path probes reported `facts match: true`, so the joined facts are from the actual
installed Mermaid render path and match the stored upstream SVGs.

The current focused deltas are unchanged from the known direct-tail class:

| fixture | root width delta | group | group dx | group dw |
|---|---:|---|---:|---:|
| `076` | `+5.000` | `pipeline` | `-3.500` | `+5.000` |
| `041` | `+5.000` | `ui` | `-1.500` | `+5.000` |
| `019` | `+3.000` | `i` | `-4.500` | `+3.000` |

The group content decomposition still splits cleanly:

| fixture | content dw | expansion dw | emitted/root dw |
|---|---:|---:|---:|
| `076` | `+3.000` | `+2.000` | `+5.000` |
| `041` | `+3.000` | `+2.000` | `+5.000` |
| `019` | `+1.000` | `+2.000` | `+3.000` |

Boundary service attribution explains the content component:

| fixture | left owner delta | right owner delta | content edge dw |
|---|---:|---:|---:|
| `076` / `pipeline` | `storage=-2.500` | `registry=+0.500` | `+3.000` |
| `041` / `ui` | `web=-0.500` | `origin=+2.500` | `+3.000` |
| `019` / `i` | `metrics=-3.500` | `store=-2.500` | `+1.000` |

The height side remains the already-known cancellation: content `dh=-2` and final expansion
`dh=+2`, so a width-only improvement that changes group expansion risks reintroducing height
tails.

## Outcome

No production behavior changed.

This revalidation keeps the next production candidate narrow: it must model service child-label
contribution, final group expansion, and root SVG consumption together. The existing evidence still
rejects standalone exact `labelWidth` lookup, global label scaling, root padding, group padding, or
font-family switching:

- exact `labelWidth` lookup previously reduced these focused rows to `+2px`, but raised full
  Architecture root mismatches to `25` and shifted `093` to `-8px`;
- combining exact lookup with smaller final group expansion made focused widths exact but made
  heights `2px` short.

The next useful implementation work should only start if a candidate handles both axes and
survives full Architecture verification, not just the three focused widths.

## Verification

- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-direct-service-tails-main-hpd050` -
  passed; all three fixtures reported `facts match: true`.
- `MANATEE_FCOSE_DEBUG_TRACE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --fixture stress_architecture_html_titles_and_escapes_041 --fixture stress_architecture_unicode_and_xml_escapes_019 --probe-dir target\compare\architecture-fcose-probe-label-contribution-active-residuals-hpd050 --render-probe-dir target\compare\architecture-render-path-direct-service-tails-main-hpd050 --out target\compare\architecture-delta-direct-service-tail-render-path-main-hpd050` -
  passed.
- `git diff --check` - passed.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
