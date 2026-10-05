# Matched Dagre library rendering: v0.7.0 to 01cc4562f

Measured on 2026-10-05 (Asia/Shanghai). This is the release-comparison scope selected by the maintainer: **v0.7.0 Dagre versus current Dagre**, measuring the complete `merman` crate SVG call inside a reused native process and engine. CLI startup and file/network I/O are excluded. ELK has no v0.7.0 Flowchart implementation baseline, so no ELK release comparison is made. Alpha.7 is excluded; its unused checkout was removed without building or measuring it.

**Observed result:** all six selected Dagre controls have lower current-version estimates in both recorded pairs, with paired-median reductions of about 15%-33%. The four stress cases that appeared slower under changed release defaults instead have 15%-21% lower costs with explicit Dagre/classic settings. These are descriptive observations, not statistically confirmed speedups: both same-executable A/A calibrations failed the preregistered noise gate. No production optimization or rendering fix was made.

The [machine-readable receipt](baselines/library-dagre-v070-to-01cc4562f-2026-10-05.json) contains every selected pair, output identity, source/executable hash, calibration result, and raw-artifact inventory. Earlier [whole-corpus default-product observations](library_rendering_expanded_2026-10-05.md) remain historical records and must not be presented as a like-for-like Dagre regression table.

## Compared operation and settings

| Item | Contract |
| --- | --- |
| Base product | v0.7.0, `00a2f291025913d612b6ca1ce9b6df6daf5bef94` |
| Current product | `01cc4562f74abaa1c7d31a44881db69296cacb3f` |
| Retained benchmark controls | Current `445c5bc9722472c5be7be3f2356b751f7ef5f5b1`; historical `d1f5f3a8700b0db986ce6a2a6a38fe3ade97bad6` |
| Target/toolchain | Windows 11, Intel Core i9-13900KF, x86_64 MSVC; Rust 1.95.0 optimized bench profile |
| Features | Base: `--no-default-features --features render`; current: `--no-default-features --features all-diagrams,svg,layout-cytoscape,layout-elk,math` |
| Requested backend and appearance | `layout: dagre`, `look: classic`, `theme: default` |
| Flowchart settings | `htmlLabels: true`, `wrappingWidth: 200`, `minNodeWidth: 0` |
| Text policy | Each release's deterministic text measurer; explicitly selected in the historical adapter |
| Sampling | 30 internal Criterion samples, 2-second warmup, 3-second measurement; two independent base/head then head/base pairs |
| Correctness checks | Identical input bytes across releases; valid nonempty SVG; stable pre/post and cross-process output within each executable; graph and DOM cardinalities recorded |

Compiling ELK into the current executable does not select it: these inputs explicitly request Dagre. The current selector in `crates/merman-render/src/layout_backend.rs` resolves that name to Dagre, while the v0.7.0 Flowchart dispatch in `crates/merman-render/src/lib.rs` calls the Dagre-backed `layout_flowchart_v2_typed`. Source, original locked dependencies, and production configuration remain unchanged. The old adapter and new fixture registrations affect only benchmark inputs and checks. No parser error, fallback result, or empty SVG is timed as successful rendering.

The matched settings improve comparability but do not assert identical output work: the Mermaid version, remaining generated defaults, text-measurement implementation, resource accounting, SVG serialization, and Criterion 0.5.1/0.8.2 differ. Every admitted operation renders its complete revision-specific SVG.

## Selected complete-operation results

Times are microseconds. Base/current columns are medians of the two independent estimates. Change and delta are medians of paired ratios/differences, not calculations from rounded columns. Every row has two pairs; the nested-cluster result is reused from the initial experiment, not a fresh run.

| Fixture | v0.7.0 us | Current us | Paired change | Paired delta us | Origin |
| --- | ---: | ---: | ---: | ---: | --- |
| `flowchart_weave_dagre_classic` | 3693.80 | 2910.15 | -21.2% | -783.65 | Current scoped investigation |
| `flowchart_ports_heavy_dagre_classic` | 3328.50 | 2681.80 | -19.4% | -646.70 | Current scoped investigation |
| `flowchart_fanout_returns_dagre_classic` | 2315.40 | 1972.15 | -14.8% | -343.25 | Current scoped investigation |
| `flowchart_long_edge_labels_dagre_classic` | 3307.90 | 2683.05 | -18.9% | -624.85 | Current scoped investigation |
| `flowchart_medium_dagre_classic` | 8613.00 | 5780.40 | -32.9% | -2832.60 | Current scoped investigation |
| `flowchart_nested_clusters_dagre_classic` | 2625.40 | 1781.95 | -32.1% | -843.45 | Earlier checkpoint |

