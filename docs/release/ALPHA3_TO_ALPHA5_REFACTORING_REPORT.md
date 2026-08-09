# Alpha.3 to Alpha.5 Refactoring Evidence Report

> [!IMPORTANT]
> This is a historical engineering evidence checkpoint, not the rolling alpha.5 upgrade guide.
> Its primary release-range measurements compare alpha.3 with `d2698d0a3`, while explicitly named
> later commits are separate focused receipts. See the
> [Alpha.3 to Alpha.5 upgrade guide](ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md) for audience-specific
> migration steps. Regenerate release measurements against the final tagged commit.

## Scope and verdict

This report's primary revision comparison uses `v0.8.0-alpha.3`
(`56227a541011a3929b808bb3555d67372d630aae`)
with `d2698d0a365b905bb65a58a7690c74075878a4f9`, the alpha.4 candidate measured for this
checkpoint. That candidate's user-visible work, together with later release-only repairs, ships
as alpha.5 after the incomplete alpha.4 publication was withdrawn.
It was measured on an Apple M4 Pro with Rust 1.95.0, Cargo 1.95.0, Node 26.5.0, and
the same local source corpus on 2026-07-27.

The later post-optimization Merman/mmdr evidence is
[`renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md`](../performance/renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md).
The alpha.3 comparisons and Mermaid.js measurements below remain fixed to `d2698d0a3`.
The older mmdr aggregate below predates byte-identical fixture gating and includes different
Treemap and XYChart inputs; retain it as a historical raw run, not a comparable ranking.

The refactor succeeds at changing the cost model: users can now choose explicit capability
leaves and artifact profiles instead of inheriting the historical full stack. The clearest
result is lint/analysis CLI work, whose measured binary falls by 67.95% and whose normal
dependency closure falls by 63.06%.

It is not a universal size or performance reduction. Complete products now carry the Mermaid
11.16 contract and, depending on the surface, both layout engines and math, so their total
artifacts are larger. A pre-release benchmark pass also exposed a recursive clone of the full
effective Mermaid configuration on every default/zero-seed SVG render. Commit `d2698d0a3`
removed that fixed cost without changing hand-drawn seed semantics. After the repair, the
minimal same-capability pipeline has a 1.12x median and 1.07x geometric-mean candidate/alpha.3
ratio across 32 fixtures: 10 are faster, 22 are slower, and seven are within 5%. That is a
substantial recovery, but not a universal native performance win. Remaining family-local
regressions are recorded in
[`PERF_PLAN.md`](../performance/PERF_PLAN.md).

A later focused repair, commit `8d45b8634`, removed duplicate Requirement label measurement
between layout and SVG emission. Three alternating-order runs reduced `requirement_medium` SVG
emission by 60.59% and end-to-end latency by 27.60%. This is a Requirement-family improvement, not
a claim that every diagram became 27.60% faster.

## At a glance

| Surface | Alpha.3 | Alpha.4 candidate | User consequence |
| --- | ---: | ---: | --- |
| Primary SVG admission records | 27 | 35 | Eight additional records across five logical family groups, with source-backed semantic, layout, SVG, and compare evidence. |
| CLI default binary | 32,194,272 bytes | 36,925,360 bytes | 14.70% larger. The product contract also changed, so this is not a same-capability implementation comparison and the size alone does not attribute a cause. |
| CLI default normal dependency closure | 387 package identities | 377 package identities | Ten fewer resolved normal dependencies despite the broader default capability contract. |
| CLI lint/analysis binary | 25,477,648 bytes | 8,166,352 bytes | 67.95% smaller for lint/CI workloads. |
| CLI lint normal dependency closure | 333 package identities | 123 package identities | 210 fewer resolved normal dependencies, or 63.06%. |
| Browser full WASM | 6,911,512 bytes | 12,339,868 bytes | Not like-for-like: the measured alpha.4 full product adds Cytoscape, math, and the expanded Mermaid 11.16 contract. |
| Browser analysis WASM | 1,914,582 bytes | 3,373,026 bytes | 76.18% larger source rebuild; the analysis capability now covers the expanded semantic baseline. |
| Measured complete browser renderer | not comparable | 11,665,436 bytes | `@mermanjs/web-render` removes analysis, editor, and ASCII APIs while retaining SVG, Cytoscape, ELK, and math. |
| Minimal native SVG end-to-end | baseline | median 1.12x alpha.3 | The shared configuration-clone regression is fixed; 10 of 32 fixtures are faster and seven are within 5%, with family-local hotspots still visible. |
| Requirement focused repair | 274.84 us pre-fix | 198.98 us | Operation-scoped label preparation cuts 27.60% from the measured Requirement end-to-end path without changing public layout JSON. |

