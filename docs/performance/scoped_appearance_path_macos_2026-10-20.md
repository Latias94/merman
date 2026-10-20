# Scoped appearance path allocation — October 20, 2026

## Decision

The registered allocation, behavior and validation gates pass. This is an
adjacent allocation/work-removal experiment, without a latency or v0.9.0 convergence claim.

## Change

`AppearanceDecision::apply_to` constructed a scoped key for each selected `theme`, `look` and
`layout` value before checking whether that scoped key existed. The candidate moves only that
construction into its existing consumer branch. Presence lookup, root assignment, scoped assignment,
same-value explicit ownership and detector replay retain their order and behavior. No new cache,
classifier, public API, configuration traversal or production instrumentation is introduced.

The clean base is `88351fe6ab8a833036951bb2c6c41506f1035be0`, after the prepared-render main merge.
The source patch SHA-256 is `5a9aa6823c2d15ac007e43ff885d837e961a6ce9eb94fa1563e1181f5e1f93a9`;
the candidate appearance source SHA-256 is
`868cc775458280f4a25888b08ef5778c2acc6800515d01811876500710c08fca`.

## Registered measurement

The public operation remains `Renderer::render(RenderRequest::svg(...))` under the all-features
release build, deterministic environment and trusted-native resource policy. Compiled themes are
prepared before measurement; one warmup precedes five saved samples. The complete rich SVG result
is retained at each memory checkpoint and dropped afterwards. The primary is Packet medium / None,
with a strict allocation-count decrease. Allocated bytes, peak growth and retained growth cannot
increase on jointly stable controls. Variable controls remain inconclusive without resampling.

The current design API requires replacement inputs for 276 legacy themed controls and 13 themed
errors. All 22 designs explicitly state the old materializer's paint/font consumers, but generated
rule provenance, recipe identity and Radar palette ordering differ. The 369 None site inputs and
13 None error inputs retain their bytes. Names and counts correlate old and new coverage; they do
not establish equivalence to legacy records. All comparisons in this report use freshly captured
current-base and candidate inputs. Historical measurements remain unchanged.

The private probes adapt only their pre-measurement theme compilation to the current public design
API. Prepared-source redraw and SVG-bytes-only calls are not substitutes for the measured operation.
The exact 26 current errors were captured, source-reviewed and frozen before candidate application;
comparison includes complete debug text, diagnostic details and order.

## Results

| Packet medium / None primary | Base | Candidate | Change |
| --- | ---: | ---: | ---: |
| Allocation count | 115 | 109 | -6 |
| Allocated bytes | 19208 | 19136 | -72 |
| Peak growth bytes | 11742 | 11742 | 0 |
| Retained growth bytes | 3951 | 3951 | 0 |

All 418 catalog cells are stable: 396 improve at least one of the four memory metrics and
22 retain all four, with no increases. Warmup result metadata and all five saved sample SVG/report
identities match. All 645 site-control final SVG/report identities and independent ordered-measurer
traces match. Of 505 jointly stable site controls, 494 improve and 11 retain all four metrics;
140 variable controls remain inconclusive without resampling. All 26 complete errors match.
These observations count avoided allocations; they do not establish a clock improvement.

## Validation

| Check | Result |
| --- | --- |
| Focused appearance/theme/config/detector release tests | 1038 pass |
| Four-crate all-features release nextest | 8100 pass, seven skipped |
| Matching strict release Clippy | Pass with `-D warnings` |
| Full 37-entry DOM matrix | Pass under the existing policies; fresh reports archived |
| Optimized full WASM, including wasm-opt | Pass; seven archived package files, including the input manifest |
| Release theme acceptance | 34 pass |
| Format, diff and indexed path length | Pass |

The ignored experiment is `target/bench/experiments/appearance-scoped-path-current-20261020/`.
It retains the registration, its focused-selector and target-spelling amendments, raw five-sample outputs,
ordered-measurer traces, exact errors, baseline executable hashes, source freezes and independent
reviews. All 15 serial stages reach successful terminal states; all 15,303 frozen source hashes
remain unchanged through validation. The WASM check verifies an optimized build, without a browser
runtime claim. The driver uses one shared target directory and serial Cargo/native commands. No new
benchmark dependency, tracing or CI benchmark job is added.

## Remaining work

The latest formal v0.9.0 comparison still belongs to `a6742779c`; it does not include this repair or
the new main integration. Continue with an independently registered CSS ownership experiment, then
re-establish comparable release lanes and fresh A/A plus fixed AB/BA measurements. The all-family
theme convergence goal remains incomplete.
