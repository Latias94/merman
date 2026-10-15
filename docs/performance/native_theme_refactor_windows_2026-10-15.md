# Native theme data-flow refactor on Windows, 2026-10-15

Base: merged theme main `b9844db7b747111690bf6369d4536885a85e35a7`. Candidate: uncommitted patch on `perf/theme-regression-repair-20261015`, recorded by `candidate-source.json` and `candidate-production.patch` in the local experiment directory.

This checkpoint implements three shared ownership/data-structure repairs. It removes the reachable quadratic provenance term and reduces terminal allocation and peak live-heap growth. It does **not** close the ordinary SVG latency regression against v0.9.0. The end-to-end observations below are diagnostic; no latency improvement is admitted. See the preceding [attribution checkpoint](native_theme_attribution_windows_2026-10-15.md) for the original regression and call boundaries.

## Implementation

- Classic staged themes use one value evaluator with optional authority bookkeeping. Authored configuration is borrowed; actual reads seed compact source IDs and cache negative ownership queries. Empty computed ownership blocks authored fallback. Sparse exact/prefix replacement preserves shallow replay and literal dotted-key barriers. Final propagation remains transactional and uses original source identities.

- The SVG writer borrows an immutable operation-owned byte ceiling and keeps explicit admitted capacity. Allocator excess capacity grants no byte credit. Geometric admission removes repeated policy work while every write still observes cancellation. Logical CSS uses bounded UTF-8 scanning, explicit operation observation and the existing XML escaping/write boundaries; serialized bytes, producer accounting and sticky first failure remain separate.

- Terminal validation still performs complete XML parsing. Preorder parent indices and sparse ID/reference/effect arenas replace per-element child/reference vectors and duplicate equal ID strings. References/effects determine whether graph expansion is needed. A single edge arena and contiguous adjacency ranges preserve duplicate-ID unions, ordering, cycles, saturation and conservative effect amplification. Static/browser conversion also checkpoints both nodes and edges. No process-global cache or family allow-list is introduced.

## Structural provenance evidence

For the registered family, E is the number of fixed-width, dot-free, unknown scalar overrides and A is actual staged update commits. The E-controlled contribution to the former whole-map retain predicates was exactly `E(E-1)/2 + A*E + E^2`, in addition to fixed theme-program work. Those terms are removed. New overlay creates no self-dependency entries; unknown-key replay adds one subtree probe per key. Tests cover Base, Dark, Forest and Neutral with E = 0/64/256/1024/4096 and assert no source queries or derived source/dependency projection, a fixed deletion count and exactly delta-E additional subtree probes.

General provenance is not claimed globally linear. It still pays for actual reads, authored object-boundary probes, affected subtree writes/deletions, owner-ID unions/copies and host ownership queries. Barrier preparation scans key dot positions and checks explicit prefixes, without pairwise key comparisons. Each inserted replacement can be deleted at most once. Added authority space is `O(B + (M+Q)P + T_peak)`: B borrowed overlapping root prefixes; M peak replacement entries; Q distinct queried source paths including negative results; P path bytes; T_peak simultaneous owner-ID memberships. For the registered family B = Q = T_peak = 0 and M depends on actual theme writes rather than E. Values remain separately proportional to input.

Counters exclude comparisons, tree traversal steps, key scanning, allocations, value cloning, color evaluation and final ownership propagation. Test-only snapshots observe cloned authority and do not inflate interpreter counters. An independent eager-bookkeeping oracle covers replacement/replay, same-value distinct ownership and the empty-object dotted-root counterexample.

The public Dark/frontmatter scale probe runs three rotating revision orders, two warmups and five parses per scale in each process. The table uses medians of 15 individual operations; this is an uncalibrated scale diagnostic, not a latency admission. Result destruction is outside the timer.

| Extra variables | v0.9.0 (ms) | Merged (ms) | Candidate (ms) |
| ---: | ---: | ---: | ---: |
| 0 | 0.2220 | 0.7083 | 0.6933 |
| 64 | 0.5051 | 1.0689 | 1.1446 |
| 256 | 1.1991 | 3.0580 | 2.5170 |
| 1,024 | 4.0471 | 15.4211 | 6.9924 |
| 4,096 | 16.4324 | 148.1264 | 30.3618 |