The CLI sizes are unstripped macOS arm64 Cargo release executables. The WASM values are
`wasm-pack 0.15.0` optimized package artifacts rebuilt from source where historical output was
required, not registry tarballs. Gzip uses `gzip -n -9`; Brotli uses `brotli -q 11 -c`.

## What users gain

### Capability-oriented selection

Alpha.3 exposed broad implementation-era feature names such as `core-full`, `core-host`,
`render`, `raster`, `cytoscape-layout`, `elk-layout`, and `ratex-math`. Alpha.4 exposes
observable leaves instead:

| User-visible capability | Additive leaf | Cost that remains optional |
| --- | --- | --- |
| SVG rendering | `svg` | Analysis, editor, ASCII, exports, layouts, and math are not implied. |
| Mermaid-compatible layouts | `layout-cytoscape`, `layout-elk` | Each engine is independent; selecting one does not require the other. |
| Mathematics | `math` | Plain SVG does not pull the RaTeX and embedded-font closure. |
| Bitmap/PDF export | `png`, `jpeg`, `pdf` | Each format is independent instead of one aggregate pulling every exporter. |
| Diagnostics | `analysis` | Analysis does not imply renderer, layout, math, export, icon, or Markdown code. |
| Editor intelligence | `editor` | The `merman` facade forwards `editor` to `analysis`; lower-level products select the pair explicitly. Neither capability implies SVG or export code. |
| CLI icons and Markdown | `icons`, `markdown`; opt into `network-icons`, `parallel-markdown` | Local icons and serial Markdown do not imply Reqwest/TLS or Rayon. |

The ergonomic Rust facade keeps `default = ["complete-svg"]`. Lower-level crates, bindings,
and transport layers intentionally have empty defaults, so embeddings can state their required
capabilities rather than inheriting a renderer by accident.

These are additive Cargo features. A lean source build must set `default-features = false` (or
use `--no-default-features`) and ensure another dependency does not re-enable a leaf through
feature unification. Only an exact artifact profile can make a release-level absence claim.

The public migration is mechanical where an old aggregate has a direct replacement:

| Alpha.3 feature | Alpha.4 selection |
| --- | --- |
| `render` | `svg` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| `raster` | Select `png`, `jpeg`, and/or `pdf` independently. |
| `core-full` | No direct replacement: core language semantics are invariant; select only the required output capabilities. |
| `core-host` | On lower-level Rust crates and the CLI, select `system-clock`, `system-timezone`, `system-random`, and/or `system-timing` for the host behavior actually required. |
| Separate binding-crate `system-clock`, `system-timezone`, or `system-random` features | Select the atomic `native-runtime` feature on `merman-bindings-core`, `merman-ffi`, `merman-uniffi`, or `merman-android-jni`; partial binding runtime sets are removed. |
| Historical `full`/`tiny` profile aliases | Use an exact artifact profile, or disable Cargo defaults and select observable leaves. |

The old Cargo names have been removed; there are no compatibility aliases. Browser consumers
must also replace historical `@mermanjs/web/<subpath>` or raw `pkg/**` imports with a standalone
public package. There is no subpath or raw-WASM fallback, and the measured `@mermanjs/web-render`
is the complete SVG/layout/math product rather than a name-only replacement for an older basic
render profile.

