# Native library release observation: v0.7.0 to 01cc4562f

Measured on 2026-10-05 (Asia/Shanghai). This checkpoint measures the `merman` crate's reused-engine SVG calls inside a native Criterion process. It excludes CLI process launch, argument parsing, filesystem output, and network icon acquisition.

**Result:** six of seven valid selected inputs rendered faster than v0.7.0. Mindmap was consistently slower in eight fresh alternating pairs: the paired median increased 11.1%, or 38.83 microseconds. Both explicit Dagre/classic configuration controls were about 32% faster. These are release-product observations, not a claim that every family, input scale, or same-output implementation improved.

The machine-readable [receipt](baselines/library-v070-to-01cc4562f-2026-10-05.json) retains independent estimates, output identities, executable digests, recipes, excluded cases, and raw-artifact digests. No production rendering code was changed. The follow-up items below are recorded for later investigation at the maintainer's request.

**Coverage update:** the [expanded native corpus observation](library_rendering_expanded_2026-10-05.md) extends this checkpoint to all 72 registered SVG fixtures and three preprocessing controls, resolves the missing AgentFlow/Usecase registrations, and records additional slower cases and historical identity blockers. The selected-input conclusions below retain their original scope and evidence; they are not a whole-corpus performance pass.

## Frozen sources and measurement

| Item | Recipe |
| --- | --- |
| Base product | `v0.7.0`, `00a2f291025913d612b6ca1ce9b6df6daf5bef94` |
| Head product | Merged PR #165, `01cc4562f74abaa1c7d31a44881db69296cacb3f` |
| Historical benchmark-only adapter | `dcd4e9d9ea2abc4c279e5f66c59bb7af58a7bb8e`; changes only `crates/merman/benches/pipeline.rs` |
| Explicit-configuration benchmark commits | Head `48eb201e3`, base `01d02d543`; only benchmark code and fixtures differ from the product revisions |
| Host | Windows 11 10.0.26200, Intel Core i9-13900KF, x86_64 MSVC |
| Compiler/profile | Rust 1.95.0, optimized `bench` profile on both sides; original locked dependencies retained |
| Base features | `--no-default-features --features render` |
| Head features | `--no-default-features --features all-diagrams,svg,layout-cytoscape,layout-elk,math` |
| Lifecycle | Reused process and engine, one library operation per estimate, strict parse, headless SVG, no filesystem I/O inside timing |
| Criterion | Base 0.5.1; head 0.8.2; 30 internal samples, 2-second warmup, 3-second measurement per independent invocation |
| Broad stage observation | Eight identical tracked inputs; two independent pairs per valid stage/input, base/head then head/base |
| Focused follow-up | Eight same-executable A/A pairs on each side for Mindmap; then eight fresh alternating A/B pairs each for Mindmap and Sequence |
| Configuration control | Two independent alternating pairs for each of two identical explicit Dagre/classic inputs |

Cargo builds were serial with one job and reused the existing target directory. Each executable was copied to an experiment-owned path, hashed, and checked during measurement; measured source paths and benchmark digests were captured from Cargo output. Both production sources and original Cargo.lock files remained unchanged. Correctness smoke checks preceded timing, and successful records require nonempty SVG XML plus stable output identity within each admitted executable/lane. The first failed State preflight and later identity failures are retained, not converted into fast timings.

There are 112 valid broad-stage timed invocations and 72 focused/control timed invocations, each with retained Criterion `sample.json`. Internal Criterion samples are not independent A/B repetitions. The focused run is an eight-pair descriptive follow-up, not power-sized `compare_self.py` confirmation. Selected source bytes match across releases, but output SVG bytes differ across releases; the strict comparator was not weakened.

## Complete library SVG rendering

Times are microseconds. Base/head columns are medians of independent estimates; change and delta are medians of paired head/base ratios and paired differences. These statistics need not equal a ratio or difference calculated from the two displayed medians. Sequence and Mindmap use the fresh eight-pair follow-up; the other rows use the two-pair broad observation.

