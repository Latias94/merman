# Semantic Parser Embedding Evidence (2026-10-22)

This receipt compares the frozen U1 full typed parser consumer with the current semantic parser
consumer. Both probes consume the same Flowchart, Sequence and State observations from the same
three fixture files. The API boundary is intentionally breaking, so the probe source is frozen at
the corresponding revision for each side rather than replayed against the other API.

## Recipe

```text
target: x86_64-pc-windows-msvc
toolchain: 1.95.0
profile: opt-level=s, lto=thin, codegen-units=1, panic=abort, strip=none, debug=false, incremental=false
features: diagram-flowchart,diagram-sequence,diagram-state
default-features: false
jobs: 1
```

The harness was `tools/bench/measure_diagram_selection.py --recipe semantic-parser`. It built an
isolated consumer with `--locked --offline`, ran the executable against the three corpus files,
copied a raw executable, stripped the delivered executable with the pinned toolchain's
`llvm-objcopy --strip-all`, and verified that stripping did not change consumed output.

## Results

| Consumer | Source revision | Probe revision | Raw bytes | Stripped bytes |
|---|---|---|---:|---:|
| U1 full typed parser | `8628c554ab0814469ada4769d99a6a4a7ccbc668` | same | 2,842,112 | 2,842,112 |
| Semantic parser | `3506f008ef63e959b1f633cb77e9a34285aee0d3` | same | 2,126,848 | 2,126,848 |

The semantic consumer is 715,264 stripped bytes smaller (25.17%) under this fixed Windows
toolchain and recipe. This is an isolated consumer observation, not a claim about workspace or
all-feature artifacts.

The three consumed output SHA-256 digests are identical on both sides:

| Corpus | Semantic output SHA-256 | Bytes |
|---|---|---:|
| `flowchart_medium.mmd` | `612a4aa93c7735a674dd49f8ca4a7d6556ef0c7325d65f68851279caeeff055d` | 2,292 |
| `sequence_medium.mmd` | `c3ded25de6f7ace71f84354fd69043b8db0a76814692a7151e161780c187fc17` | 1,051 |
| `state_medium.mmd` | `9c425f08877aa9424a62e1af3de6641e497d05524f67ca61880291504a12713f` | 624 |

The semantic probe also accepted the three enabled families and returned the expected
`merman.parse.unsupported_diagram` control result for an omitted Sequence family in its subset
check. Full machine receipts were retained in the ignored local artifact directories
`target/bench/semantic-boundary/size-base/` and `target/bench/semantic-boundary/size-head-final/`.

The baseline receipt retains the pre-boundary full parser probe; the head receipt uses the migrated
`SemanticParser` probe. This preserves the API-break migration evidence required by the refactor
plan while comparing the same runtime-fed meaning and corpus bytes.

## Verification Receipt

- Passed: semantic output equivalence, isolated size measurement, feature-matrix and artifact-profile
  validation, representative dependency-closure checks, parser differential, Rust/Node legal
  projection checks, native ABI and binding contracts, and the focused ASCII suite.
- Platform-deferred: `python scripts/verify-platform-bindings.py` reached the Flutter phase but the
  installed `F:\SDKs\flutter_old` reports Dart 3.9.2 while this repository requires Dart >=3.10.0.
  No source or SDK files were changed; rerun that phase with a compatible Flutter SDK.

## Artifact retention update

At 2026-10-22 21:51 UTC+08:00, following a local `cargo clean`, both ignored size evidence
directories were absent. The results above remain historical observations at the named revisions;
their machine receipts and executables are no longer available locally for rehashing or replay.
They do not certify final integration revision `6b2d15d19`. Fresh committed-source measurements
must produce new receipts and equivalent consumed output before final size admission.

## Final committed-source remeasurement (2026-10-23)

Fresh evidence compares the same U1 source `8628c554ab0814469ada4769d99a6a4a7ccbc668`
with final implementation `d0d26db24baa5823e235553c9a9251e3f70815a2`. It uses the recipe
above and the committed probe belonging to each API. Both raw executables were retained,
stripped with Rust 1.95.0's `llvm-objcopy`, and exercised before and after stripping.

| Consumer | Raw bytes | Stripped bytes |
| --- | ---: | ---: |
| U1 full typed parser | 2,843,648 | 2,843,648 |
| Final canonical semantic parser | 2,126,848 | 2,126,848 |

