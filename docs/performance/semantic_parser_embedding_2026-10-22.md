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

Registered U3/U4/integrated timing controls have completed with unchanged thresholds,
protocol and sample caps. The U3 schedule has since completed with 23 confirmed non-regression
controls and one inconclusive State E2E row. U4 has also completed with 23 confirmed controls
and one inconclusive Class E2E row. Integrated warm parse adds six confirmed controls;
cold-engine parse adds five confirmed controls and one inconclusive Flowchart row. Known-type
JSON adds six confirmed controls; E2E adds two confirmed controls and four inconclusive rows.
The adjacent inconclusive rows failed head A/A calibration; integrated cold Flowchart failed
base A/A and exceeded the pair cap. Adjacent and whole-plan performance acceptance remain
unproven. See the
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
semantic-only timing or a claim about the pending private cfg patch. At this warm-parse
checkpoint, 52 controls confirmed non-regression, two adjacent E2E rows remained inconclusive
and 18 integrated rows had no terminal classification. Whole-plan performance remains unproven.
Concurrent unrelated host activity remains possible; no uncontended-host claim is made.

The final root retains `integrated-discovery-audit.json`, SHA-256
`ceffb007733154f6b5762c147624cf0d905db3b82a06b6620cc7e3772bcab577`, and
`integrated-parse-confirmation-audit.json`, SHA-256
`1f171e2ac772bc3761aab6eed0068b838a3bbe2b8e70cb0570a4fcfab3347a29`. Raw reports/logs,
registration and copied executables remain under `timing-integrated/`.

## Integrated cold-engine control remains inconclusive (2026-10-23)

The registered integrated cold-engine group completed with five `confirmed_non_regression`
rows and one inconclusive Flowchart row, exiting 3 with zero contract failures. This operation
constructs a fresh full engine per iteration inside a reused process; it is not a cold process.
Both runners completed all eight A/A pairs for every fixture. Flowchart head was stable, but
base failed the relative and absolute identity and order margins. Its absolute identity interval
was -164.249 to +13.188 us and its order interval was -9.034 to +168.075 us, outside the
registered +/-50 us margin. The dispersion-derived requirement was 36 pairs, above the unchanged
cap of 32.

Flowchart retains matched consumed typed output but has no fresh confirmation pairs, means or
effect bounds. The unchanged suite rule selected 32 fresh balanced AB/BA pairs for each of the
five stable fixtures. Their own calibration required eight pairs; the suite count derives once
from the maximum across all six selected rows, capped at 32. This was not a budget extension or
a retry after viewing confirmation outcomes.

| Cold-engine full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 161.623 us | 143.514 us | Confirmed non-regression | 32 |
| State medium | 171.509 us | 159.535 us | Confirmed non-regression | 32 |
| Class medium | 231.233 us | 208.601 us | Confirmed non-regression | 32 |
| ER medium | 147.032 us | 136.109 us | Confirmed non-regression | 32 |
| Pie medium | 11.648 us | 9.599 us | Confirmed non-regression | 32 |
| Flowchart medium | Not admitted | Not admitted | Inconclusive: base A/A unstable and required pairs exceed cap | 0 |

The parent checked all A/A observations, fixed fresh pair counts/order, divisor one, output and
preflight/postflight identities, clean sources, input files, fixtures, discovery and both frozen
executables. The original generic_v2/long protocol, 30 samples, 2s warmup, 3s measurement,
10% AND 50,000 ns, 95% confidence, seed 0 and 10,000 bootstrap resamples remain unchanged.
All five mirrored improvement outcomes are `confirmed_non_improvement`; no speedup is admitted.

At this cold-engine checkpoint, completed controls totaled 57 confirmed non-regression and
three inconclusive rows: U3 State E2E, U4 Class E2E and integrated cold Flowchart. Twelve integrated
JSON/E2E rows remained without terminal classification. These three unmet controls prevent
whole-plan performance acceptance; no additional samples or threshold changes follow.
The fixed queue continued to known-type JSON.
Concurrent unrelated host work remains possible; no cause for calibration noise is established.