The binding aggregate does not replace the concrete runtime vocabulary. Runtime catalogs and
generated language projections still expose `system-clock`, `system-timezone`, and
`system-random`; `native-runtime` exists only at artifact assembly time so transports cannot
compile a native runtime policy that is impossible to call successfully.

At the manifest level, `merman` moved raster implementation dependencies behind
`merman-export`: its direct dependency count changed from 11 to 9 while its public capability
vocabulary changed from 9 features to 16. `merman-cli` still has 10 direct declarations, but
four formerly unconditional dependencies are now optional: analysis, Rayon, Reqwest, and shell
completion support.

### Diagram coverage and parity

The authoritative admission inventory, rather than the older human status table, changed from
27 to 35 primary SVG records:

| New primary admission or promotion | User-visible effect |
| --- | --- |
| `error` | Error diagrams moved from parse-only tracking to primary SVG admission. |
| `swimlane` | Swimlane diagrams have typed layout and SVG evidence. |
| `railroad`, `railroadEbnf`, `railroadAbnf`, `railroadPeg` | Railroad syntax and the three grammar dialects are independently admitted. |
| `wardley` | Wardley mapping joins the source-backed primary matrix. |
| `cynefin` | Cynefin joins the source-backed primary matrix. |

`zenuml` remains compatibility-only because its upstream renderer is an external browser plugin;
it must not be described as an upstream SVG parity claim.

## Size and dependency evidence

### CLI profiles

| Measured build | Alpha.3 | Alpha.4 candidate | Interpretation |
| --- | ---: | ---: | --- |
| `cargo build --release -p merman-cli` | 32,194,272 | 36,925,360 | Default product grows 4,731,088 bytes while its capability contract also changes; this measurement does not isolate which additions account for those bytes. |
| Unique normal `cargo tree` identities for default | 387 | 377 | The resolved package count falls by ten even though the linked capability set grows. |
| Lean lint build | 25,477,648 | 8,166,352 | Alpha.3 `--no-default-features` still carried historical renderer/export/tool dependencies; the alpha.4 candidate's `--no-default-features --features analysis` build does not. |
| Unique normal `cargo tree` identities for lean lint | 333 | 123 | Measured from `cargo tree --locked --edges normal --prefix none --format '{p}' | sort -u`. |

The complete CLI is the right choice for `mmdc`-style conversion, export, icons, and Markdown
work. The analysis profile is the right choice for a CI lint gate, where the old implicit full
closure was pure cost.

### Browser WASM profiles

| Artifact | Raw bytes | Gzip bytes | Brotli bytes | Contract |
| --- | ---: | ---: | ---: | --- |
| Alpha.3 `browser-full` | 6,911,512 | 2,641,193 | 1,946,384 | Historical full package; lacks the alpha.4 complete SVG contract. |
| Alpha.4 `@mermanjs/web` | 12,339,868 | 4,648,558 | 3,359,651 | Analysis, ASCII, editor, SVG, Cytoscape, ELK, and math. |
| Alpha.3 analysis | 1,914,582 | 718,044 | 546,783 | Historical semantic-analysis profile. |
| Alpha.4 `@mermanjs/web-analysis` | 3,373,026 | 1,270,087 | 970,361 | Browser diagnostics and semantic analysis only. |
| Alpha.4 `@mermanjs/web-render` | 11,665,436 | 4,385,258 | 3,180,859 | Complete public SVG renderer: SVG plus Cytoscape, ELK, and math. |
| Alpha.4 `@mermanjs/web-editor` | 3,519,126 | 1,327,314 | 1,010,768 | Parser-backed editor intelligence plus analysis. |
| Alpha.4 `@mermanjs/web-ascii` | 3,518,439 | 1,332,638 | 1,019,437 | ASCII output only. |

The measured renderer saves 674,432 raw bytes (5.47%) and 178,792 Brotli bytes (5.32%) against
the measured full product while retaining the complete SVG contract. This is a useful split for viewer-only
browser applications, but it is intentionally not presented as a dramatic size win.

