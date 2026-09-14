# Installed Web and Typst theme consumers — 2026-09-14

Source: `0e0caebdbb3330b7b44f04d2271d3d45a52b92d8`.

## Result

The canonical Web workspace built all five package profiles from one source. The existing Web
smoke passed package contracts, runtime capability projections, WASM input freshness, the full
five-package matrix, and DOM safety. Every package was packed and installed into a separate
consumer with lifecycle scripts disabled and no registry access.

A Chromium 151.0.7922.34 consumer loaded all five WASM files from their installed package paths
with HTTP 200 responses. Analysis, ASCII, and editor correctly exposed no structured theme
catalog. Render and full each passed the 22 revision-90 support vectors, three shared error
vectors, light/dark materialization and cold-spec rendering across Flowchart, State, and Sequence,
three preset exports, and 13 SVG renders. Each render/full package also passed three valid-budget
and three insufficient-budget cases, six non-finite-number rejections, and preservation of an
explicit node-fill null clear. This is an installed browser consumer check, not a
browser qualification of arbitrary diagrams.

The Typst publish profile was built with the existing xtask pipeline and local Typst 0.15.1; the CI owner workflow pins Typst 0.15.0, so this is host-tool evidence rather than CI-tool identity.
Its WASM authoring vectors passed: 22 support queries, two materializations, and three diagnostic
errors. The package smoke compiled 22 positive fixtures and rejected nine expected invalid
fixtures. The Typst package and Web package group were verified through their existing owner
contracts; no new packaging framework was added.

## Size observations

The existing `wasm-size-matrix` measured all four metrics and returned `result=ok` for all six
profiles. Values are final WASM artifacts, not peak memory, startup time, or complete installed
application footprints. Hashes identify this build only.

| Profile | Raw | Stripped/post-link | Gzip | Brotli | Packaged artifact SHA-256 |
| --- | ---: | ---: | ---: | ---: | --- |
| web-analysis | 3,675,043 | 3,674,778 | 1,412,251 | 1,073,788 | `8df9ffe336959666d68a955f7462a4437653616f5c37edd8e82da5d1960b0c66` |
| web-ascii | 5,198,169 | 5,197,904 | 1,914,963 | 1,445,313 | `28563124c29b09456292b535d7b7585c8d3e3bcef880b1dcf96a4f2bee5d366c` |
| web-editor | 3,786,176 | 3,785,911 | 1,456,761 | 1,108,837 | `f6ea46cbc862235fbbed1c39b4792e03af5b8424e06569f2e5db223a5f9c01e0` |
| web-full | 15,934,094 | 15,933,829 | 5,940,878 | 4,371,709 | `4c9edc6971ad03008c9e2b17e8736f54ee903d21220a27d545d687589f205854` |
| web-render | 14,048,135 | 14,047,870 | 5,296,007 | 3,904,012 | `2e7a8cab6d1ae319ea93757af498d38e57660c9a96ef3ab547d9eadc58a1794c` |
| typst-wasm | 18,771,827 | 11,610,736 | 4,448,777 | 3,285,939 | `66ba64a553c7a274057e94f40f0baf2c42db58e6584b67aac9eea46c833369fc` |

The Web npm archives were also created for the five installed consumers; their sizes and hashes
are retained in the experiment ledger. The accepted limits in
`docs/release/WASM_SIZE_BUDGETS.json` remain unchanged. Current values are within the established
approximately three-percent regression margin, so there is no evidence-based reason to widen the
budget. The complete renderer and Typst profiles are larger because their declared capabilities
include SVG, layout backends, and the corresponding renderer closure; the slim profiles do not
carry that closure. This is capability-bound evidence rather than per-symbol byte attribution.

## Commands and evidence

```text
CARGO_BUILD_JOBS=2 npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
npm pack <each assembled Web package> --json --pack-destination <tarballs>
npm install --offline --ignore-scripts --no-audit --no-fund <five local tarballs>
node <browser-smoke>  # Chromium 151, installed package entries and package WASM paths
cargo run --locked --release -p xtask -- typst-package-smoke --profile publish --out <typst-package> --keep-artifacts --typst /opt/homebrew/bin/typst
<xtask> wasm-size-matrix --surface web --web-package-root platforms/web/packages --budget-file docs/release/WASM_SIZE_BUDGETS.json
<xtask> wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
python3 scripts/web_package_group.py create-manifest ...
python3 scripts/web_package_group.py verify-artifact ...
```

The Web smoke, package installation, Chromium browser check, Typst package smoke, 24 metric budget checks, and package-group verification all passed. The main checkout contained only the two
pre-existing untracked knowledge directories, which were left untouched. Build and smoke logs,
commands, package archives, installed consumer, browser results, Typst package, metrics and source
receipt are under `target/bench/experiments/theme-web-typst-revision90/`.

## Delivery boundary

This closes the current revision-90 Web and Typst artifact-consumer verification slice. It confirms
that the existing adjusted budgets are reasonable for these declared profiles and that the public
package resource paths work after installation. It does not prove minimum possible size, startup
or peak-memory acceptability, Linux/Windows parity, external-host assurance, public preset-cell
promotion, or C7a contract freeze. The Web/Typst consumer evidence is now stronger, but the final
candidate still requires the remaining cross-host/profile matrix and formal public discovery and
rollout decision.