The final root retains `integrated-cold-parse-confirmation-audit.json`, SHA-256
`b94bcb6682839a609f6ec1938ca846d5af4c4f57f36e8bd4ad2d41792ab4f61a`, and the raw
`timing-integrated/parse_cold_engine.{json,md,log}` files, alongside discovery, registration and
copied executables. This timing remains scoped to the registered adapter pair and measured
product `f0365e6a4`, without admission of the pending private cfg patch.

## Integrated known-type JSON confirmation (2026-10-23)

The registered integrated `compatibility_json_parse` group completed with six
`confirmed_non_regression` rows, zero contract failures and zero inconclusive non-regression
rows, exiting 0. This known-type operation excludes detection. Both runners passed all eight
A/A pairs; each fixture completed eight fresh balanced AB/BA pairs. The parent checked every
pair/order, divisor one, matched output and preflight/postflight identities, clean sources,
input files, fixtures, discovery and frozen executable retention. Protocol, thresholds and
sample cap remain unchanged from the warm/cold groups.

| Known-type JSON fixture | Base mean | Head mean | Non-regression | Mirrored improvement | Fresh pairs |
| --- | ---: | ---: | --- | --- | ---: |
| Sequence medium | 175.579 us | 158.414 us | Confirmed | Confirmed non-improvement | 8 |
| State medium | 375.985 us | 330.462 us | Confirmed | Inconclusive | 8 |
| Class medium | 292.591 us | 265.391 us | Confirmed | Confirmed non-improvement | 8 |
| ER medium | 153.776 us | 146.787 us | Confirmed | Confirmed non-improvement | 8 |
| Pie medium | 7.941 us | 6.479 us | Confirmed | Confirmed non-improvement | 8 |
| Flowchart medium | 533.551 us | 504.475 us | Confirmed | Inconclusive | 8 |

State and Flowchart have inconclusive mirrored improvement decisions: their bounds establish
non-regression but neither confirm nor exclude improvement under both registered materiality
thresholds. The other four mirrored outcomes are `confirmed_non_improvement`. No speedup is
admitted. These mirrored decisions are distinct from the three failed A/A non-regression
controls retained from the earlier groups.

At this JSON checkpoint, completed controls totaled 63 confirmed non-regression, three
inconclusive non-regression rows and six integrated E2E rows without terminal classification. U3 State E2E, U4 Class E2E
and integrated cold Flowchart remain unmet; whole-plan performance acceptance is unproven.
The fixed queue continued to E2E without retries, added samples or threshold changes.
Concurrent unrelated host work remains possible.

The final root retains `integrated-json-parse-confirmation-audit.json`, SHA-256
`58bc157c4bb27bc2ccf91b89d1b50d2701fb3cb99c7f70df057581210d4821c3`, and the raw
`timing-integrated/compatibility_json_parse.{json,md,log}` files. Timing remains scoped to the
registered integrated adapter pair and measured product `f0365e6a4`; the pending private cfg
cleanup has not received runtime or artifact acceptance.

## Integrated E2E and final timing classification (2026-10-23)

The final registered E2E group completed with two `confirmed_non_regression` rows and four
inconclusive rows, exiting 3 with zero contract failures. Every fixture completed eight A/A
pairs on each executable and retained matched exact SVG identity. Sequence and ER were stable;
each completed 32 fresh balanced AB/BA pairs under the unchanged suite rule. State, Class, Pie
and Flowchart have no fresh confirmation pairs, admitted means or effect bounds.