The medium control was rerun during this investigation: its paired reduction is 32.9%, versus 31.8% in the earlier experiment. Absolute times changed materially between sessions, including on the unchanged control. Do not join those sessions into one synthetic pair or claim a cross-session absolute latency improvement. The nested-cluster row keeps its original 32.1% observation and provenance.

### Output controls

| Stress input | Node groups, both | Edge paths, both | SVG elements, both | SVG bytes, base/current |
| --- | ---: | ---: | ---: | ---: |
| `flowchart_weave_dagre_classic` | 16 | 22 | 318 | 44,861 / 46,887 |
| `flowchart_ports_heavy_dagre_classic` | 18 | 39 | 423 | 62,072 / 64,264 |
| `flowchart_fanout_returns_dagre_classic` | 12 | 26 | 296 | 44,631 / 46,664 |
| `flowchart_long_edge_labels_dagre_classic` | 9 | 13 | 222 | 34,299 / 36,588 |

Equal graph/element counts rule out a simple omitted-node or omitted-edge explanation; they are not a full semantic or raster parity proof. Output bytes differ across versions. Rebuilding the retained Dagre-only benchmark registrations reproduces the recorded current SVG identities for all six controls.

## Calibration and claim boundary

Eight same-executable A/A pairs were collected independently for each side on `flowchart_weave_dagre_classic`, alternating role order. The registered margins were log(1.10) and 50,000 ns, using the existing comparator bootstrap at 95% confidence, seed 20261005, 10,000 resamples, and a maximum of 16 confirmation pairs.

- Historical calibration is inconclusive: identity and order intervals exceed their margins; the absolute-noise power estimate requires 1,308 pairs, above the fixed cap.
- Current calibration is also inconclusive: the identity absolute interval reaches about -55.8 us, exceeding the -50-us boundary, even though the estimated required count is eight.
- The failed schedule was not extended or retried until favorable. No fresh confirmation run or confirmed-regression/confirmed-speedup outcome is claimed. Internal Criterion samples are not independent release comparisons.

Recommended release wording: "On six selected Flowchart fixtures with explicit Dagre/classic settings, current native library rendering showed approximately 15%-33% lower paired-median time than v0.7.0. These are Windows-host observations; statistical confirmation remains outstanding." Do not say the whole library improved by that range, infer a cause from source-code growth, or attribute these results to a particular alpha commit.

## Disposition

- Retain four new Dagre/classic stress fixtures alongside the existing medium and nested controls. No new production switches, renderer changes, or CI timing gates are added.
- Preserve the earlier default-product and configuration exploration under the ignored experiment directory. ELK/theme comparisons and diagnostic profiling are outside this release-comparison report.
- Treat the four earlier default-configuration slowdowns as non-comparable to Dagre performance; the matched-Dagre observations do not reproduce their direction. Other historical signals, including Mindmap and output-identity blockers, retain their original scope and remain unresolved.
- A stronger release claim requires a new preregistered stable-runner experiment with A/A calibration and fresh balanced pairs. This checkpoint does not relax the existing comparator, rerun the failed schedule, or claim blanket non-regression.

## Reproduction and validation

The local campaign is `target/bench/experiments/flowchart-release-attribution-2026-10-05/` in the main workspace. Its original preregistration, raw attempts and executables remain immutable; `scope-update.json` records the maintainer-directed Dagre-only scope. `archived-source/` preserves the measured harnesses and the exploratory sources removed from the retained tree. The retained current and historical benchmark commits are listed above; the receipt distinguishes the original timed executable from the final validation-only rebuild.

```console
cargo +1.95.0 bench --locked -p merman --no-default-features --features <base-or-current-recipe> --bench pipeline --no-run --jobs 1
<frozen-pipeline> --bench --color never --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 --exact end_to_end/flowchart_weave_dagre_classic
```

Use a separate `CRITERION_HOME` for each invocation and preserve both AB/BA rounds, checks and samples. A single command does not reproduce the independent-pair schedule. Builds are serial, reuse the shared target directory, and do not run alongside timing. The scoped receipt includes 20 fresh A/B timed invocations plus 32 A/A invocations; four nested-control timings are retained through the earlier receipt.

Validation: the final frozen current executable has 459 expected benchmark selectors and 459 preflight receipts across seven groups. All six retained Dagre controls pass exact pre/post output checks and match their measured current SVG identities. All 217 Python performance-owner tests pass (121 performance, 16 baseline-manifest, 51 native-memory, 29 native-memory-driver); both checkouts pass `cargo fmt --all -- --check` and `git diff --check`. No production source, lockfile, Cargo feature, default backend or renderer policy is changed. Full renderer parity and production nextest suites were not rerun for benchmark-only inputs and documentation.
