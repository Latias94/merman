# Native Font Measurement Optimization Checkpoint

> Current policy (2026-10-12): ordinary CLI Mermaid graphical output defaults to
> deterministic measurement without a native font context or system discovery for
> layout. PNG/JPEG/PDF painting can still load system fonts through the existing codec
> path. Default/release profiles still compile native/system context support. Explicit
> files/directories enable native measurement with a closed font set;
> `--font-measurement native` without sources or
> `--font-system on` explicitly requests system discovery when available. The original
> executable defaults, measurements, and results below are historical and unchanged;
> this note does not claim a new timing run. See the [current CLI font guide](../../crates/merman-cli/README.md#explicit-fonts).

Date: 2026-10-10. Windows 11, Intel i7-11700 (8 cores / 16 logical processors),
Rust 1.95.0, x86_64-pc-windows-msvc, release/bench profiles. At the time of these
measurements, native fonts were opt-in at runtime and in library builds; the CLI
compiled the feature by default but did not automatically discover system fonts.
This checkpoint compares uncommitted implementation snapshots, not published
releases. The structural optimization did not change cache capacity, font
resolver or wrap policy.

The subsequent CLI quality policy enables actual-font measurement and bounded
system discovery by default for Mermaid graphical input. Its separate
[fresh-process system-discovery checkpoint](native_system_discovery_2026-10-10.md)
compares two already-native implementations, not native versus deterministic
measurement. The explicit-font deltas below
must not be presented as the new default's startup cost. Library defaults remain
host-independent. Release notes must link durable visual assets and reproducible
performance evidence rather than the ignored local experiment directories.

## CLI Compiled Versus Active Cost

This historical CLI distribution checkpoint enables `native-fonts` in source
defaults and the explicit release recipe, without activating a context unless
font sources are supplied. Two same-source Windows `release` executables differ
only in that feature; this is not the thin-LTO `dist` profile. Their sizes are 49,321,472 and
49,551,360 bytes (+229,888 bytes, +0.466%).

An exploratory process-latency probe uses three warmups and 30 fresh-process
samples per lane, alternating ABC/CBA. Both inputs request Cousine with
`htmlLabels: false`; explicit mode loads the test-only Cousine-Regular.ttf.
Timing includes startup, input, font acquisition, render and stdout capture;
OS file caches remain warm. All samples succeed and have stable lane-local SVG
hashes. The inactive executables produce identical output. Active mode changes
geometry, so its extra cost is a feature tradeoff, not a same-output regression.

| Input | Feature absent, ms | Compiled but inactive, ms | Explicit font, ms |
| --- | ---: | ---: | ---: |
| Two-node Flowchart | 45.56 | 43.35 | 46.66 |
| 25-node Flowchart | 44.68 | 44.07 | 52.17 |

| Input | Feature absent p95, ms | Compiled but inactive p95, ms | Explicit font p95, ms |
| --- | ---: | ---: | ---: |
| Two-node Flowchart | 50.47 | 48.85 | 55.43 |
| 25-node Flowchart | 49.21 | 50.05 | 57.69 |

p95 is the nearest-rank 29th sorted observation from each 30-sample lane
(`ceil(0.95 * 30)`), recalculated from the retained raw samples. It is a
single-run tail observation, not a confidence interval or repeated admission.

Values are medians. Active versus inactive median differences are +3.31 ms
(+7.6%) and +8.10 ms (+18.4%). Median paired differences are +3.58 and +8.51 ms.
No inactive slowdown is observed; the negative inactive deltas are not admitted
speedups. This single-host, single-font probe does not establish a universal
overhead, pure measurement cost, warm library throughput or an optimization
admission. The [versioned experiment ledger](evidence/cli-native-default-2026-10-10/experiment.yaml)
retains executable, lockfile and font hashes; the
[raw process samples](evidence/cli-native-default-2026-10-10/process-latency.json)
retain all 180 observations, lane output hashes and input hashes.
The original timing script and exact source inputs were not saved, and the
uncommitted executable source snapshots are not reconstructible from these
hashes. This is durable historical evidence, not an exactly reproducible
benchmark. See the [evidence limitations](evidence/cli-native-default-2026-10-10/README.md).

## Accepted Structural Change

Shared wrapping can return one measured payload per final emitted row. Native SVG
measurement uses that row's advance and vertical bbox together, removing a second
height-only shape request. Candidate callback order and final row order remain
unchanged. No cache is added, including when cache entries/bytes are zero.
Temporary payload storage is O(emitted rows), bounded by admitted label size.
The measurement budget continues to count actual native probes, so a request may
now succeed with a smaller budget when duplicate work has been removed.

An exact zero-cache test proves two unwrapped rows require two native probes,
not four. Callback sequence/payload tests cover SVG/HTML/single-run modes, blank
and whitespace labels, NBSP, explicit empty rows, graphemes and invalid widths.
Callback errors still stop immediately; native final checkpoints remain intact.

For the existing 3,136-byte long-wrap fixture, three AB/BA/AB pairs had identical
allocation counts per variant:

| Metric | Before | After |
| --- | ---: | ---: |
| Cumulative allocated bytes | 283,754,706 | 241,630,578 |
| Allocation count | 1,000,831 | 855,174 |
| Peak live heap growth | 133,339 | 133,339 |

Cumulative allocation decreased 14.85%; this is not RSS. Timing point estimates
were 174.88/166.78/164.45 ms before and 141.08/171.78/155.81 ms after. One matched
pair reversed direction. Timing remains diagnostic/inconclusive, not a confirmed
percentage speedup. The structural duplicate-work repair is retained on exact
work and allocation evidence plus correctness gates.

Command: `cargo bench -p merman --no-default-features --features
native-fonts,diagram-flowchart --bench native_font_measurement -- --quick
long_wrap_uncached`. Preserved executables were run with `--bench --quick` in
alternating order. Local logs and experiment contract live under ignored
`target/bench/experiments/native-final-lines/`.

Baseline executable SHA256: `8e3c4803b78990d8304ff646b9c08eb1dc73397bc8ac497957952f1292eea316`.
Candidate executable SHA256: `040d072d75f23d374ab79bd16fa2a17d92798dab3bfb750d84fe49fbd95e393b`.
Lockfile SHA256: `77c66b401c6e1400b3e4868b1dd6e2f8092abecbb1447df440f6a93912c90631`.

## Cold And Batch Attribution

The existing benchmark now distinguishes fresh-context full render, reused
context with new labels, cache-saturated repeated working sets and shared-context
thread batches. Setup uses Criterion per-iteration batching so contexts do not
accumulate measurements across iterations. Thread lanes include thread creation
and join, not CLI publication. One-operation exploratory observations put cold
context plus Flowchart at about 19–26 ms, new labels in a reused context at about
14–15 ms, and warmed rendering at about 6–7 ms. These are attribution, not timing
admission. OS file caches are not flushed and the process is not cold.

Actual release CLI batch runs rendered eight topology-identical Flowcharts with
different labels into new directories. Three rounds rotated jobs 1/2/4 order,
including process startup, explicit font loading, I/O and transactional publication:

| Mode | Jobs 1, ms | Jobs 2, ms | Jobs 4, ms |
| --- | --- | --- | --- |
| Native | 936.64, 938.06, 963.72 | 892.52, 889.15, 954.27 | 905.28, 908.95, 923.37 |
| Deterministic | 860.70, 884.34, 852.87 | 850.90, 811.14, 853.59 | 2297.10, 814.78, 819.03 |

All runs produced eight SVGs; native SVG hashes were identical across concurrency
levels. The deterministic outlier is retained. Neither this noisy whole-CLI lane
nor the thread lane attributes the full cost to a mutex. CPU/lock-wait profiling
and separate same-capability controls are required before changing concurrency.

## Verification

- Native export: 27 nextest tests passed, including exact zero-cache probe count,
  resource ceilings, cancellation, TTC, fallback and independent advance/bounds.
- Shared text and HTML fallback: 183 tests passed; expanded final-row sequence
  and failure tests were rerun after adding the additional boundary corpus.
- Facade/native output: 20 tests passed across all compiled families, typography
  admission, actual SVG/HTML wrapping and independent painted containment.
- Explicit-font CLI contract suite rerun, including batch failure publication.
- Export all-target Clippy with `-D warnings`, formatting and diff checks passed.
- Correctness subagent found no logic defect; O(rows) temporary payload storage
  and input-size bounds are recorded above. No commit was made.

## Advance Prototype Rejected For Production

`crates/merman-export/examples/native_advance_probe.rs` compares direct single-face
rustybuzz shaping against the current usvg measurement without caching. It runs
four alternating-order rounds, 100 iterations each, using checked-in explicit
fonts. Font loading is outside timing. It is an experiment, not a fast path.

Single-face short examples measured roughly 3–9 us directly versus 36–128 us via
usvg. A long single-face example measured 70–94 us versus 1.24–1.75 ms. However,
`A中V` needs fallback: direct shaping produces missing glyphs and differs by
12.396973 px at 31px size. Long-string advance also differs by 0.019141 px due to
the differing floating-point paths. Matching some mixed-bidi total widths does
not establish glyph/run equivalence.

usvg 0.47 internally handles bidi visual runs, fallback shaping/replacement and
automatic optical sizing. Its shaping functions are not public reusable run APIs.
The prototype omits these semantics, XML whitespace folding, native resource
accounting and cancellation; variable fonts are untested. It must not be selected
by an ASCII blacklist, family allow-list or a single-face assumption.

Run: `cargo run -p merman-export --release --no-default-features --features
native-fonts --example native_advance_probe -- 100`.

## Next Experiments

1. Measure cache saturation with a bounded replacement candidate. Preserve byte
   ceilings and actual-work budget semantics; test thrashing and cross-batch reuse.
2. Attribute lock wait separately from XML/shaping/outline work. Do not move heavy
   work outside the lock without bounding in-flight work and duplicate probes.
3. Investigate a reusable upstream metrics-only shaping API sharing usvg's resolver
   and run semantics. Keep exact usvg probes as the oracle; direct shaping is not
   admitted until fallback, bidi, whitespace, variable fonts and numeric behavior
   are covered.