| E2E fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 484.932 us | 455.538 us | Confirmed non-regression | 32 |
| State medium | Not admitted | Not admitted | Inconclusive: base A/A absolute order margin | 0 |
| Class medium | Not admitted | Not admitted | Inconclusive: base A/A absolute identity/order margins | 0 |
| ER medium | 692.742 us | 676.023 us | Confirmed non-regression | 32 |
| Pie medium | Not admitted | Not admitted | Inconclusive: both A/A relative identity/order margins | 0 |
| Flowchart medium | Not admitted | Not admitted | Inconclusive: both A/A unstable; base pair requirement exceeds cap | 0 |

State base's absolute order interval reaches +55.192 us, outside the registered +/-50 us margin.
Class base's absolute identity interval is -53.077 to +62.236 us and order interval is -72.533
to +40.496 us. Pie fails relative identity/order on both runners despite absolute intervals
within the margin. Flowchart base fails relative and absolute identity/order margins; head fails
absolute identity/order. Flowchart base requires 82 pairs and head 14, while Class base requires
ten and all remaining calibration requirements are eight. The unchanged maximum-across-rows
rule caps the suite at 32; it does not qualify the four unstable rows or extend their budgets.

The parent checked every A/A observation, 32 fixed fresh pairs on each stable row, alternating
order, divisor one, exact SVG/preflight/postflight identity, clean sources, input files, fixtures,
discovery and frozen executables. Original generic_v2/long, 30 samples, 2s warmup, 3s measurement,
10% AND 50,000 ns, 95% confidence, seed 0, 10,000 resamples and the 32-pair cap remain unchanged.
The two mirrored improvement outcomes are `confirmed_non_improvement`. Concurrent unrelated
host activity remains possible; these observations do not establish its effect on calibration.

| Registered comparison | Confirmed non-regression | Inconclusive | Contract failures | Confirmed regressions |
| --- | ---: | ---: | ---: | ---: |
| U3 adjacent | 23 | 1 | 0 | 0 |
| U4 adjacent | 23 | 1 | 0 | 0 |
| Integrated | 19 | 5 | 0 | 0 |
| Total | 65 | 7 | 0 | 0 |

All 72 rows are terminal. The seven unmet controls are U3 State E2E, U4 Class E2E, integrated
cold Flowchart and integrated E2E State/Class/Pie/Flowchart. The two separately inconclusive
JSON mirrored improvement decisions remain unchanged. No speedup, retry, additional samples,
threshold change or whole-plan performance acceptance follows. The scheduler's successful
wrapper exit means it retained every group, not that every group passed.

The final root retains `integrated-e2e-confirmation-audit.json`, SHA-256
`3f0714f9d081c6b21ab788277b92151060f5a0510919ede7e8e1feddbeb41711`, and the raw
`timing-integrated/end_to_end.{json,md,log}` files. Exact production-feature lint and the pending
cfg cleanup's owner tests follow serially; final source/artifact acceptance remains separate.

## Committed private-definition cleanup and final lean control (2026-10-23)

Commit `599f714d344e94fd15c7f3a3abeb8be8ec51eeee` gates seven private helper/field definitions by
their actual presentation, Usecase, ZenUML or Agentflow consumers. Treemap's intermediate editor
field is retained only for combined compatibility/editor requests; canonical snapshot capture
takes its requested facts before conversion. A public snapshot regression verifies complete and
recovered facts with CRLF/frontmatter offsets and diagnostics-only recovery. Production runtime
bodies, public API/ABI, feature manifests and capability descriptors are unchanged.

The first exact lint queue found Treemap's now-unused `Result` import on three lean selections.
The first owner commands also passed both nextest `-j 2` and `--test-threads 2`; nextest treats
`-j` as test threads, so these commands exited before running tests. Both original failure sets
remain retained. The import now follows presentation and corrected commands use
`--build-jobs 2 --test-threads 2`. All seven exact production selections pass `-D warnings`,
including the three-family core, language stdio binary, facade, native binding and official CLI
recipes. Format and 79 semantic, 91 editor and 258 full owner tests pass: 428 tests total.

