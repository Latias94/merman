# Title 029: deterministic and display-owned bounds

## Scope

Investigated `flowchart/stress_flowchart_title_padding_subgraph_029` separately at
`0a04fce1ac6e86b86488e3fe4b440e4ad4848186`. The trusted branch base remains
`2d70832e25497aae282de9da78d1d6db12f2b475`. Mermaid remains 12.0.0,
`mermaid@12.0.0`, source `98a0945418c76238f15df2afaddbba4272656c3b`.
The selection identity remains
`9634181804174022a42a1288be799a7612226a4a95b3374642a32515348b40e7`, with receipt
SHA-256 `5a77f613f2d1f380ddaa5d9bab5e8111adb2cd833f319462a916694bebf10e28`.

This is a diagnosis and product-boundary decision record, not a claim that the
canonical deterministic artifact has been repaired. No production code, reference,
feature, dependency, residual receipt, comparator, or existing test was changed.
The seven pre-existing stat-only working-tree entries and upstream SVG staging
files were preserved.

## Current evidence

Chromium 151.0.7922.34 on Windows reproduced the issue with the current release
comparison SVG. Its SHA-256 is
`a48f7f0a4b8e6b513fecb476d196481b1109bcb5bdb933055e41a657cf83edfa`.
The unchanged root-paint oracle still classifies it as blocking.

| Measurement | Result |
| --- | --- |
| Deterministic root viewBox | `12 -27 376 241` |
| Actual configured-font title width | 391.21875 px |
| Actual title horizontal bounds | 4.390625 to 395.609375 |
| Left/right overflow beyond deterministic root | 7.609375 px each |
| Public browser callback root horizontal bounds | 4.390625 to 395.609375 |
| Public browser callback oracle classification | upstream-inherited |

The actual configured font stack is `"Recursive Variable", arial, sans-serif`.
A separate display-only probe measured 441 px after selecting the browser's serif
or monospace family. This demonstrates that increasing a fixed margin to cover the
initial 7.609375 px overflow does not solve unknown display-font selection.
These measurements are diagnostic evidence, not runtime constants or font tables.

The existing assembled-public-Web regression suite passed all three tests with
fresh WASM transaction `76a16c141fc0`: configured-font title bounds, monospace title
bounds, and its co-located UTF-16 API regression. A declined host callback remains
byte-identical to the deterministic renderer. No Rust implementation changed, so
no Cargo rebuild or new Rust test run was needed.

Local evidence is under `target/title-029-font-boundary/`:

- `deterministic-audit.json` and `.log`: original root-paint oracle, one blocking
  fixture. The one unused XYChart receipt is expected for this single-fixture
  subset; the title's blocking result is independently present.
- `public-browser-tests.json`: public package test results, three passed.
- `host-audit.json`: attached title measurement and unchanged oracle audit.
- `font-selection-evidence.json`: the same canonical SVG displayed with different
  font selections.
- `measure-font-selection.mjs`: temporary display-only probe.

## Implementation assessment

The pinned upstream Flowchart renderer inserts its title before
`setupViewPortForSVG`, which obtains display-owned bounds. Merman's
`svg/parity/flowchart/viewbox.rs` likewise fixes the pre-title anchor, requests
`TitleBBoxX`, and unions its left and right extents independently. Its measurement
and emitted CSS agree on 18 px. The current public browser implementation answers
that request with SVG `getBBox()`.

An independent correctness review found no missing title union, wrong anchor,
wrong font size, or unused host measurement. The discrepancy comes from the
font-agnostic estimate in the absence of a host measurement. This is precisely the
boundary documented by [ADR-0086](../adr/0086-deterministic-text-measurement-without-vendored-font-tables.md).

The native exporter's private system-font database is loaded after SVG generation
for PNG/JPEG/PDF conversion. It does not answer the layout or title measurement
callback, cannot establish the eventual browser's font selection, and cannot be
imported into the renderer without reversing the existing dependency direction.
The `render_svg_monospace` example demonstrates a custom font-backed measurement
profile, but its advance-based symmetric bounds are not a browser-bbox contract.

## Disposition and next implementation boundary

Keep the canonical failure visible. The verified solution for a browser-owned
surface is the existing public browser measurement session, after the intended
fonts are ready. That route is already implemented and passing.

Eliminating this residual for standalone headless output requires a narrower new
contract: identify the supported output surface and supply the actual font assets
used by both measurement and rendering. A native raster/PDF adapter could share
its shaping/font-selection policy with its own exporter. Portable SVG also needs
a defined way to preserve that font selection at its destination. Neither route
can promise exact bounds for arbitrary later CSS or font substitution.

Do not modify the deterministic estimator merely to pass this fixture. Padding
coefficients, font-specific lookup tables, forcing `textLength`, changing the
oracle to use host measurement silently, or relabeling the failure would conceal
the missing font authority. A new optional font-backed capability needs a product
scope decision and the normal dependency, license, size, and platform admission;
it is not required to fix an error in the current title-union implementation.