The final consumer is 716,800 bytes smaller (25.21%) on this Windows/MSVC recipe. The three
input hashes, consumed semantic output hashes and lengths match exactly, including the
historical output identities listed above. The final core feature closure is exactly
`diagram-flowchart,diagram-sequence,diagram-state`; it excludes presentation, editor facts,
compatibility JSON and terminal text. No renderer, theme-contract or math backend is resolved.

The workspace lockfiles differ, so this is not an identical-lockfile claim. Both builds keep
their committed locks and run `--locked --offline`. The isolated consumers' common resolved
external package versions and sources are identical, including build dependencies. The final
normal closure removes `euclid 0.22.14`, `num-traits 0.2.19`, `ryu-js 1.0.3`,
`unicode-segmentation 1.13.3`, and `unicode-width 0.2.2`; the full resolved graph also removes
their `autocfg 1.5.1` build dependency. No external package is added. Separate network-related
workspace lock changes are outside these isolated consumer closures.

A full SVG control uses the same committed probe on both sources, defaults disabled, and
the same positive `diagram-flowchart,diagram-gantt,diagram-sequence,diagram-state,svg,
layout-cytoscape,layout-elk,math` capability set. Its fixed size profile strips symbols during
the build. Delivered bytes decrease from 13,202,944 to 13,162,496, a reduction of 40,448 bytes
(0.31%). Flowchart small/medium and Gantt medium SVG outputs are byte-identical. Its resolved
external packages, features, toolchain and profile are equivalent; no capability removal is
counted as that control's reduction. This control is not a latency result or a family-complete
output comparison.

New raw evidence is retained outside Cargo targets at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-d0d26db24-20261023/`.
The corresponding receipt hashes are:

| Receipt | SHA-256 |
| --- | --- |
| `semantic-size-u1/receipt.json` | `1d89fd137c0e945d16718b59929998675457cc22c0b5b0503f70d50731bc6052` |
| `semantic-size-final/receipt.json` | `d737a64fcce73b316f6f92031d3c6efd8d1953438068c6c55440b6e54871b305` |
| `svg-size-u1/receipt.json` | `24b4f1c64c15efd6bc8f62adf0baa0080b84661ce07b46df26b6a49e72135fc4` |
| `svg-size-final/receipt.json` | `448e8b1c6d072db7026addcd030c2a2bbef57faba27da73ccab0259e35c5d0be` |

The comparison JSON binds corpus outputs and the inspected dependency boundaries. These are
new observations, not recovered samples from the deleted historical directories. They satisfy
the scoped final-byte and same-capability size controls; adjacent allocation/timing, installed
host contracts and whole-plan completion remain separate gates.

The subsequent `3c858bdfbc4372b0253144a98c4df47bd3ab72da` commit changes only three
Python native-build/symbol-inspection files. Rust sources, probes, inputs and capability
recipes are unchanged from the measured product revision. The external
`integration-python-only-bridge.json` records that exact changed-path inventory.
Fresh Node N-API and Node-WASM packages passed local packing, offline installation and
installed-package smoke. Typst 0.15.1 compiled 23 package examples with nine expected
failures. Windows GNU native packaging and standalone Dart 3.11.6 ABI 3, example and
theme-authoring checks also passed. This resolves the standalone Dart prerequisite;
it does not claim an actual Flutter desktop UI test or other-platform runtime coverage.
These results are retained in `node-installed-verification.json`,
`typst-retained-files.json` and `dart-host-verification.json` under the same external root.
All five final Web packages also passed fresh WASM/TypeScript builds, source/artifact
identity checks, 37-family package smoke and DOM safety. The Chromium desktop mounted-diagram
suite passed 10/10. `final-host-integration-receipt.json` freezes the seven resumed Dart/Web
steps and five Web input manifests. The earlier strict run remains exit 1 at its historical
stale-WASM failure; these current scoped passes do not rewrite that terminal result.

## Corrected final-source measurements (2026-10-23)

Independent completion review found missing common-DB accessibility normalization in lean
Flowchart/Swimlane and eager full-engine initialization in binding analysis/ASCII construction.
Commits `732438a8a` and `0ae3496d4` correct these behaviors; `1cf3bf137` explicitly selects
the Tree-sitter conformance oracle's compatibility capability. All 429 scoped Rust tests,
owning all-target/all-feature Clippy and formatting passed. The corrected product source is
`f0365e6a4948c4baff2f16adb735e0f143ee38c6`, including the associated documentation commits.

The final sides were rebuilt from that committed source. The frozen U1 binaries above were
reused without rebuilding: semantic probe observations and SVG probe source bytes are unchanged.
Toolchain, profiles, capability selection, corpus identities and consumed outputs match the
corresponding baseline receipts. Corrected final external dependency packages/features match
the previously inspected final closures; the SVG control also matches the U1 external closure.

| Isolated consumer | Frozen U1 bytes | Corrected final bytes | Reduction |
| --- | ---: | ---: | ---: |
| Three-family canonical semantics, stripped | 2,843,648 | 2,126,336 | 717,312 bytes (25.2251%) |
| Same-capability SVG, delivered | 13,202,944 | 13,158,400 | 44,544 bytes (0.3374%) |

These replace the earlier final-size values for current-source admission. The earlier source
measurements remain valid historical observations. The three consumed semantic outputs and
three SVG outputs remain byte-identical to the corresponding U1 outputs. The same scope limits
apply: these are isolated Windows/MSVC size controls, not latency or all-family output claims.

Corrected raw artifacts and receipts are retained at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/`.