The existing artifact driver then rebuilt only the candidate from the exact committed cleanup,
using the frozen f036 semantic consumer and corpus, Rust 1.95.0, original Windows/MSVC profile,
lock policy and three family selectors. Frozen U1 and prior f036 artifacts were rechecked and
reused without rebuilding. As already documented, U1's full typed consumer and the migrated
semantic consumer have distinct frozen source bytes for their respective APIs; their exercised
meaning is identical. The baseline catalog's repeated variants reduce to the same three logical
families. No identical baseline probe-byte claim is made.

| Final control | Raw bytes | Stripped bytes | Change from f036 |
| --- | ---: | ---: | ---: |
| Committed 599f three-family canonical consumer | 2,126,336 | 2,126,336 | 0 bytes |

All three consumed outputs match both U1 and f036 byte-for-byte. The normal package/feature tree
and complete resolved metadata package, feature and dependency-edge sets match f036. Core has
exactly `diagram-flowchart,diagram-sequence,diagram-state`, excluding presentation, editor facts,
compatibility JSON, terminal text and rendering. The candidate remains 717,312 bytes smaller
than frozen U1, a 25.2251% reduction. A distinct executable hash is retained; equal size/output
does not imply binary byte identity or another size improvement from cfg cleanup.

Full installed products, SVG control, oracle, allocation and timing receipts retain their
original source identities. Their full recipes keep the changed definition predicates; the
bounded source comparison and passing owner checks establish scoped applicability without
claiming new full-product executions. The exact three-family candidate received its own refresh.
`final-cfg-source-applicability.json` records this distinction.

| Final cleanup receipt | SHA-256 |
| --- | --- |
| `cfg-cleanup-quality-verification.json` | `ae488c21ebabe5fbc72afd70ae982c6ab2cef6e65096228f8254e45dd5ea9e06` |
| `semantic-size-cfg-final/receipt.json` | `1b97264c91174ac520ab39529c1f578b4091d29fab5811acf652d61fa77b955a` |
| `semantic-size-cfg-final-comparison.json` | `77c0d0151c2b2cddcdc00801416fc20017be7a50567427dd97caf23b00749552` |
| `final-cfg-source-applicability.json` | `85f966133cfb2782509eadc044f3886e4069cf90d440fcf1de61f96c62bfd97f` |

This closes the final source quality and lean artifact controls. The seven inconclusive timing
rows still prevent complete performance acceptance; no retry, budget extension, speedup claim
or overall completion follows. Platform deferrals and the historical strict failure retain their
previous scopes. No push, PR, merge, publication or release occurred.

## Authorized independent follow-up (2026-10-23)

The maintainer subsequently authorized an independent experiment to begin immediately. Before
fresh sampling, the four affected original operation cohorts were registered with all six
fixtures each: U3 E2E, U4 E2E, integrated cold-engine parse and integrated E2E (24 required rows).
The experiment reuses the clean original source pairs, frozen binaries, corpus and consumed
output identities. No product rebuild, new harness or production change is involved.

The generic_v2/long protocol remains 30 Criterion samples, 2s warmup, 3s measurement, eight
balanced A/A pairs per executable, a maximum of 32 fresh balanced AB/BA pairs, 10% AND 50,000 ns,
95% per-cohort confidence, seed zero, 10,000 bootstrap resamples and divisor one. Each cohort
runs once. There is no optional stopping, retry, budget extension, row removal or threshold
change. All 24 rows must confirm non-regression for the new experiment to qualify; any unstable
or regressing row leaves full acceptance unproven. The four cohorts do not establish a joint
95% confidence claim, and samples are not pooled between groups or experiments.

Read-only preflight host observations reported total CPU at 0%, 0% and 2% over three short
intervals. Background processes remained; these observations do not prove an uncontended host,
hardware isolation or the cause of historical calibration instability. No processes, power
settings or CPU affinity were changed. Source, binary, discovery and input identities were
verified before sampling.