| Fixture | v0.7.0 us | Head us | Paired change | Paired delta us | Pairs |
| --- | ---: | ---: | ---: | ---: | ---: |
| `flowchart_small` | 685.53 | 437.02 | -36.2% | -248.50 | 2 |
| `flowchart_medium` | 4640.45 | 1677.75 | -63.8% | -2962.70 | 2 |
| `flowchart_nested_clusters` | 2693.25 | 1279.95 | -52.5% | -1413.30 | 2 |
| `sequence_medium` | 460.12 | 419.88 | -8.6% | -39.55 | 8 |
| `class_medium` | 1302.80 | 1019.25 | -21.7% | -283.55 | 2 |
| `gantt_medium` | 198.96 | 51.82 | -74.0% | -147.14 | 2 |
| `mindmap_medium` | 352.76 | 389.82 | +11.1% | +38.83 | 8 |

The broad pass originally found Mindmap +8.6% (+30.94 us); the independently collected follow-up found +11.1% (+38.83 us). All eight follow-up pairs were slower, ranging from +7.6% to +17.6%. Base A/A second/first median was 1.0155x (range 0.9707-1.0216x); head A/A median was 1.0059x (range 0.9815-1.0514x). The paired slowdown persisted beyond the observed A/A range. Sequence, collected as a cross-family control in the same schedule, remained faster by a paired median 8.6% (-39.55 us).

The Mindmap median does not exceed the plan's joint fallback `>10% AND >50 us` materiality gate. It is nevertheless a reproducible slower product observation worth investigating, not a regression pass. A release claim or candidate fix needs a preregistered comparable-operation admission test with appropriate power and a reviewed output contract.

## Separating default changes from implementation work

v0.7.0's generated default layout is Dagre; the head's default is ELK, with changed Flowchart/Class presentation. Old headless defaults also use `VendoredFontMetricsTextMeasurer`, while the current deterministic environment uses `DeterministicTextMeasurer`. The main table measures the selected product recipes as an embedder would call them; it does not isolate one refactor, text policy, backend, theme, feature, or dependency.

Two additional inputs explicitly request `layout: dagre`, `look: classic`, `theme: default`, and Flowchart `htmlLabels: true`, `wrappingWidth: 200`, `minNodeWidth: 0`. Both benchmark adapters use their revision's `DeterministicTextMeasurer` for these rows. The source bytes match on both sides. The requested configuration is aligned, but the metric implementation, resource/accounting policy, Mermaid semantics, and generated SVG can still differ.

| Explicit configuration | v0.7.0 us | Head us | Paired change | SVG element count, base/head |
| --- | ---: | ---: | ---: | ---: |
| `flowchart_medium_dagre_classic` | 4407.60 | 3004.90 | -31.8% | 683 / 683 |
| `flowchart_nested_clusters_dagre_classic` | 2625.40 | 1781.95 | -32.1% | 269 / 269 |

Both controls remain faster, so the measured Flowchart benefit is not solely a consequence of switching the default backend to ELK. These two-pair results do not attribute a percentage to a particular optimization. SVG sizes still differ: 77,990 versus 80,751 bytes and 37,964 versus 40,089 bytes, respectively.

## Stage attribution

Ratios below show the range of the two independent head/base pairs, not a confidence interval. A ratio above one is slower. Later private-stage runs show substantial shared host drift, so their exact absolute medians are not stable enough for a release claim. Preserve individual rounds rather than adding independently timed stages together.

| Fixture | Parse ratio range | Layout ratio range | SVG-emission ratio range |
| --- | ---: | ---: | ---: |
| `flowchart_small` | 0.81-0.86x | 0.34-0.35x | 1.64-1.71x |
| `flowchart_medium` | 1.11-1.16x | 0.20-0.21x | 1.18-1.27x |
| `flowchart_nested_clusters` | 0.94-0.99x | 0.26-0.32x | 1.98-2.19x |
| `sequence_medium` | 0.86-0.89x | 0.79-0.80x | 1.41-1.46x |
| `class_medium` | 1.12-1.12x | 0.53-0.58x | 0.94-1.66x |
| `gantt_medium` | 0.10-0.11x | 0.65-0.66x | 1.17-1.22x |
| `mindmap_medium` | 1.21-1.21x | 0.92-0.95x | 1.10-1.22x |

Current layout prepares an owned artifact and current SVG emission consumes that artifact through `iter_batched`; v0.7.0 SVG emission borrows a persistent layout. Setup and destruction boundaries differ. These private-stage ratios locate follow-up work; they are not equivalent-ownership microbenchmark regressions. The historical adapter replaces silent error returns with failures and black-boxes complete successful results, and records pre/post SVG controls outside timing.