## Terminal allocation evidence

The probe reuses the checked-in native-memory counting allocator. It measures one public `RenderedFamilySvg::finalize_standalone(None)` call, with input rendering outside measurement and returned artifact destruction afterward. Five operations follow two warmups. Counts are stable across the five samples; every output retains exact SVG SHA-256, terminal status and resource fingerprint. Temporary validation storage and untransferred input destruction are inside the boundary. This does not measure process RSS, allocator overhead, whole-operation memory or browser rendering. Timing uses a separate uninstrumented executable.

| Fixture | Allocations: merged / candidate | Cumulative allocated bytes | Peak growth bytes |
| --- | ---: | ---: | ---: |
| class_medium | 1,699 / 746 | 332,099 / 120,091 | 160,850 / 30,234 |
| journey_medium | 277 / 174 | 50,111 / 23,761 | 21,140 / 4,960 |
| class_tiny | 340 / 172 | 53,060 / 28,030 | 24,804 / 8,190 |
| xychart_medium | 175 / 149 | 39,373 / 13,919 | 14,856 / 1,850 |

Class medium reduces allocation count by 56.09%, cumulative allocation by 63.84% and peak growth by 81.20%. These are exact probe observations at this boundary, not an inferred end-to-end speedup.

## Ordinary latency remains open

Host: Windows 11, Intel Core i7-11700, Rust 1.95.0, x86_64-pc-windows-msvc. Frozen executables use locked default features plus `svg`, a reused Engine/Renderer, strict parsing, deterministic measurement and trusted-native resources. No compiler runs during timing. The identical checked-in pipeline harness supplies pre/postflight SVG hashes and element counts. Three rotating/alternating rounds use 30 samples, 2-second warmup and 3-second measurement; values below are arithmetic means of independent console estimates.

| Fixture | Merged (us) | Candidate (us) | Candidate / merged | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 3198.867 | 2988.800 | -6.57% | -210.067 |
| journey_medium | 286.503 | 289.587 | +1.08% | +3.083 |
| class_tiny | 736.393 | 702.640 | -4.58% | -33.753 |
| xychart_medium | 464.087 | 460.083 | -0.86% | -4.003 |
| flowchart_medium | 4265.267 | 4151.000 | -2.68% | -114.267 |
| sequence_medium | 1322.567 | 1330.500 | +0.60% | +7.933 |
| kanban_medium | 361.477 | 337.703 | -6.58% | -23.773 |

No row satisfies the registered 10% plus 50 us improvement gate. Journey includes a high first candidate observation (367.330 us versus later 253.120/248.310 us). A fresh four-pair same-binary A/A diagnostic observes consecutive candidate shifts from -3.64% to +7.17%; it is not the eight-pair confirmation protocol. The source is an uncommitted frozen patch, so these data cannot serve as a clean-revision admission receipt.

A fresh three-round public terminal diagnostic also fails to establish a consistent terminal latency improvement. Median batch-normalized merged/candidate times are Class medium 754.993/799.004 us, Journey 117.109/127.869 us, Class tiny 137.349/124.487 us and XY Chart 133.254/137.397 us. Do not subtract this independently batched experiment from end-to-end measurements. Less allocation alone does not demonstrate less CPU time.

Fresh historical same-output controls retain a large gap: v0.9.0 means are Class medium 1972.600 us, Journey 71.504 us and Class tiny 460.183 us. These are separate release-range context, not proof of one candidate's causality. The architecture frontier remains generated XML being reparsed/validated and other shared terminal ownership work.

## Rejected CPU experiment

An independent bounded ASCII-block candidate for XML character validation preserved the original
64-scalar checkpoint schedule, first-invalid error and final checkpoint in 1,589 extracted
result/cancellation differential cases. The first variant reduced ASCII CSS microbenchmark time
by 65.67% but regressed dense Unicode by 13.45% in 16 alternating pairs. A simpler scalar-tail
fallback reduced ASCII CSS time by 62.52% but regressed the Unicode control by 37.35%.
Both variants were rejected and never entered this patch. Extracted probes, source hashes and
all raw controls remain under rejected-xml-character-blocks/ in the local experiment directory.
These are microbenchmark diagnostics, not public-operation performance claims.