Evidence is retained separately at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/followup-independent-20261023T042304Z/`.
`registration.json` has SHA-256
`7283b84c55fba50184922ea362dd52d26a65d3075ff85acba565cb1cf85845ed`;
`preregistration-seal.json` also freezes the one-shot scheduling wrapper and existing comparator.
The original 65 confirmed and seven inconclusive outcomes remain unchanged. The new schedule
is running; no result, speedup or completion is claimed from its registration or launch.

### Independent U3 E2E result

The first independent cohort completed with five `confirmed_non_regression` rows and one
inconclusive State row, exit 3 and zero contract failures or confirmed regressions. Both runners
completed eight A/A pairs for every fixture. State base was stable; head's absolute identity
interval was -65.959 to +44.020 us and order interval -56.164 to +57.764 us, outside the unchanged
+/-50 us equivalence margin. It has matched exact SVG identity but no fresh confirmation pairs,
admitted means or effect bounds.

The dispersion-derived suite count was ten, within the unchanged cap of 32. The five stable
fixtures each completed ten fresh balanced AB/BA pairs; their own requirements were eight.
The parent checked every calibration observation and fresh pair, order, divisor one, consumed
SVG/preflight/postflight identity, clean source/input files, discovery and frozen binaries.

| U3 E2E fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 472.715 us | 467.783 us | Confirmed non-regression | 10 |
| State medium | Not admitted | Not admitted | Inconclusive: head A/A absolute identity/order | 0 |
| Class medium | 1,070.870 us | 1,077.850 us | Confirmed non-regression | 10 |
| ER medium | 677.682 us | 669.458 us | Confirmed non-regression | 10 |
| Pie medium | 37.444 us | 35.675 us | Confirmed non-regression | 10 |
| Flowchart medium | 1,829.010 us | 1,819.350 us | Confirmed non-regression | 10 |

All five mirrored improvement decisions are `confirmed_non_improvement`. State remains unmet;
this independent experiment cannot meet its all-24-controls criterion. The fixed queue continued
to U4 E2E to preserve all registered observations, without retrying State or extending a budget.
Historical outcomes remain unchanged. Host CPU snapshots during sampling reached 15-18%, so
the initial low-load observation does not establish a quiet measurement window or explain the
calibration result. No background tasks were stopped and no speedup is admitted.

The independent root retains `u3-end_to_end-audit.json`, SHA-256
`a2366c9e22d2c40faddd550f16249c9c6588867cf981055dc27d537f98b71c1a`, and the raw
`u3-end_to_end.{json,md,log}` files. Eighteen independent rows remain without terminal results.

### Independent U4 E2E result

The second independent cohort completed with five `confirmed_non_regression` rows and one
inconclusive State row, exit 3 with no contract failures or confirmed regressions. Class,
previously inconclusive in the original U4 run, passes this independent observation. State
fails A/A on both runners: base fails relative and absolute identity/order; head fails the
absolute identity/order margins. Base absolute identity is -91.008 to +18.715 us and order is
-28.276 to +86.423 us. Head identity is +1.358 to +57.295 us and order -18.979 to +53.699 us.
State retains exact SVG identity but no fresh confirmation pairs, admitted means or bounds.

Both runners completed eight A/A pairs for all six fixtures. The unchanged suite rule derives
ten fresh balanced pairs, within the cap of 32, for each of the five stable fixtures. The parent
checked every observation, pair/order, divisor one, output and preflight/postflight identity,
clean source/input files, fixtures, discovery and frozen binaries.

| U4 E2E fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 471.771 us | 473.280 us | Confirmed non-regression | 10 |
| State medium | Not admitted | Not admitted | Inconclusive: both runners' A/A | 0 |
| Class medium | 1,074.180 us | 1,082.210 us | Confirmed non-regression | 10 |
| ER medium | 675.872 us | 674.099 us | Confirmed non-regression | 10 |
| Pie medium | 36.591 us | 36.479 us | Confirmed non-regression | 10 |
| Flowchart medium | 1,841.800 us | 1,835.900 us | Confirmed non-regression | 10 |

All five mirrored improvement outcomes are `confirmed_non_improvement`. Independent totals
are ten confirmed non-regression, two inconclusive and twelve integrated rows still without
terminal results. The original 72-row classifications remain unchanged; the new Class pass
does not erase the original Class result or compensate for State's failed calibration. The
fixed queue continued to integrated cold-engine parse. No extra samples, retries, budget or
threshold changes, or speedup claims follow.

The independent root retains `u4-end_to_end-audit.json`, SHA-256
`839c22a1d93bec21065a7059754dce21cf4d0938b7f70b39ad6fd6487760705d`, and raw
`u4-end_to_end.{json,md,log}` files.

### Independent integrated cold-engine result

The third independent cohort completed with six `confirmed_non_regression` rows, zero
inconclusive rows, zero contract failures and zero confirmed regressions, exit 0. Both runners
passed all eight A/A pairs; every fixture completed eight fresh balanced AB/BA pairs. Flowchart,
inconclusive in the original integrated cold-engine run, passes this independent observation.
This lane constructs a fresh full engine inside a reused process; it is not a cold process.

| Cold-engine full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 161.649 us | 143.511 us | Confirmed non-regression | 8 |
| State medium | 170.882 us | 153.056 us | Confirmed non-regression | 8 |
| Class medium | 230.836 us | 210.007 us | Confirmed non-regression | 8 |
| ER medium | 147.905 us | 130.643 us | Confirmed non-regression | 8 |
| Pie medium | 11.557 us | 9.496 us | Confirmed non-regression | 8 |
| Flowchart medium | 368.389 us | 347.736 us | Confirmed non-regression | 8 |

The parent checked every A/A and fresh observation, balanced order, divisor one, exact consumed
output and preflight/postflight identities, clean sources, input files, fixtures, discovery,
frozen executables and the unchanged registration/harness. All six mirrored improvement
outcomes are `confirmed_non_improvement`; no speedup is admitted. The independent totals are
16 confirmed non-regression, two inconclusive and six E2E rows still without terminal results.
The two State E2E controls remain unmet, so this experiment cannot satisfy its all-24-controls
criterion. Neither this pass nor any mean replaces an original observation.

The independent root retains `integrated-parse_cold_engine-audit.json`, SHA-256
`7bdc86f525d2e8fb025163801a23272e6cc70e0fad0dd7b22a569c99ab51c05d`, and raw
`integrated-parse_cold_engine.{json,md,log}` files. The fixed queue continued to integrated E2E,
without retries, extra samples or changed thresholds/caps.

### Independent integrated E2E result

The final independent cohort completed with five `confirmed_non_regression` rows and one
inconclusive Class row, exit 3 with zero contract failures or confirmed regressions. Both
runners completed eight balanced A/A pairs for all six fixtures. Class head A/A passes its
relative margins and pair-count budget, but its absolute identity interval is -31.213 to
+58.613 us and its absolute order interval is -67.818 to +7.342 us, outside the unchanged
50 us margins. Base Class A/A passes. Class retains exact 77,419-byte SVG identity, with
zero fresh confirmation pairs and no admitted means or bounds.

The unchanged calibration rule derives eight fresh balanced AB/BA pairs for each of the five
stable rows. State, Pie and Flowchart pass this independent E2E observation after remaining
inconclusive in the original integrated cohort; the historical classifications remain intact.

| Integrated E2E fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 478.669 us | 440.776 us | Confirmed non-regression | 8 |
| State medium | 760.665 us | 706.015 us | Confirmed non-regression | 8 |
| Class medium | Not admitted | Not admitted | Inconclusive: head A/A | 0 |
| ER medium | 672.996 us | 650.950 us | Confirmed non-regression | 8 |
| Pie medium | 37.408 us | 34.748 us | Confirmed non-regression | 8 |
| Flowchart medium | 1,824.175 us | 1,809.138 us | Confirmed non-regression | 8 |

The parent checked all 192 A/A observations and 80 fresh confirmation observations for
balanced order, divisor one, consumed SVG identity and preflight/postflight verification.
Using the unchanged existing harness, it deterministically recomputed all twelve A/A
summaries and the five confirmation intervals and both classifications. Clean frozen
sources, input files, fixtures, discovery, executable copies, sealed registration and exact
scheduled commands also match. These checks inspect retained samples; they collect no new
observations. All five mirrored improvement decisions are `confirmed_non_improvement`.

The independent root retains `integrated-end_to_end-audit.json`, SHA-256
`3facd78941330bd4534fcefdd99af1ccfaf50ea1948656dc87bfec20f62f3a51`, and the raw
`integrated-end_to_end.{json,md,log}` files.

### Independent experiment terminal classification

All four preregistered cohorts completed at 2026-10-23T06:20:43Z, with every required row
retained. The final independent classification is 21 confirmed non-regression and three
inconclusive, with zero contract failures and zero confirmed regressions.

| Independent cohort | Confirmed non-regression | Inconclusive | Unmet control |
| --- | ---: | ---: | --- |
| U3 E2E | 5 | 1 | State head A/A |
| U4 E2E | 5 | 1 | State base and head A/A |
| Integrated cold-engine parse | 6 | 0 | None |
| Integrated E2E | 5 | 1 | Class head A/A |
| Total | 21 | 3 | All-24-controls admission criterion unmet |

The registration requires all 24 controls to confirm. The three inconclusive rows therefore
leave this experiment unqualified for complete performance acceptance. Its cohorts keep
their separate confidence scopes; the original 65 confirmed and seven inconclusive rows
remain a separate historical result. Neither choosing favorable old/new rows nor pooling
samples establishes acceptance. All 21 mirrored improvement classifications are
`confirmed_non_improvement`; no speedup is admitted.

The independent root's `terminal-admission-audit.json`, SHA-256
`f957d2884a74fc404d75b6d656a766f37137a2745fdc8ca3e0e4c8649e831005`,
retains all four report/audit/log hashes and verifies that later commits change documentation
only relative to the tested cfg-cleanup source. Source, allocation, artifact and
installed-product evidence keep their previously recorded applicability.
No measurement remains running or pending. The one-run registration permits no automatic
retry or extra samples; further measurement needs a separately agreed experiment or an
explicit acceptance-contract change.

## Authorized quiet-window follow-up (2026-10-23)

After the preceding experiment ended, the maintainer explicitly authorized starting a new
measurement window and reported no interference. The same Windows 11 host is retained:
Intel Core i9-13900KF, 24 cores / 32 logical processors, Rust 1.95.0 MSVC. Three read-only
pre-sampling CPU snapshots were 4%, 4% and 2%. This records the maintainer's arranged window
and bounded observations, not a claim of physical host isolation. No applications, priority,
affinity or power settings were changed.

The new preregistration is retained at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/controlled-host-20261023T083654Z/`.
Its `registration.json` SHA-256 is
`77e6f3fc243ba33a8632d27648be2cca7fbf09febcfdcb148ad7b7d0b0895687`.
The original clean U3, U4 and integrated source pairs, frozen executable copies, source/input
files, fixtures, discovery receipts and unchanged comparison-harness digest were checked
before sampling. The existing harness repeats live output preflight before fresh A/A.