The alpha.3-to-alpha.4 full and analysis rows are valuable cost evidence, not a like-for-like
artifact-size regression gate. Mermaid 11.16 coverage and the alpha.4 capability contracts changed
what those packages promise.

## Performance evidence

### Historical Merman, Mermaid.js, and mermaid-rs-renderer checkpoint

The detailed generated checkpoint is
[`renderer_comparison_2026-07-27.md`](../performance/renderer_comparison_2026-07-27.md).

The checked-in comparison harness ran the `standard` suite: 34 end-to-end fixtures across 24
families, 20 Criterion samples, one-second warm-up, and one-second measurement windows. It used
Merman at `d2698d0a3`, `mermaid-rs-renderer` at `7ff1196`, and Mermaid.js 11.16.0 in one warm
Headless Chromium 131 process.

| Runner | Requested | Measured | Missing | Result |
| --- | ---: | ---: | ---: | --- |
| Merman native | 34 | 34 | 0 | Complete suite coverage. |
| Mermaid.js browser | 34 | 34 | 0 | Complete suite coverage. |
| mermaid-rs-renderer native | 34 | 32 | 2 | Missing `flowchart_large` and `info_medium`. |

On this host, Merman's median `Merman / Mermaid.js` warm end-to-end ratio was 0.0237 across all
34 rows: Mermaid.js's median latency was approximately 42.2x Merman's. This is a native Rust
pipeline compared with a warm browser renderer, not an intrinsic language benchmark or a
browser-WASM claim.

The original availability-only aggregation reported 32 `mermaid-rs-renderer` rows, with Merman
faster on 19 and slower on 13 and a median ratio of 0.697. Subsequent fixture hashing found that
Treemap and XYChart used different same-named inputs, so this aggregate is retained only to explain
the historical checkpoint. Use the post-optimization report for comparable mmdr conclusions. The
raw result was workload-dependent:

| Fixture | Merman | mermaid-rs-renderer | Mermaid.js |
| --- | ---: | ---: | ---: |
| `flowchart_medium` | 3.68 ms | 105.62 ms | 57.65 ms |
| `flowchart_ports_heavy` | 1.13 ms | 1.28 s | 32.30 ms |
| `class_medium` | 949.54 us | 2.38 ms | 44.70 ms |
| `mindmap_medium` | 685.48 us | 76.79 us | 15.40 ms |
| `kanban_medium` | 153.33 us | 29.38 us | 6.20 ms |

The raw timings show that Merman is particularly strong on the measured complex Flowchart cases,
while `mermaid-rs-renderer` was faster on Mindmap and Kanban at this revision. The harness confirms
successful execution, but this historical version did not prove aligned fixture bytes, DOM output,
or Mermaid-semantic equivalence for `mermaid-rs-renderer`. These timings are not a quality-adjusted
winner. That distinction matters more than a single geometric mean.

The post-Requirement long run provides a later native reference. It uses 30 samples, a
two-second warm-up, three-second measurement windows, and byte-identical input gating:

| Later native reference | Result |
| --- | ---: |
| Byte-identical, jointly measured rows | 30 |
| Merman faster / slower | 18 / 12 |
| Median Merman / mmdr ratio | 0.664x |
| Geometric-mean ratio | 0.297x |
| Rows above both 1.10x and 50 us | Requirement and Mindmap |

Requirement's later full standard row measures 196.96 us versus mmdr's 71.08 us. In the separate
paired A/B lane, the repair reduced Merman from 274.84 us to 198.98 us.
Mindmap measures 165.26 us versus 74.06 us, but Merman runs COSE-Bilkent while mmdr falls back to
radial placement. Complex Flowchart cases remain Merman's strongest measured scaling result. None
of these ratios prove equivalent layouts, DOM, sanitization, or Mermaid release parity.

### Alpha.3 to alpha.4 native pipeline

The 34 `standard` fixture files are byte-for-byte unchanged across the range. The comparison used
two lanes so product-default cost is not confused with an implementation-only delta:

| Revision A/B lane | Shared rows | Median candidate / alpha.3 | Geometric mean | Candidate faster / slower |
| --- | ---: | ---: | ---: | ---: |
| Revision-complete SVG product | 34 | 1.10x | 1.09x | 13 / 21 |
| Minimal same-capability SVG | 32 | 1.12x | 1.07x | 10 / 22 |

The complete lane enables each revision's documented full SVG/layout/math product and therefore
includes deliberate capability changes. The minimal lane disables defaults and selects only the
smallest equivalent SVG leaf. Mindmap and Architecture require optional layouts and are
unavailable in both minimal builds, leaving 32 shared rows. The minimal lane used:

```console
cargo bench --locked -p merman --no-default-features \
  --features <render-or-svg> --bench pipeline -- \
  --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

| Metric | Result |
| --- | --- |
| Shared minimal-SVG rows | 32 |
| Rows within 5% | 7 |

The remaining delta is not uniform. State medium improved from 1.12 ms to 501.4 us end-to-end,
while large and medium Flowchart cases remain within about 1.7%. The largest minimal-lane
regressions are Kanban (4.08x), Requirement (2.25x), Info (1.99x), Packet (1.93x), and Radar
(1.31x). The complete-product lane additionally exposes a 4.78x Mindmap regression.

An additional focused run used 30 samples, a one-second warm-up, and two-second measurement
windows to verify the repaired small-diagram path:

| Stage | Info medium: alpha.3 -> candidate | Packet medium: alpha.3 -> candidate |
| --- | ---: | ---: |
| SVG emit | 1.68 us -> 2.37 us (1.41x) | 2.28 us -> 3.17 us (1.39x) |
| End-to-end | 2.45 us -> 4.84 us (1.97x) | 3.60 us -> 6.87 us (1.91x) |

Criterion's `iter_batched` excludes the `family::prepare` setup from the timed render routine.
The emit measurements therefore isolate SVG emission/finalization. Packet output is
byte-identical between revisions; Info differs only by the `v11.15.0` to `v11.16.0` text.

#### Fixed Info and Packet root cause

The pre-fix shared SVG dispatch called `SvgExecution::effective_config` before selecting a
diagram family. Mermaid's generated default configuration sets `handDrawnSeed` to `0`, which
means "derive a seed for this operation." Resolving that sentinel cloned the full top-level
`serde_json::Map`, inserted the derived seed, and rebuilt a `MermaidConfig`. The measured default
config is 15,409 bytes, with 49 top-level keys and 461 scalar values. Info and Packet do not
consume the seed, but every default render paid for the recursive clone and its drop.

The path was introduced in commit `84477e467` (`refactor(render): own operation render
environment`). Before committing the repair, a same-worktree A/B based on the unfixed parent
`1bd6f9b90` and the eventual `d2698d0a3` patch used 30 samples, a one-second warm-up, and
two-second measurement windows:

```console
cargo bench --locked -p merman --no-default-features --features svg --bench pipeline -- \
  'render/(info_medium|packet_medium)|end_to_end/(info_medium|packet_medium)' \
  --noplot --sample-size 30 --warm-up-time 1 --measurement-time 2
