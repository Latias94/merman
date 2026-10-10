# Typst WASM size baseline comparison

Date: 2026-09-14

The `wasm-size-matrix --surface typst` gate was run against the current source and a clean
worktree at `2a92c56b6`. Both builds use the same locked dependency graph, `wasm-size` profile,
`wasm32-unknown-unknown`, and `analysis+layout-cytoscape+layout-elk+svg` feature set.

| metric | baseline `2a92c56b6` | current | budget |
| --- | ---: | ---: | ---: |
| raw bytes | 18,722,522 | 18,722,650 | 15,000,000 |
| stripped bytes | 11,579,933 | 11,580,095 | 10,200,000 |
| gzip bytes | 4,438,859 | 4,438,960 | 3,925,000 |
| brotli bytes | 3,275,876 | 3,276,943 | 2,750,000 |

Both revisions fail the existing four Typst budgets. The differences are tiny and are limited to
post-baseline documentation and qualification metadata; there is no evidence that the theme
refactor introduced the multi-megabyte overage. The budget failure remains an independent release
configuration decision. Raising the budget requires a separately reviewed artifact-size decision;
this record does not change the budget or claim C7a closure.