This is one new fixed schedule for each of the four complete six-fixture cohorts: U3 E2E,
U4 E2E, integrated cold-engine parse and integrated E2E. It retains `generic_v2`/`long`,
30 Criterion samples, 2s warmup, 3s measurement, eight A/A pairs per runner, max 32 fresh pairs,
divisor one, 10% AND 50,000 ns, 95% per-cohort confidence, seed zero and 10,000 bootstrap
resamples. All 24 controls must confirm with exact consumed output identity. No source/build,
threshold, budget, confidence-scope or acceptance change is introduced.

The original 72-row result (65 confirmed / seven inconclusive) and the first independent
24-row result (21 confirmed / three inconclusive) remain separate historical observations.
No samples are pooled or classifications replaced. The newly authorized queue started with
U3 E2E and executed serially once; no retries, optional stopping, extra samples or concurrent
Cargo work were authorized. Its terminal results and independent retained-sample audit are
recorded below.

### Quiet-window cohort outcomes

All four preregistered reports reached terminal status by 2026-10-23T10:22:55Z. The fixed
schedule produced 16 confirmed non-regression rows and eight inconclusive rows, with zero
contract failures and zero confirmed regressions. The all-24-controls admission rule is not
met. Each cohort retains its own 95% confidence scope; these counts are not pooled with one
another or with either historical experiment.