## Correctness and review

The optimized core/render run passes 5,228 tests with the two newly added invalid-zero-budget cases excluded. Those cases originally failed during public policy construction, before writer execution; zero is forbidden by the pre-existing policy. They were corrected to the legal minimum budget of one (and two for the differential matrix). The corrected writer group passes all 13 tests, including all 56 limit/cancellation differential combinations. This rerun keeps the release dependency profile but sets only merman-render optimization to zero to avoid repeating the large optimized test code generation; it is correctness evidence, never timing evidence. No production policy was relaxed.

The public facade passes 125 focused integration tests: prepared rendering, theme authoring/isolation, security, operation/resource boundaries, ResvgSafe representative fixtures, all benchmark fixtures and XY Chart label/series styling. Clippy passes all core/render targets with all diagram/layout/math features and warnings denied. Workspace cargo fmt and git diff --check pass.

The frozen pipeline smoke emits 459 preflight identities per executable. All 152 `render` / `end_to_end` SVG identities match exactly; the other 300 matching projections and seven State JSON mismatches are retained in the diagnostic report, not suppressed. State uses a pre-existing randomly seeded `HashMap` and `serde_json/preserve_order` when projecting `.states`, also embedded in layout semantic JSON. A 48-dump public-projection probe compares complete JSON semantics with every array position preserved: repeated tiny/medium outputs are equal, but object member order changes. Reordering only the tiny `.states` byte slice reconstructs both frozen historical hashes exactly. All repeated tiny/medium raw bytes also match after changing only the states-object member order; the historical medium hash alone cannot reconstruct every historical member order. No comparator or State production code was changed.

Three disjoint implementation worktrees were cross-reviewed. Review found and fixed the CSR conversion cancellation gap and an empty-object literal dotted-root replay barrier. Existing production XML/attribute behavior is kept; independent test-only legacy collectors, eager bookkeeping and scalar serialization provide differential oracles. The terminal graphs cover duplicate raw/parsed IDs, namespace aliases, entity normalization, edge order, cycles, effect amplification and saturation. The writer covers allocator excess credit, exact/below limits, replacements, UTF-8, split `]]>` and cancellation/first-failure boundaries.

The complete upstream DOM structure gate passes across 37 diagram groups using the exact repository command below and its unchanged reviewed browser-text-layout/parser residual policy. Reports are preserved in the experiment's dom-structure-reports directory. This mode does not check root viewport or browser-painted containment. Workspace `xtask verify --strict` has not run. The latter includes unrelated Web, editor, package, platform and release checks. These results do not replace that CI owner lane. This checkpoint does not establish release readiness; its measurements were collected before implementation commits.

```powershell
cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

## Local evidence

Raw output and exploratory probes remain under `target/bench/experiments/theme-repair-20261015/`: experiment.yaml, candidate-source.json, candidate-production.patch, candidate-frozen-build.json, core-render-*.log, facade-nextest.log, all-bench-identities-*.log, terminal-alloc-*.jsonl, terminal-paired.json, staged-paired.json, end_to_end-3pairs.json and journey-aa-diagnostic.json. The earlier frozen base/merged binaries remain in the attribution experiment; no commit or push had been performed when these measurements were frozen. The candidate benchmark executable is 40,279,040 bytes versus 40,287,232 bytes for merged under the same recipe; this is an artifact control, not a package-size claim. Cargo manifests and lockfile are unchanged.

The next latency experiment should remove a proven duplicate data-flow boundary rather than add a family classifier. A complete structured XML writer can create validated facts from actual emitted events, carry those facts through root/theme/family ownership and invalidate them for opaque mutation. Journey is a bounded initial feasibility family; arbitrary fragments retain the current complete parser. Such a change must preserve terminal error order, prefix resource admission, cancellation, namespaces and exact bytes before it can be measured or expanded.
