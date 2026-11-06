# Typst raw-link budget after the alpha.2 dependency refresh

The alpha.2 dependency refresh changes only the `typst-wasm` raw-link ceiling from
21,609,000 to 26,285,000 bytes. All 23 other ceilings, including Typst's final stripped,
gzip and Brotli limits, remain unchanged. This follows the maintainer's authorization
to reassess reasonable dependency and toolchain growth; it does not accept a larger
delivered Typst package by exchanging one metric's headroom for another's.

## Recorded measurement

[PR 280 Linux CI run 42177129359](https://github.com/Latias94/merman/actions/runs/42177129359/job/127758171937)
measured test merge `8c210d9f4e9c47b53a54c721b4368a33cc5e16a6`, combining PR head
`662103769885d0cde89101c4e85942283921567f` with main
`1fc62283668c6c909b224aa499d8b98a418c1786`, using Rust `1.99.1`, Binaryen `133` and
wasm-tools `1.263.0`. The historical Linux comparison uses the same product recipe
from the [November 5 budget decision](typst_theme_budget_2026-11-05.md), with its
original source and tool identities retained there.

| Metric | November 5 measurement | Dependency-refresh measurement | Previous ceiling | Selected ceiling |
| --- | ---: | ---: | ---: | ---: |
| Raw Cargo linker output | 20,979,113 | 25,519,288 | 21,609,000 | 26,285,000 |
| Final optimized and stripped WASM | 12,793,758 | 12,771,400 | 13,178,000 | 13,178,000 |
| Gzip of final WASM | 4,849,760 | 4,820,396 | 4,996,000 | 4,996,000 |
| Brotli of final WASM | 3,588,751 | 3,591,201 | 3,697,000 | 3,697,000 |

The raw ceiling uses the existing rule: measured bytes plus 3%, rounded upward to
1,000 bytes. The raw stage grows by 4,540,175 bytes against that historical observation;
the final WASM is 22,358 bytes smaller. These are release-range observations with both
source and dependency changes. They do not isolate a compiler-only cause, attribute
the growth to custom/name sections, or establish a timing or memory improvement.

## Artifact stages and unchanged checks

The [Typst artifact owner](../../crates/xtask/src/cmd/typst_artifact.rs) freezes the
Cargo output as `raw.wasm`, applies `wasm-opt -Oz` into `optimized.wasm`, and then
uses `wasm-tools strip --all` for the final `merman_typst_plugin.wasm`.
The [size owner](../../crates/xtask/src/cmd/wasm_size_matrix.rs) reads the manifest's
raw and final fingerprints and compresses the final WASM with the existing gzip/Brotli
settings. Raw bytes therefore describe an intermediate build artifact, rather than
the installed plugin.

The descriptor-owned recipe remains defaults off,
`all-diagrams,analysis,layout-cytoscape,layout-elk,svg`, target
`wasm32-unknown-unknown`, Cargo profile `wasm-size`. No capability, parser, theme,
security, resource, cancellation or export check is removed. Web budgets are untouched.

The original CI job passes the offline dependency closure after locked prefetching,
then fails only the old raw ceiling. That failure occurs before plugin validation
and package smoke; its size measurement alone does not qualify the artifact. A repaired
source requires its own successful owner build, all four budget checks, plugin/theme
validation and actual Typst smoke:

```text
cargo run --locked -p xtask -- build-typst-package --profile publish --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build --typst <typst-binary>
```

The full strict verifier and platform owners remain separate release evidence. This
decision changes a build-size ceiling; it does not establish release qualification
or registry publication.