The repeated Mindmap parse increase was about 21% (+32.14 us in the broad observation), with SVG emission also slower and no consistent layout slowdown. That makes parse/configuration/model construction and SVG preparation the first profiling targets. It does not yet prove which call, allocation, policy check, or commit caused the change. Flowchart, nested Flowchart, and Sequence SVG emission also have repeated higher ratios despite faster public complete operations. Class emission changes direction between rounds and remains noisy.

## Follow-up register

| ID | Priority | Observation | Next investigation / acceptance boundary |
| --- | --- | --- | --- |
| LIBPERF-01 | P1 | Mindmap public SVG +11.1%, +38.83 us paired median; all eight pairs slower. Parse is the leading stage signal. | Reproduce with a pinned runner, profile parse/configuration and SVG preparation, then bisect from an appropriate alpha optimization checkpoint. Keep default-policy and matched-policy comparisons separate. Preserve Mermaid semantics and require fresh public-operation confirmation before claiming a fix. |
| LIBPERF-02 | P2 | Repeated higher SVG-emission ratios for Flowchart, nested Flowchart, Sequence, Gantt, and Mindmap; complete operations mostly improved. Class emission is inconsistent. | Align artifact ownership/drop boundaries before attribution; inspect output growth, CSS/style serialization, and actual rendering work with a profiler. Treat these as hypotheses, not established causes. Do not remove required output to improve the benchmark. |
| LIBPERF-03 | P1 evidence blocker | Head `parse/state_medium` changes compatibility-projection hash within one invocation. Historical State SVG changes path coordinates/byte count between processes. All State timing lanes excluded. | Reproduce independently. Inspect HashMap-to-JSON ordering on head and historical path/seed determinism, without assuming either is a visible rendering error. Fix or explicitly define non-semantic identity only under the owner contract; do not disable the hash check to pass timing. |
| LIBPERF-04 | P1 coverage blocker | The original merged compiled pipeline lacks AgentFlow and Usecase across six groups: twelve selectors listed in the corpus are absent. The same gap remains with the new controls. | Restore the owner benchmark registrations and run the full compiled-list/preflight contract before claiming complete-family coverage. The two new controls add exactly twelve expected selectors and no extra coverage gap. |
| LIBPERF-05 | P2 release coverage | Seven accepted inputs cover five families; large stress inputs, the remaining families, memory, browser/WASM, Node, and mobile are not measured here. | Expand the native standard/stress suites after identity and coverage blockers are resolved; keep each transport and memory lane independent. |

No rendering fix, sanitizer shortcut, normalization change, or performance admission is made in this checkpoint. The register is ready for later subagent investigation; no claim is made that an alpha-era gain survived every later commit or that increased source size itself explains runtime cost.

## Reproduction and validation

The local experiment is `target/bench/experiments/stable-library-2026-10-05/` in the main workspace, outside both worktree-owned target folders. It retains `experiment.yaml`, `measure.py`, `followup.py`, original and control executables, build receipts, complete benchmark-only patches, raw logs, output SVGs, and Criterion samples. The checked-in receipt records SHA-256 for the major artifacts. Local benchmark commits are supporting evidence and have not been published as release commits.

The owning build pattern is:

```console
cargo +1.95.0 bench --locked -p merman --no-default-features --features <recipe-above> --bench pipeline --no-run --jobs 1
```

Invoke the frozen executable separately for each selector, using `--bench --color never --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 --exact end_to_end/mindmap_medium`; use a unique `CRITERION_HOME` per run to retain samples. The broad pass also invokes `parse`, `layout`, and `render` independently. A direct single invocation does not reproduce the complete AB/BA schedule. Base adaptation and current request setup are retained in the patch and source digests.

Validation: both locked optimized builds passed; the targeted pre/post SVG checks passed for the seven included inputs; all 217 Python owner-contract tests passed (121 performance, 16 baseline-manifest, 51 native-memory, 29 native-memory-driver); `cargo fmt --all -- --check` and `git diff --check` passed. The complete compiled-list contract fails on both the untouched merged executable and the control executable with the same twelve missing AgentFlow/Usecase selectors; this failure is intentionally reported, not waived. The two newly added fixtures appear in every expected group. No fresh full SVG DOM/image parity matrix or cross-platform performance claim is made.