| Corrected receipt | SHA-256 |
| --- | --- |
| `semantic-size-final/receipt.json` | `d75cc7a03c8f0d893a0783e9634c0b4c10f85757786cb6a8954c1d940f1a68cf` |
| `svg-size-final/receipt.json` | `052d4cebc9e3fae7f50cf9d62e1effc1ad5e58824a25f3c91ed01bad1590ae2e` |
| `semantic-size-comparison.json` | `a12cdae7a195c276b45e296306d92d448835538bcd2f530e44799a0ca8a0b1bc` |
| `svg-size-comparison.json` | `46894581622078a350c6357c1fcef1d2cf2d57ab4a27d249d148cabab2831451` |

The new installed-host/archive queue and registered public-operation timing remain separate
obligations. No whole-plan completion is inferred from these size passes or from the earlier
installed artifacts that predate the correctness fixes.

The corrected Node N-API Windows and Node-WASM packages subsequently passed assembly,
verification, tarball creation, offline installation and their public installed-package smoke.
Each installed backend exercised 30 SVG renders, 26 JSON operations, 44 support queries and
saved-recipe exchange through three fresh processes. Corrected Typst 0.15.1 package smoke
compiled 23 fixtures with nine expected failures; all 110 package/runtime files were retained.
Windows GNU native packaging and Dart 3.11.6 ABI 3, real capability/error/service smoke and
theme authoring also passed. The real native smoke includes required-math rejection. A frozen
copy of the tested DLL is retained outside Cargo targets. This remains standalone Dart/native
host evidence, not actual Flutter UI or other-platform runtime coverage.

| Corrected host receipt | SHA-256 |
| --- | --- |
| `node-installed-verification.json` | `6ab909e601daffb1de2d9fe6b620d86c62c15799d8de44075359ff593e173106` |
| `typst-retained-files.json` | `0d65e0d426d426699703585f06ba16a60b67fe5c12195cc3fdf9c66d1b9a7216` |
| `dart-host-verification.json` | `bce57bd77760d52e258c973b62f9fc8d2de4f716689f95fab96d3cf92437331c` |

Corrected Web full, analysis, render, editor and ASCII packages also passed their five-package
build and source-input verification, 37-family smoke matrix, required-math rejection and DOM
safety smoke. The Chromium mounted-diagram suite passed all ten tests. The 447 retained
package and source-input files are copied outside Cargo targets;
`web-host-verification.json` has SHA-256
`3ee7c89efddfbef27ef141cd56da5f77c0c96157c5f01f147e7c983cdfad924e`.
This supplies scoped recovery of the historical stale-WASM mount failure without rewriting
that strict command's exit status.

The corrected 27-step integration queue then completed successfully. cargo-dist 0.32.0 built
the local Windows/MSVC CLI archive, the existing release archive verifier executed its binary,
and the exact retained ZIP advertised `math` and rendered the required `$$x^2$$` formula through
RaTeX. The SVG contains 36,883 bytes and no literal formula delimiters. This is local archive
preflight, not a publication or a claim about another platform.