```

| Benchmark | Unfixed candidate | Formal fix | Improvement |
| --- | ---: | ---: | ---: |
| `render/info_medium` | 45.28 us | 2.36 us | 19.2x lower latency |
| `render/packet_medium` | 45.48 us | 3.01 us | 15.1x lower latency |
| `end_to_end/info_medium` | 47.38 us | 4.26 us | 11.1x lower latency |
| `end_to_end/packet_medium` | 49.60 us | 6.22 us | 8.0x lower latency |

Commit `d2698d0a3` makes shared dispatch borrow the effective configuration directly and resolves
only the `0`/`-0` sentinel inside `SvgExecution::rough_randomness`. Explicit non-zero negative,
fractional, and large-number JavaScript seed semantics remain unchanged, as does the
domain-separated operation stream. Every production RoughJS/hand-drawn randomness consumer uses
that central method, so the repair needs no diagram-family allow-list.

Focused validation after the change covered:

- `cargo fmt --all -- --check`.
- `merman-render` library tests: 796 passed.
- Hand-drawn seed, State, Ishikawa, and Venn integration tests: 32 passed.
- Architecture integration tests with Cytoscape: 12 passed and one skipped.
- Runtime determinism tests: two passed.

The full workspace `nextest`, SVG DOM parity, and complete golden gates were not rerun for this
focused repair. The final clean alpha.3 comparison above shows that the catastrophic fixed cost
is gone; the remaining roughly 1.9x end-to-end delta on these ultra-small fixtures is a separate,
much smaller profiling target.

#### Fixed Requirement duplicate label work

Requirement layout measured each node and edge label to size Dugong nodes, then SVG rendering
rebuilt the label plan and measured the same text again. Commit `8d45b8634` replaces that ownership
gap with a private operation-scoped prepared artifact. It carries exact metrics and label identity
from layout to render while keeping Markdown conversion and strict sanitization in the render
phase. There is no global cache, family allow-list, or syntax heuristic, and the public layout JSON
still projects the original `RequirementDiagramLayout`.

Three same-host long runs alternated base/head order. Each used 30 Criterion samples, two seconds
of warm-up, and three-second measurement windows:

| Stage | Before median | After median | Change |
| --- | ---: | ---: | ---: |
| Parse | 5.528 us | 5.613 us | +1.54% |
| Layout | 132.05 us | 131.56 us | -0.37% |
| SVG emission | 137.43 us | 54.17 us | -60.59% |
| End-to-end | 274.84 us | 198.98 us | -27.60% |

The stage comparison at `75c9fd156` against the same `mermaid-rs-renderer` revision measures
0.72x parse, 2.81x layout, 3.85x SVG emission, and 2.81x end-to-end. This closes the
duplicate-measurement cause
but not the whole family gap: Dugong layout and SVG DOM construction remain separate profiling
targets. The clean isolated worktree passed all 1,148 `merman-render` tests, with one existing
skip, plus the focused Requirement and Look SVG selections.

## Node candidate evidence

`@mermanjs/node` remains a private, inconclusive candidate. It is not a release recommendation
yet. The schema-2 comparison binds both candidates to the same source, lockfile, binding contract,
trusted 4,001-case corpus, and raw artifacts. It was measured on one macOS arm64 host, so it does
not satisfy the all-target admission contract.

| Candidate, macOS arm64 | Node-WASM | N-API |
| --- | ---: | ---: |
| Runtime artifact | 16,881,944-byte WASM, plus 8,070 bytes of JS/manifest | 21,223,312-byte `.node` |
| Packed / installed | 6,157,111 / 17,784,897 bytes | 8,966,029 / 22,906,407 bytes |
| Warm successful-SVG p50 | 0.3189 ms | 0.2903 ms |
| Warm successful-SVG p95 | 1.6305 ms | 1.3467 ms |
| Cold parent-to-result p50 | 137.74 ms | 47.39 ms |
| Engine-init-through-first-SVG p50 | 96.05 ms | 7.39 ms |
| Peak RSS | 638,189,568 bytes | 240,648,192 bytes |
| Four-request concurrent batch p50 | 1.4387 ms | 0.2418 ms |

N-API lowers warm p50 by 9.0%, cold outer p50 by 65.6%, first-SVG operation p50 by 92.3%, and peak
RSS by 62.3% on this host. Its installed footprint is 28.8% larger than Node-WASM. All 4,001
semantic/typed-error outcomes and SVG structure signatures match; 426 exact geometry and raw-byte
digests differ. The five concurrency batches are directional evidence, not a stable throughput
gate.

The warm boundary includes the public facade call plus SHA-256 and byte-length evidence projection;
cold and concurrent boundaries stop before that projection. These are harness-level product
operation timings, not isolated renderer CPU measurements. The transport remains unselected because
the declared target matrix is incomplete; the 426 differences are unattributed report residuals,
not a semantic/structure failure or an alpha.4 admission gate.

A separate synchronous N-API run compared the private Merman candidate directly with published
`@xingwangzhe/satteri-mermaid@0.7.1`, which wraps mmdr 0.3.1. Across 30 shared source arguments that
both rendered successfully, Merman was faster on 13 fixtures and Satteri on 17. Satteri's public
wrapper trimmed one trailing LF from each input; the benchmark did not bypass that product
behavior. The median fixture ratio was 1.247x Merman/Satteri, while the geometric mean was 0.458x
because Merman's large Flowchart wins dominate the aggregate. Requirement, Mindmap, C4, Sequence
tiny, Kanban, and Class tiny were the material Satteri leads. The separate transport control does
not show a broad local latency penalty for Merman's N-API candidate, but it also changes target and
runtime. It does not isolate renderer/layout cost from Merman's facade, marshalling, binding,
allocation, build-profile, or output differences. In the direct comparison, all 30 raw, structure,
and geometry digests differed.

The native size difference remains real. Satteri's macOS arm64 addon is 4,206,544 bytes; the measured
source-bound transport artifact is 21,223,312 bytes. A separate `e311f9e6a` size-control experiment
measured a 21,256,336-byte complete baseline and a 15,771,392-byte SVG-only lane, with Cytoscape,
ELK, and math adding 413,392, 1,257,136, and 3,863,920 bytes within Merman. In that same experiment,
`lto = true` plus `codegen-units = 1` reduced the complete candidate to 18,998,416 bytes. Adding
Cargo `strip = "symbols"` after the existing napi CLI `--strip` saved no Darwin arm64 raw bytes,
but changed the binary and increased gzip output by 2,735 bytes; it is not a cross-target policy
conclusion.

The detailed protocol, hashes, capability boundary, and package findings are recorded in the
[Node transport admission](../performance/NODE_TRANSPORT_ADMISSION.md),
[Satteri Node comparison](../research/2026-07-22-satteri-mermaid-npm-integration.md), and
[article-claim audit](../research/2026-07-27-satteri-web-wasm-claim-audit.md). The Satteri lane
embeds mmdr 0.3.1; the native Criterion lane pins later revision `7ff1196`, so their aggregates are
separate evidence.

A separate `semantic-json` probe gives a cleaner parse-only comparison:

| Parse-only measurement | Node-WASM | N-API |
| --- | ---: | ---: |
| Shared accepted cases | 3,902 | 3,902 |
| Shared rejected cases | 99 | 99 |
| Canonical semantic JSON matches | 3,902 / 3,902 | 3,902 / 3,902 |
| Warm p50 | 0.2058 ms | 0.1924 ms |
| Warm p95 | 0.4770 ms | 0.4515 ms |
| Throughput | 5,260 cases/s | 5,430 cases/s |

Raw semantic JSON has nondeterministic object-key order across transports. After recursively
sorting object keys, every shared successful result matches; raw byte comparison must not be used
as semantic parity evidence.

## Scenario guide

| User workflow | Recommended surface | Selection | Why |
| --- | --- | --- | --- |
| Static Rust documentation or a blog generator | `merman` | Default `complete-svg`, or `default-features = false` with `svg,layout-cytoscape,layout-elk,math` | Complete deterministic SVG without CLI/tooling dependencies. |
| CLI conversion and broad export | `merman-cli` | Release/default artifact profile | SVG, ASCII, PNG, JPEG, PDF, icons, Markdown, and math are available. |
| CLI lint or CI | `merman-cli` | `--no-default-features --features analysis` | Measured lean executable with no renderer, exporter, icon, network, or Markdown closure. |
| Embedded lint library | `merman-analysis` | Depend on the crate directly | Analysis APIs do not require a nonexistent `analysis` feature on this crate. |
| Browser diagram viewer | `@mermanjs/web-render` | Standalone render package | Complete SVG layout and math without analysis/editor/ASCII APIs. |
| Browser diagnostics or lint | `@mermanjs/web-analysis` | Standalone analysis package | Detection, validation, facts, and diagnostics without rendering or editor APIs. |
| Browser editor | `@mermanjs/web-editor`, or `@mermanjs/web` when one realm also renders | The editor package contains analysis plus parser-backed editor intelligence | Use `web-editor` for a dedicated Worker; do not describe it as editor-only or combine full with duplicate slim packages. |
| Native editor integration | `merman` | `default-features = false, features = ["analysis", "editor"]` | Parser-backed diagnostics and editor facts without linking SVG/export code. |
| LSP process | `merman-lsp` | Published binary after release, or `--no-default-features --features stdio` | The LSP executable requires the `stdio` leaf; use artifact profile `lsp-stdio-release` for the exact release contract. |
| Markdown or MDX conversion | `merman-cli` | `--no-default-features --features markdown`; select `parallel-markdown` instead only after throughput measurement | `markdown` already implies SVG; avoid Rayon unless batch conversion needs it. |
| Terminal preview | `merman` or `merman-cli` | Disable defaults and select `ascii`; query `ascii_capabilities` | No SVG backend is required, but only 14 families are admitted and support is graded Full, Partial, or Summary. SVG admission does not imply ASCII support. |
| Node SSR or a Node static-site generator | CLI subprocess | Do not depend on private `@mermanjs/node` yet | There is no admitted in-process Node package; the candidate still lacks reproducible all-target admission. |
| Typst | `@preview/merman:0.2.0` | Published Typst package on its independent version track | The published package embeds Merman alpha.3 and its `typst-wasm` profile has no math; it is not an alpha.4 artifact. |
| Python or Flutter embedding after alpha.4 publishes | `merman` on PyPI or pub.dev | Planned full ABI 3 package; verify the installed version before use | The target contract has no slim prebuilt SKU; a declared channel is not proof that this candidate is already live. |
| Android or Apple embedding after alpha.4 publishes | GitHub Release AAR or XCFramework | Planned full ABI 3 artifact-only output | Maven Central and a remote SwiftPM binary are not published channels; verify the release asset version. |
| C ABI embedding after alpha.4 publishes | `merman-ffi` from crates.io | Build the source crate and verify its version | Its artifact profile proves reproducible reference libraries; it is not a downloadable prebuilt SDK. |

## Evidence status and release refresh

This report freezes named historical checkpoints. The rolling performance owner is
[`PERF_PLAN.md`](../performance/PERF_PLAN.md), which records completed Requirement, Mindmap, and
Kanban work as well as rejected hypotheses and the remaining Flowchart and comparison work.

Before the final alpha.4 release report:

1. rerun the complete-product and minimal same-capability alpha.3 A/B lanes against the final
   release commit, including Class, Sequence, Requirement, and Mindmap attribution;
2. refresh artifact-size evidence and rerun the current artifact-profile dependency claims;
3. attach the exact host, toolchain, recipe, fixture, and source-commit ledger; and
4. keep Node and browser-WASM throughput claims explicitly unproven until their own admission
   evidence exists.

## Changelog extraction

- Added eight primary SVG admission records across five logical groups: Error, Swimlane,
  Railroad dialects, Wardley, and Cynefin on the Mermaid 11.16 baseline.
- Replaced broad feature aggregates with explicit capability leaves for rendering, layouts, math,
  exports, diagnostics, editor support, icons, and Markdown.
- Added separate browser packages for complete rendering, analysis, editor intelligence, and
  ASCII output.
- Reduced the measured lint/analysis CLI binary by 67.95% and its normal dependency closure by
  63.06% compared with alpha.3's historical lean build.
- Added capability discovery and artifact-profile contracts for safer native and browser embedding.
- Removed the default/zero-seed full-configuration clone while preserving deterministic
  hand-drawn seeds; see this report for the final native SVG comparison and private Node
  candidate limitations.
- Reused operation-scoped Requirement label measurements between layout and SVG emission, reducing
  the measured medium Requirement path by 27.60% without changing its public layout JSON.
