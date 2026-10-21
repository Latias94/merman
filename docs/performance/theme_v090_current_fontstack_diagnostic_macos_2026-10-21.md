# Current FontStack Release-Range Diagnostic

This is a fresh diagnostic comparison of production head `68f213f1ea4676102f735bac4c158c136498eb18`
against Mermaid-compatible Merman `v0.9.0` production commit `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`.
The comparison uses the same 19 fixtures whose complete SVG bytes match between those revisions.
It does not replace the previously admitted formal `14/2/3` classification for production `b9a58731c`.

## Method and admission

The comparison uses the existing public `Renderer.render(RenderRequest::svg(...))` rich-result
benchmark. Engine and renderer reuse, request/options construction, rendering, full SVG observation,
and ordinary result destruction stay inside the timed closure. It does not time the bytes-only API.
Both revisions use the same bench-only snapshot of the public end-to-end function, Rust 1.95.0,
default features plus `all-diagrams,svg,layout-cytoscape,layout-elk,math`, and isolated existing
Cargo target directories. The base and head builds freshly compiled `merman-core`, `merman-render`,
the facade and the benchmark at opt-level 3. No capture hooks were present. Frozen executable and
runtime library identities remained unchanged at discovery, before the first sample and after
timing.

The fresh discovery covered 76 exact input fixtures and reported no global contract errors. It
found the same 19 cross-version exact SVG matches and 57 output-mismatch exclusions. The 152
existing physical SVG files were independently rehashed (7,175,894 bytes total); each full SHA-256,
byte length and element count matched both the new no-hook renderer receipt and its previous
same-side audited witness. This content-addressed check avoids rebuilding capture-hook executables.
The inventory command itself exits 2 because the 57 cross-version output differences are reported
as non-comparable rows. Its first wrapper then hit a local validator `KeyError` after writing the
inventory; a registered validator-only correction read the already bound pipeline digest from the
original plan and independently admitted the unchanged 76-row output. It did not rerun rendering
or timing.

Each of the 19 rows then received four fresh balanced `AB, BA, AB, BA` pairs. Every Criterion call
used 20 samples, one-second warmup and one-second measurement. The comparison uses explicit 10%
relative and 1,000 ns absolute materiality thresholds. All 152 per-call preflight and postflight
SVG receipts matched; no global or row contract errors occurred. This is an **uncalibrated diagnostic**:
its bootstrap bounds and point estimates are useful for choosing the next investigation, but they
do not establish release non-regression or change any formal classification.

## Results

Positive deltas are slower on the current head. Values are arithmetic means of the four paired
Criterion estimates; the percentage is the head/base ratio. Rows stay in the fixed registered
19-scene cohort.

| Fixture | v0.9.0 (µs) | Current (µs) | Change (µs) | Change (%) |
| --- | ---: | ---: | ---: | ---: |
| architecture_medium | 44.484 | 46.711 | +2.227 | +5.01 |
| c4_medium | 104.243 | 109.692 | +5.450 | +5.23 |
| class_medium | 650.997 | 643.342 | -7.655 | -1.18 |
| class_namespace_dense | 400.475 | 408.640 | +8.165 | +2.04 |
| class_nested_namespaces | 1,595.700 | 1,621.275 | +25.575 | +1.60 |
| class_nested_namespaces_large | 15,433.500 | 15,636.500 | +203.000 | +1.32 |
| class_tiny | 127.040 | 135.542 | +8.502 | +6.69 |
| cynefin_medium | 28.815 | 30.713 | +1.899 | +6.59 |
| gantt_medium | 36.237 | 38.613 | +2.376 | +6.56 |
| journey_medium | 27.241 | 28.229 | +0.988 | +3.63 |
| kanban_medium | 44.328 | 52.883 | +8.555 | +19.30 |
| packet_medium | 7.950 | 10.065 | +2.115 | +26.60 |
| pie_medium | 25.185 | 27.378 | +2.193 | +8.71 |
| quadrant_medium | 26.659 | 27.807 | +1.148 | +4.31 |
| requirement_medium | 234.903 | 241.875 | +6.973 | +2.97 |
| sankey_medium | 23.852 | 25.811 | +1.958 | +8.21 |
| timeline_medium | 39.274 | 41.511 | +2.237 | +5.70 |
| wardley_medium | 40.205 | 43.318 | +3.112 | +7.74 |
| zenuml_medium | 19.525 | 21.614 | +2.089 | +10.70 |

Eighteen rows have a slower point estimate and one is faster. Only `journey_medium` is within both
goal thresholds at its mean. `kanban_medium`, `packet_medium` and `zenuml_medium` exceed both
thresholds in this diagnostic. The previously admitted formal checkpoint for production
`b9a58731c` classified Packet and ZenUML as regressions and Kanban as inconclusive. Later formal
checkpoints on other production heads retain their own classifications; none changes that b9 result
or supplies a formal result for `68f213f1e`. A new formal A/A-qualified run is needed to classify
this head. The other 15 slower means are not evidence that every one has a distinct regression:
their scales range from roughly 1 µs to 203 µs, and this run is not calibrated.

The run compares the cumulative changes from v0.9.0 through this head; it cannot attribute a delta
to the FontStack change alone. The separate FontStack experiment demonstrates clone-allocation
removal for typed presets, while this public release-range diagnostic does not isolate that work.
The 57 SVG-different fixtures are excluded rather than normalized into equivalence. Typed preset
allocation results remain an adjacent lane because v0.9.0 has no matching typed request API.

## Evidence

The preregistered recipe and raw outputs are in the ignored local experiment directory
`target/bench/experiments/theme-current-v090-refresh-20261021/`. The key report SHA-256 is
`f562092d40c2feada300b4b9a801f085743dac871399d254ec3eeefc999ecea5`; the fresh 76-scene inventory
SHA-256 is `fd2e18e349c3c9c9b30dfbcdc3be99bc6fad375c1288f3d38196da2002b28a90`. Frozen source
snapshots were `2a1ebf70af98c01ca74a64cb4567af4c1bc8880f` (base) and
`6ede7452adb092d5436176b0b28795153acfe160` (head); each changes only the bench source to retain
the identical public end-to-end benchmark across the two production APIs.