| Cohort | Confirmed non-regression | Inconclusive | Fresh pairs per admitted row | Result |
| --- | ---: | ---: | ---: | --- |
| U3 end-to-end | 0 | 6 | 0 | Every fixture is blocked by unstable base A/A; State and Flowchart also exceed the fixed 32-pair cap. |
| U4 end-to-end | 4 | 2 | 8 | State head A/A identity interval is unstable; Class base/head identity and order intervals are unstable. |
| Integrated cold-engine parse | 6 | 0 | 8 | All fixtures confirm non-regression. |
| Integrated end-to-end | 6 | 0 | 8 | All fixtures confirm non-regression. |
| Total | 16 | 8 | Separate cohort schedules | All-24-controls admission criterion unmet. |

The integrated E2E State row's non-regression result is confirmed, while its mirrored
improvement result is inconclusive; no speedup is admitted. The remaining U3/U4 rows have no
admitted AB/BA estimates. All reports retain matched consumed-output identities and identical
base/head fixture bytes for all six selected inputs in each cohort.

The parent verified the preregistration seal and each recorded log hash, then used the exact
checked-in `compare_self.py` harness (SHA-256
`a553540abab962044a3aa5950748c63f47533031d7d76edc13cc6a907e71b46e`) to deterministically
recompute every A/A calibration and every admitted AB/BA interval and classification. It also
checked clean runner revisions, frozen/source executable digests, comparable fixture bytes,
matched output receipts, one-operation divisors, postflight verification and alternating
AB/BA order. These are read-only checks of retained observations; no samples were collected
or reused.