| Archive receipt or artifact | SHA-256 |
| --- | --- |
| `final-integration-receipt.json` | `25a9d0a04e9a399db5276c6c21cf1f4cd945e509529f3e73096afb08a61aca3f` |
| `merman-cli-x86_64-pc-windows-msvc.zip` | `dd1347e1641fc63e831736d43d463ca9d3a81a9f5c620e8e49a20e5ec714a9f7` |
| `cli-archive-math-receipt.json` | `227d36412a7ed7fc610084285e4b0bde1e8fe2c7171eb9fc984e3526a1db9648` |

Registered U3/U4/integrated timing controls are now running with unchanged thresholds,
protocol and sample caps. The U3 schedule has since completed with 23 confirmed non-regression
controls and one inconclusive State E2E row. U4 has also completed with 23 confirmed controls
and one inconclusive Class E2E row. Integrated warm parse adds six confirmed controls;
18 integrated rows remain without terminal classifications.
Both inconclusive rows failed head A/A calibration, so adjacent and whole-plan performance
acceptance remain unproven. See the
[adjacent-work report](semantic_boundary_adjacent_work_2026-10-22.md#renewed-u3-warm-parse-confirmation-2026-10-23)
for its protocol, row classifications and retained receipt. The completed artifact queue does
not establish latency, and no speedup or whole-plan admission is claimed.

## Integrated warm full-parse confirmation (2026-10-23)

The registered integrated adapters are base `75def7949daaa2de9f51f727a77e8c9ab74a49fc` and
head `bf6d101cd4ab5d2214682a4139ee6f922d786b11`. The head differs from product `f0365e6a4` only
in the four preregistered benchmark/corpus adapter paths. Both checkouts remain clean; common
pipeline, JSON identity helper, corpus and preflight contract bytes match. Discovery retains
six comparable fixtures with matched consumed full typed output. Frozen executable SHA-256
values are base `f4979b4606cac794543ab06b7a3de95ab294331965763b4bec2264b31e3246c2` and
head `60168cfbadf1ab1bd312d2ef81ba298928e25fa3fdfad0964e63a51072cd803e`.

Warm full parse completed with six `confirmed_non_regression` rows, zero contract failures and
zero inconclusive rows. Both runners passed eight A/A pairs and each fixture completed eight
fresh balanced AB/BA pairs. The original generic_v2/long protocol, sample size 30, 2s warmup,
3s measurement, 10% AND 50,000 ns, 95% confidence, seed 0, 10,000 resamples, divisor one and
maximum 32 pairs remain unchanged. Pair completeness/order, output/preflight/postflight identity,
clean sources, fixtures, discovery and frozen executables were checked after sampling.

| Warm full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 152.998 us | 132.854 us | Confirmed non-regression | 8 |
| State medium | 165.751 us | 149.048 us | Confirmed non-regression | 8 |
| Class medium | 220.366 us | 198.031 us | Confirmed non-regression | 8 |
| ER medium | 134.930 us | 123.191 us | Confirmed non-regression | 8 |
| Pie medium | 6.136 us | 4.749 us | Confirmed non-regression | 8 |
| Flowchart medium | 358.655 us | 342.476 us | Confirmed non-regression | 8 |

All mirrored improvement results are `confirmed_non_improvement`; lower means do not establish
an admitted speedup. This is full-operation timing on the registered source adapters, not
semantic-only timing or a claim about the pending private cfg patch. Across completed controls,
52 confirm non-regression, two adjacent E2E rows remain inconclusive and 18 integrated rows have
no terminal classification. Whole-plan performance remains unproven. Concurrent unrelated host
activity remains possible; no uncontended-host claim is made.

The final root retains `integrated-discovery-audit.json`, SHA-256
`ceffb007733154f6b5762c147624cf0d905db3b82a06b6620cc7e3772bcab577`, and
`integrated-parse-confirmation-audit.json`, SHA-256
`1f171e2ac772bc3761aab6eed0068b838a3bbe2b8e70cb0570a4fcfab3347a29`. Raw reports/logs,
registration and copied executables remain under `timing-integrated/`.