The machine-readable terminal audit is retained at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/controlled-host-20261023T083654Z/terminal-admission-audit.json`
with SHA-256 `e067810530c1ba87ca728a84158fa2b4d7e1aa6646c01dae0ef526065bf4079`. It records
48 A/A recomputations and 128 admitted AB/BA pairs (256 runner samples), and confirms that no
matching benchmark process remained at the audit time.

| Cohort report | SHA-256 |
| --- | --- |
| `u3-end_to_end.json` | `71bf08a1f0700ace3432123f2b6b4512e41077710bf683ca55b264851dae03a0` |
| `u4-end_to_end.json` | `1f89ffcb5ec7eb41ebedbc0ed81bf584f84e49625b532fb8ce2cd5a70725169d` |
| `integrated-parse_cold_engine.json` | `cad8bf07da46354c1f7e09d384cd85570fe5539c5b83de5d7e31edbbdd53933f` |
| `integrated-end_to_end.json` | `9dc6363e1d20ab9f9003a07396eb8764866efc21d0e7d85cdbf08948d41098d0` |

All 24 scheduled rows are terminal; no measurement process remains. The experiment does not
complete the required timing controls or establish an improvement. Any further measurement
requires a separately agreed experiment; no automatic retry, extra samples, threshold change,
or acceptance-contract change is authorized by this receipt.
