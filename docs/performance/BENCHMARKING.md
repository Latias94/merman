# Benchmarking

This document defines the measurement contract for `merman`. Use
`docs/performance/RUNBOOK.md` for the operational sequence and
`docs/performance/PERF_PLAN.md` for the active optimization queue. The [renderer benchmark reports](reports/README.md) contain dated results for readers.

Performance evidence never overrides semantic parity, deterministic output, resource limits, or
security behavior. A faster result with a different public operation, input, output contract, or
error contract is not an accepted optimization.

## Measurement lanes

Keep these lanes separate because they answer different questions:

- Native Criterion `pipeline` benchmarks measure Merman stages and public end-to-end operations.
- `bench.py ab` compares two Merman revisions on one host. Its confirmation mode is the
  decision-grade regression and candidate-admission lane.
- Native stress and stage observations attribute work after a public-operation change has been
  observed. They do not independently admit production code.
- `bench.py node` and the browser harness provide cross-renderer context. Different
  transports, algorithms, capabilities, fixture bytes, or output-quality contracts must not be
  combined into one aggregate admission number.
- Native allocation, peak-live-heap, Node RSS, and artifact-size evidence use their own harnesses.
  Do not infer memory or size behavior from latency alone.

## Ordered public workloads

Use `tools/bench/bench.py` to freeze one ordered plan for Rust, Node and CLI runs.
`tools/bench/workloads.json` refers to existing `corpus.json` fixture IDs, adds small config
controls, and preserves each occurrence and SVG ID. The full suite includes the five diagrams
from Merman's issues #183–#187 guide in document order. Standalone public calls, repeated identical
source, alternating families, repeated versus unique Sequence message labels, and retained
artifacts exercise different behavior. Keep per-scenario results alongside coverage.

| Operation | Observation boundary |
| --- | --- |
| Rust `svg-data`, `svg`, `layout-json` | Public bytes-only SVG, SVG with evidence, or compact geometry written to a `Vec<u8>`. Requests in a batch use one renderer. |
| Rust `static-svg`, `resvg-svg` | Complete static SVG or static resvg-compatible SVG, including pipeline processing. |
| Rust `prepare-drop`, `prepare-retained` | Prepare a presentation per source. Drop each result immediately or release all results after the batch checkpoint. Untimed verification checks family, title, source and effective configuration, and explicit paths. |
| Rust layout work | Compare checked geometry bytes at default and elevated layout budgets. Record the smallest accepted budget and the adjacent rejection. A `maxEdges` parser rejection has its own status. |
| Node | Merman, Pele and Mermaid v13 in the same Node consumer. Each scenario starts a process, imports the package and renders distinct IDs sequentially. A v13 request ends when `mermaid.render(id, source)` resolves. Reports separate initialization, first render and warm batches. |
| CLI | The chosen binary reads and publishes one file per fresh process. A batch is the sequential execution of those processes. SVG and PNG have separate results. |

Each Rust clock or memory sample starts a fresh process, constructs the renderer, then runs the
specified number of warmup passes before observing one complete batch. `--warmups 0` captures
first use on a constructed renderer. CLI timings include process startup. The memory executable
uses the existing counting allocator and reports allocation traffic, live bytes at the
measurement start and peak, signed growth at the batch boundary and growth after releasing
results. Renderer construction, plan decoding, hashes, semantic checks and report formatting
are outside the observed interval.
The clock executable uses the ordinary allocator. Its raw samples and median deltas are diagnostic.

For cross-renderer comparisons, report completion first. The common plan freezes the UTF-8
sources, their SHA-256 digests and the request order. Scenarios can declare text expected in the
rendered SVG. Output bytes, PNG dimensions and artifact checks describe the
delivered product. Visual equivalence still requires the family-owned DOM or raster checks.
The Rust, Node and CLI workloads use the same Python SVG and label reader. In ordinary Rust
batches the measured calls report their output sizes. Independent processes verify the outputs
before and after sampling. A matching comparison reports `matching-verification-outputs` and
`measured_output_check: size-only`. For a one-request SVG clock run, `--verify-measured-svg`
stops the clock when the public call returns and verifies that same SVG against both witnesses.
The report records a digest per sample and reserves `same-output` for that comparison. Dropping
the returned string happens after this clock boundary. The batch clock includes each SVG drop.
The two measurements have separate comparison options. The Node worker retains its outputs through
the last timed batch and then checks each distinct output against the request's validation policy.
The worker timeout covers the complete scenario, including these checks.

When a Rust SVG scenario declares drawing facts, `--check-drawings` runs the shared browser
observer after sampling and binds the result to the verified SVG digest. Comparisons withhold
deltas for that scenario until both sets of facts pass in matching observer environments. Raw
metrics remain available. Historical reports that omit source expectations carry the status
`source-outcomes-unverified`. A changed measured digest or size records a failed sample in the
report, continues with later scenarios and gives a failing command exit status.

Each frozen request has an `expected_outcome`. Older plans without the field use `draw`.

| Source expectation | Evidence used by the comparison |
| --- | --- |
| `draw` | A drawable SVG or a decoded PNG with visible variation against white, plus any declared labels or drawing checks. An explicitly zero-sized SVG root fails the mechanical check. |
| `allow-empty` | A valid artifact may be empty. External reports identify the observed class so two valid outputs with different content classes can both pass coverage and have their speed ratio withheld. |
| `reject` | A recognized parser diagnostic counts as an expected rejection. It has its own coverage count and supplies no render time to a speed ratio. |
| `unreviewed` | The collector records the observed output or error. Its timing stays outside the comparable render groups until the source has been reviewed. |

An imported source with reviewed labels, drawing facts, or an explicit expectation carries that
review in the plan. Other new imports are `unreviewed`. For PNG, the CLI checks the decoded pixels
of the delivered file after timing. It also checks the auxiliary SVG for the source's required
labels. Pixel variation is evidence of content. Geometry, label placement and shape ownership
have their own drawing checks. A PNG above 64 MiB encoded or 16,777,216 pixels is recorded as
`observer-limit` and its timing is excluded. An interrupted Node scenario records its worker failure and its
unmeasured requests. The matrix continues with a new process for the next scenario and compares
the observed common requests. On POSIX the collector waits for the interrupted worker and its SVG
validator to exit before starting the next scenario. The process exit status and bundle retain the
collection failure.
Keep the browser comparison in its existing Chromium harness. The [runbook](RUNBOOK.md#run-an-ordered-workload)
includes reproduction commands and external corpus imports.

### Maintaining the benchmark suite

| Comparison | Reference | Purpose |
| --- | --- | --- |
| Node package to SVG | Pele and the locked Mermaid v13 Node export | Headless public API latency and coverage on the same ordered requests. The consumer lock pins each release, including prereleases. |
| Fresh process to file | `mmdc` via `bench.py cli` | What a CLI consumer pays for SVG or PNG, including startup. |
| Browser to SVG | The pinned Mermaid.js build and Merman WASM in the playground | Browser initialization, drawing and family-specific visual evidence. |

Add a case for a concrete workload or failure mechanism. Reuse its source from `corpus.json` or
an existing project fixture, record its origin, and add the expected labels to `workloads.json`.
For a drawing with no text to assert, an explicit empty label list records mechanical coverage.
Use consecutive occurrences when order, reuse or retention matters. Keep a nearby control when
changing a variable such as layout, label content or input size. `quick` covers frequent local
checks. `full` adds family, downstream, configuration and scaling cases. An ordered document
import preserves the weight of repeated examples. The coverage inventory counts both occurrences
and distinct source digests and lists the layout-budget topology population separately.
Within a frozen scenario, the request's position identifies the occurrence. The SVG `diagram_id`
can repeat across editor revisions. `editor-fixed-id` and `editor-rotating-id` cover a valid edit,
an incomplete edit and its repair through one engine per Node scenario.

`full` includes the author order of the Mermaid examples and C4 pages at the revision named in
their manifests. The generated examples page repeats each diagram in adjacent fences. Its
manifest is opt-in so a generated duplicate does not add weight to the default author population.
The work runner also calibrates paired five-node strongly connected components and a DAG formed
by reversing each cycle's closing edge. The pair preserves every label and undirected connection.
The default work report records both geometry and public SVG completion. High-budget evidence
includes both output identities and the work consumed by SVG. The layout writer also checks
the exact minimum passing budget and the adjacent failure. `topology-orders` freezes the nine
statement orders of each 75- and 100-node graph. They preserve node names, directed edges,
labels and input byte count. Check output stability for repeated runs of the same source.
Different statement orders can legitimately produce different layouts.

The `families` suite follows the same `family-baseline` entries and fixture files as the browser
playground. `full` includes those one-request scenarios as well as ordered documents and scale
controls. `coverage.json` gives each product's completed and byte-repeatable family counts and
names missing, unstable and unreported baselines. Each count describes the selected example at
that product's installed features and defaults. The `info` and numeric-only XY examples carry
their reason for using mechanical artifact checks in the frozen plan.

Every entry in `expected_labels` is required. A string matches a complete text spelling. A nested
list accepts one of the listed complete spellings for that fact. For example, the Requirement
fixture accepts both `A requirement` and `Text: A requirement` because renderers place the field
name and value in different text elements. Keep alternatives tied to the authored value and
retain a negative check that removes or changes that value.

The native Criterion `pipeline` takes fixture registration from `tools/bench/corpus.json`.
After editing that catalog, run `python tools/bench/bench.py sync-fixtures --write` to refresh the
package-local Rust inventory and any mirrored external fixture bytes. The benchmark embeds the
sources at compile time so a copied executable retains its inputs. Running the same command
without `--write` checks the generated files. The `corpus-<suite>` workload selector reads this
same catalog for public API runs. Stage timers, stress curves and native memory contracts retain
their own observation boundaries.

To register another Node renderer, add its package and public API to `node_products` in
`tools/bench/workloads.json`,
pin the consumer dependency in `tools/bench/node-reference`, then implement its call in the
existing adapter. Exercise a successful drawing, a failed call and repeated IDs across batches.
Use its real asynchronous completion boundary when applicable. Run the installed product on the
same frozen plan as its peers. Version upgrades get a fresh result bundle and preserve the old lock
and raw reports with the measurement. Use a separate experiment if a product needs a different DSL,
output format, font policy or process lifecycle.

External reports include product versions, collector fingerprints and per-request results.
`artifact_identity` describes the measured files for each arm. CLI collectors fingerprint the
executable and any JavaScript entry in the resolved command. Node collectors fingerprint the
public entry plus the Merman native addon or WASM glue and module observed by the worker.
For JavaScript peers the scope is the public package entry, with the dependency lock in the
bundle. An unknown or missing product fingerprint leaves raw observations available and
withholds the product ratio.
For `mmdc`, a configured browser file is recorded separately from the command. The collector
reports the actual browser executable as unobserved.

For a Merman Node `--variant-layout`, collection and report replay use `node_fixed_layouts` in
`workloads.json` to check the SVG plan's exact `layout-*` capability list. Intrinsic families
report none. For Dagre or ELK site choices, Architecture requires `layout-cytoscape`, and Block
and C4 report none. All three require `layout-shoal` in the Shoal variant. Swimlane can use the
Flowchart renderer under a site layout and is not exempt. A selectable family's fallback does
not prove that the requested backend ran.

Read coverage before interpreting a time. A matching text assertion establishes presence in the
SVG text representation. Clipping, overlap and shaping require the visual checks. Complete-batch
ratios use the same successful, repeatable population on both sides. When a report comes from a
different collector, both observed time series remain available and the ratio is withheld.
The default Node matrix measures one fresh process per product and scenario. For variation
between processes, `--process-rounds` repeats the collection with an even number of rounds.
Each consecutive pair of rounds reverses product order. Each independent observation is the
median of one worker's warm batches. The summary reports the distribution of the within-round
peer/Merman ratios, the counts on each side of one and the number of eligible rounds. A missing
pair or a changed product or output withholds the aggregate for that scenario.
Use the calibrated `bench.py ab` protocol for release claims about a Merman revision.

### SVG drawing checks

A fixture or document occurrence in `workloads.json` can declare `drawing_facts`. Each fact names
an observable property of the source. The current cases include the loop-back arrow from issue
#183, the two sibling groups and their connecting edge from #186, and the branch-name label from
Zed #56767, and the Sequence loop's repeated message labels. Choose the type of evidence that
would change if the drawing were wrong.

| Fact | Chromium observation |
| --- | --- |
| `arrows` | The edge ID identifies the route. Its visible triangular arrowhead must face out of the route at the destination. The source and destination must be the unique nearest node outlines, within one rendered arrowhead of the respective endpoints. Both referenced SVG markers and inline triangular heads are supported. |
| `groups` | The node's outline bounds must fit inside the visible rectangular cluster border and remain clear of the other sibling borders named in the same fact set. |
| `label_widths` | The named rectangular node must contain the expected text. Chromium text ranges must fit horizontally within the node and a clipping `foreignObject` viewport. Signed left and right clearances are in CSS pixels. The report records the computed CSS font-family lists and observer environment. Physical font selection depends on the host. |
| `sequence_messages` | In a Sequence diagram, count visible message labels with exactly the declared text. The loop workload exercises the same message at 1, 32 and 256 occurrences. Removing an occurrence fails the check even when other copies of its text survive. |

The Node collector can save the actual SVG from the last warm batch after its clock stops. The SVG
CLI already retains each rendered file. The observer verifies repeated output identity, the raw
file size and its SHA-256 before inspecting the bytes in a sandboxed, offline Chromium frame.
Missing artifacts, unstable draws and unsupported representations are reported as `unavailable`.
Definite geometry mismatches are `failed`. A scenario with declared facts publishes a timing ratio
only when both products pass every declared fact in that scenario. Raw milliseconds and artifact
completion counts remain available. PNG pixel checks, arbitrary SVG paint and vertical glyph
clipping require their own evidence.

For an `unstable-output` investigation, retain both original files and their hashes. Compare
their DOM changes, then replay the files in one offline browser at a fixed viewport, font setup
and device scale. Compare decoded RGBA pixels, include a same-file repeat and a visibly changed
control, and record the browser and scale with the result. A matching raster at the sampled sizes
can explain a byte difference. The raw SVG repeatability result remains part of the benchmark.

## Native stage attribution

Use the instrumented workload executable when a public-operation observation needs attribution.

```console
python tools/bench/bench.py rust --plan target/bench/plan.json --metric stages --site-layout shoal --out target/bench/stages.json --chrome-trace target/bench/stages-trace.json
```

`stages` takes the same ordered input, capability and SVG verification path as `clock`. It builds
with `benchmark-stages` and times stage scopes exclusively. Each report includes `end_to_end` and
`unattributed_ns`, the difference between the batch duration and the sum of instrumented scopes.
`layout_prepare` covers the renderer's family adapters, including input and output projection.
`layout_solve` covers instrumented geometry builders and solver calls. Theme preparation includes
Mermaid theme materialization and the Flowchart family theme binding. `measure` covers routed
measurement and built-in streaming label widths. `css_emit` covers streamed style output and the
XML serialization of buffered style blocks. A stage with no captured work reports zero calls.
The stage matrix includes the quick `fan-in-500` control and records the realized backend for every
successful request. Family-fixed layouts such as Sequence contribute phase observations and are
ineligible for cross-backend comparisons. An unexpected fallback fails verification. Resource
rejections appear alongside completed cases without stage samples. The optional Chrome trace adds
collection overhead. `bench.py report --rust-stages` accepts one recorded host, revision, runner set
and sampling recipe across the selected layouts. Do not splice layout observations from different
collections. Use the ordinary clock runners for revision comparisons.

For allocation call stacks, collect a DHAT heap profile from one post-warmup batch. The runner
builds the `memory` target with `dhat-heap` and records the output path in the workload receipt.

```bash
python3 tools/bench/bench.py rust --plan target/bench/plan.json --scenario small-svg \
  --metric memory --samples 1 --heap-profile target/bench/dhat-heap.json --out target/bench/heap-run.json
```

## Native Criterion benchmarks

The `pipeline` bench can be run directly for local attribution:

```bash
cargo bench -p merman --features all-diagrams,svg,compat-json,editor-facts,layout-dagre --bench pipeline
```

It measures:

- `parse/*`: parsing with a reused `MermaidEngine`, without layout.
- `parse_cold_engine/*`: request-style parsing with a fresh `MermaidEngine` per iteration.
- `compatibility_json_parse/*`: compatibility JSON parsing when the diagram type is already known.
  Historical reports may use its registered `parse_known_type/*` alias.
- `editor_snapshot/*`: requested editor snapshot capture, retaining outcome, warnings, source
  configuration and parser-owned facts.
- `layout/*`: geometry and route computation from a parsed diagram.
- `render/*`: SVG emission from an already laid-out diagram.
- `end_to_end/*`: parse, layout, and SVG emission through the public pipeline.

The native `pipeline` groups require `svg,compat-json,editor-facts,layout-dagre`.
The recipe above includes all diagram families. Cargo defaults supply Cytoscape and ELK.
With defaults disabled, bench-list verification uses
`all-diagrams,svg,compat-json,editor-facts,layout-cytoscape,layout-dagre,layout-elk,math`.

The native corpus registers prepared-render, Sequence message, and parser memory contracts under
`tools/bench/corpus.json` in `memory_bounds`, keyed by lane ID. The entries fix metric bounds,
workload scales, and semantic response expectations. `probe.inputs` lists repository-relative
source paths. The driver hashes those files, Cargo manifests, and `Cargo.lock` for each report and
checks that the recorded files stay stable during the run.

JSON identity projection is outside timing. It sorts object keys on a managed copy while
preserving every array order and value; the measured output is unchanged. This stabilizes
identities for models such as State whose records use randomized map iteration. SVG identities
retain exact serialized bytes. Compare only revisions with the same identity harness; older
unsorted JSON hashes are historical observations, not interchangeable identity receipts.
Include `crates/merman/benches/support/json_identity.rs` when freezing and comparing pipeline
harness sources: the driver's primary benchmark-source digest does not include support modules.

Bench fixtures live under `crates/merman/benches/fixtures/`. A direct Criterion invocation is useful
for exploration, but its internal samples are not independent base/head observations and cannot by
themselves establish an optimization claim. Use `bench.py ab --evidence-mode confirmation` for
that claim.

For one allocation observation per native font case, run the same workloads in `memory`.
It records allocator traffic, peak growth, and bytes live when the operation returns.
The `pipeline` executable uses the ordinary allocator for all timed font groups.

```console
cargo bench --locked -p merman --features native-fonts --bench memory -- --mode fonts
```

### Parser allocation and embedding observations

The parser lanes of `memory` use the shared system-allocator probe protocol and driver. The shared
`tools/bench/corpus.json` registers typed, known-type JSON and requested snapshot
routes for generated Flowchart, Sequence and State chains at scales 1, 2, 4, 10, 32 and 100.
Input/engine setup and warmup precede measurement; each measured result stays alive through the
allocation, retained and peak checkpoints, then drops before the final live-byte checkpoint.
Semantic validation and requested recovery characterization run outside those windows. Zero
mode enters no parser. Seven contracts preregister U3 bounds for Sequence/State typed, JSON and
snapshot requests plus a Flowchart typed control. The remaining Flowchart JSON/snapshot contracts
are infrastructure observations. Candidate admission also requires exact removed-work proof,
semantic parity and the registered latency controls; a memory report alone is insufficient.

```console
cargo bench --locked -p merman --no-default-features --features all-diagrams,compat-json,editor-facts --bench memory --no-run -j 1
python tools/bench/bench.py memory --corpus tools/bench/corpus.json --lane parser-sequence-typed-memory --smoke --json-out target/bench/semantic-boundary/sequence-typed-smoke.json
```

Use the same committed probe and generated input policy on adjacent revisions for removed-work
evidence, retaining raw samples and preregistering bounds before candidate confirmation. These
lanes do not establish latency. Full parser output identities remain separate from their fixed
verified semantic projection. The [three-family embedding recipe](../../tools/bench/fixtures/semantic-parser/README.md)
records how the historical run retained raw and stripped executables and verified the
consumed outputs from the typed parser.

The `profile_render` example reports SVG emission time for `--stage render`. Each sample starts
after layout and artifact preparation. Whole-process profiles still include that setup, so compare
the `profile_render::emit_prepared_svg` subtree with Criterion's `render/*` group. Use `--layout`
to select the default site backend from `dagre`, `elk`, and `shoal`.

### Theme compilation and discovery

The `bindings` Criterion target measures Theme v2 preset compilation, editable document export,
JSON resolution, and catalog lookup. It uses Studio Dark and
Cyberpunk with one engine and compiler for the timed iterations. The resolve cases check the
response before measurement.

```bash
CARGO_BUILD_JOBS=1 cargo bench --locked -p merman-bindings-core \
  --no-default-features --features svg --bench bindings -- --test
CARGO_BUILD_JOBS=1 cargo bench --locked -p merman-bindings-core \
  --no-default-features --features svg --bench bindings -- \
  --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3
```

Keep `target/criterion/theme_*` raw samples with the source, lockfile, and executable hashes.
Use a separate `CRITERION_HOME` for another feature profile. Times include admission and result
allocation and destruction within an already running process. Catalog timings apply to the
capabilities compiled into that build.

### Native SVG/PNG/PDF throughput

The `pipeline` export lane measures the public `Renderer` source-to-output path for the
unchanged public Cyberpunk Flowchart, Sequence and XY Chart fixtures in Mermaid default and native
Cyberpunk modes. It reuses the renderer and compiled theme, uses strict parsing, native SVG text
(`htmlLabels: false`), a resvg-safe pipeline, fixed diagram IDs, and default render/export resource
limits. All three targets use the checked-in Noto Sans and Cousine test fonts for measurement and
painting. PNG uses the default 1x scale without a fit box.

```bash
MERMAN_BENCH_PIPELINE_LANE=export CARGO_BUILD_JOBS=1 cargo bench --locked -p merman --no-default-features \
  --features svg,png,pdf,native-fonts,diagram-flowchart,diagram-sequence,diagram-xychart,layout-dagre \
  --bench pipeline -- --test
MERMAN_BENCH_PIPELINE_LANE=export CARGO_BUILD_JOBS=1 cargo bench --locked -p merman --no-default-features \
  --features svg,png,pdf,native-fonts,diagram-flowchart,diagram-sequence,diagram-xychart,layout-dagre \
  --bench pipeline -- \
  --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3
```

Each of the 18 rows includes parse, layout, SVG emission, terminal encoding, request cloning and
output destruction. The font context is loaded before registration and preflight renders each
scene before timing. Flat sampling avoids linear iteration growth for expensive PDF cases. Slow
cases may exceed the requested measurement duration to collect all samples. Criterion reports
throughput as complete outputs per second.

Pre/postflight requires identical output bytes. SVG must parse, PNG must decode with positive
dimensions, and PDF must have valid framing; these checks do not establish visual qualification.
Preflight emits source/output hashes. Set `MERMAN_BENCH_ARTIFACT_DIR` to retain the actual output
files outside the timed loop, and use a separate `CRITERION_HOME` for each measured feature lane.
A failed or rejected output stops the run rather than becoming a fast timing sample. These are
diagnostic baselines until a separate matched, calibrated comparison establishes a revision claim.

### ASCII semantic-depth benchmark

The `pipeline` ASCII lane measures the public synchronous ASCII renderer with strict parsing,
the explicit `UnboundedForTrustedInput` resource profile, Plain 7-bit output, a reused engine, and
one logical render operation per estimate. This is a controlled local batch/stress lane whose
inputs come only from the tracked corpus; the public `Interactive` and `TrustedNative` rejection
boundaries remain covered by exact resource tests rather than benchmark admission:

```bash
MERMAN_BENCH_PIPELINE_LANE=ascii CARGO_BUILD_JOBS=1 cargo bench --locked -p merman \
  --no-default-features --features all-diagrams,ascii --bench pipeline
```

Its schema-v2 metadata is the `ascii` view in `tools/bench/corpus.json`. The `closeout` and `large-closeout`
suites cover the five common families selected by U25, while `comparable` contains only fixtures
whose base and head output identities may support a latency comparison. Every registered benchmark
must emit a `plain_ascii` preflight receipt with exact output bytes and SHA-256 before Criterion
timing begins, and an exact invocation must repeat the same identity after timing. Verify the
compiled list and those receipts with:

```bash
CARGO_BUILD_JOBS=1 python3 tools/bench/bench.py criterion-list \
  --bench pipeline \
  --corpus-view ascii \
  --features all-diagrams,ascii
```

The dedicated Performance workflow exposes the same bounded recipe without adding timing to pull
requests. Dispatch a decision-grade comparison explicitly, using a benchmark-only backport ref
whose harness and corpus are byte-identical to the head checkout:

```bash
gh workflow run performance.yml \
  --ref main \
  -f run=ascii \
  -f base_ref=<benchmark-backport-ref> \
  -f head_ref=my-ascii-branch \
  -f preset=long \
  -f ascii_suite=comparable \
  -f evidence_mode=confirmation \
  -f relative_threshold_percent=10 \
  -f absolute_threshold_ns=50000
```

The workflow accepts `comparable` and `closeout`; use `closeout` for the five medium U25 families.
The larger `large-closeout` suite is intentionally excluded from unattended timing and must be run
locally for the tracked closeout scorecard. Pull requests whose changed paths select the performance
owner still compile and verify the complete ASCII benchmark list without a timing label. Ordinary
renderer changes do not run the standalone performance contracts unless `perf` or
`perf-frontmatter` is applied.

For a decision-grade adjacent-revision comparison, first ensure both checkouts contain the same
benchmark-only harness: `pipeline.rs` and its ASCII and identity modules, the `Cargo.toml` bench
entry, the selected corpus, the preflight contract, and every selected fixture must be byte-identical. If the
historical product revision predates that harness, create a clean benchmark-only backport commit
whose parent is the product revision; do not copy only the corpus into an otherwise different
checkout. The recipe below assumes both revisions support `all-diagrams`. For a historical
base that predates diagram selectors, use `--base-features ascii` instead; retain
`--head-features all-diagrams,ascii` for the current implementation. Select features according
to each revision's manifest, while keeping the exercised diagram families identical.
Keep the ASCII feature closure and each checkout's corpus path explicit:

```bash
python3 tools/bench/bench.py ab \
  --base-dir ../merman-base \
  --head-dir . \
  --base-label BASE \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench pipeline \
  --head-bench pipeline \
  --base-features all-diagrams,ascii \
  --head-features all-diagrams,ascii \
  --no-base-default-features \
  --no-head-default-features \
  --base-corpus-view ascii \
  --head-corpus-view ascii \
  --suite comparable \
  --group ascii_end_to_end \
  --preset long \
  --evidence-mode confirmation \
  --calibration-pairs 8 \
  --max-pairs 64 \
  --relative-threshold-percent 10 \
  --absolute-threshold-ns 50000 \
  --out target/performance/ascii_comparison.md \
  --json-out target/performance/ascii_comparison.json
```

Both corpus paths are resolved relative to their respective checkout. For a base checkout that
retains the standalone catalog, use `--base-corpus tools/bench/ascii_corpus.json` in place of
`--base-corpus-view ascii`. The benchmark workload,
`UnboundedForTrustedInput` resource profile, bench source, lane contract, and selected fixture
bytes must be identical on both sides. Confirmation mode rejects a different Cargo `[[bench]]`
entry, benchmark source, corpus manifest, or preflight contract before sampling; selected fixture
bytes are checked independently. Do not compare an older checkout's historical harness under the
same operation name. Record the benchmark-only backport revision separately from its product parent
so the final receipt cannot mistake harness provenance for a product change.

The generated Markdown and JSON are machine-local evidence until their exact revision, tree,
environment, corpus and harness digests, output identities, numeric results, and maintainer
disposition are committed in an ASCII closeout scorecard. Until that tracked scorecard exists, a
direct Criterion run or a receipt without those provenance fields is not a U25 closeout claim.

## Decision-grade self-comparison

### Two independent runner recipes

Base and head are separate `RunnerRecipe` values. Each recipe records its own:

- checkout and human-readable label;
- Cargo package and benchmark target;
- feature list and default-feature policy;
- Rust toolchain and optional compilation target;
- Cargo target directory;
- corpus manifest path;
- a corpus schema-v2 lane contract that owns public operation, lifecycle, transport, selector
  history, and the logical-operation divisor.

Do not project the head recipe onto an older revision. Capability names can change across releases.
For example, `v0.8.0-alpha.3` exposes SVG rendering through `render`, while the current candidate
uses `all-diagrams,svg,compat-json,editor-facts,layout-dagre` for the full pipeline bench.
`bench.py ab` keeps the historical base default at `svg` and uses that full recipe for head.
Override `--base-features` and `--head-features` independently according to each checkout's bench
requirements. A base that predates `compat-json` or `editor-facts` must keep its own feature
vocabulary; copying the head recipe onto it can fail before measurement.

This historical comparison makes that difference explicit:

```bash
python3 tools/bench/bench.py ab \
  --base-dir ../merman-alpha3 \
  --head-dir . \
  --base-label v0.8.0-alpha.3 \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench pipeline \
  --head-bench pipeline \
  --base-features render \
  --head-features all-diagrams,svg,compat-json,editor-facts,layout-dagre \
  --base-toolchain 1.95.0 \
  --head-toolchain 1.95.0 \
  --base-target-dir target/performance/base \
  --head-target-dir target/performance/head \
  --base-corpus tools/bench/corpus.json \
  --head-corpus tools/bench/corpus.json \
  --suite canary \
  --preset long \
  --evidence-mode confirmation \
  --calibration-pairs 8 \
  --max-pairs 64 \
  --relative-threshold-percent 10 \
  --absolute-threshold-ns 50000 \
  --out target/performance/alpha3_to_head.md \
  --json-out target/performance/alpha3_to_head.json
```

The default-feature policy is also side-specific and defaults to enabled. Use
`--no-base-default-features` or `--no-head-default-features` when the corresponding recipe must have
a narrow explicit feature closure and the selected suite does not require a disabled family. Missing
capability for any mandatory fixture is an evidence-contract failure. Use `--base-target` and
`--head-target` for explicit cross-target recipes; otherwise the native target is recorded.

The alpha.3 range is historical context, not causal proof for one optimization. Prove a candidate
against its adjacent clean pre-change commit using the same two-sided recipe contract.

### Build and executable freeze

`bench.py ab` performs build work before any timed pair:

1. It invokes locked Cargo bench compilation independently for each recipe with `--no-run`, Cargo
   JSON diagnostics, and a forced `CARGO_BUILD_JOBS=1` environment recorded under each runner's
   `build_environment` receipt.
2. It accepts exactly one matching Criterion executable for the requested package and bench.
3. It records and verifies the executable digest and discovers exact benchmark names directly from
   that executable.
4. Every timed sample invokes the frozen executable directly with Criterion's bench mode enabled.

Cargo is therefore outside the timed AB/BA schedule. A missing lockfile, ambiguous executable,
changed digest, absent benchmark, or failed direct invocation is an evidence-contract failure, not
a slow sample.

Base and head should normally use distinct target directories; those prebuilds are still serialized
with one Cargo build job each. When disk constraints require `--freeze-shared-target`, the runner
clears only the shared target's `bench` profile before building each side, then copies that side's
executable under `target/perf-frozen`. This reset is required:
Cargo's cache is not a content-addressed boundary between different path-workspace checkouts, so a
plain second `--target-dir` build can otherwise reuse the first checkout's executable. The debug
profile and immutable frozen copies are not removed. Do not run another Cargo process against that
target during either reset, build, or timing schedule. Different Git trees that still resolve to
byte-identical benchmark executables fail the comparison, because they do not provide an
independently built binary delta to measure. The JSON report records the comparison harness digest
as well as both executable digests.

### Corpus and fixture byte gate

The harness loads the corpus independently from both recipes. It compares the selected fixture on
both sides before timing and records each path, byte length, and SHA-256 digest.

Only byte-identical fixtures available to both executables are comparable. Missing or different
bytes remain visible as coverage evidence but cannot produce a timing ratio. A required selection
with no comparable row fails the evidence contract. This prevents source evolution from being
misreported as an engine regression or improvement.

### Diagnostic evidence

Diagnostic mode runs one to four paired observations with alternating AB/BA order, balanced whenever
the requested pair count is even. It is intended for PR triage and hypothesis selection only:

- observed movement is always advisory;
- it cannot accept or reject a performance candidate;
- it exits successfully when timing completes, regardless of movement;
- recipe, byte, capability, runner, and report-contract failures still use exit code `2`.

Do not keep sampling a diagnostic run until it looks favorable. Move to a preregistered confirmation
run when a result will influence production code.

### Confirmation evidence

Confirmation mode separates calibration from the final comparison:

1. Run at least eight order-balanced same-binary A/A pairs for base and for head.
2. Require the two-sided identity and order-effect intervals to remain fully inside the
   preregistered relative and absolute equivalence margins, and estimate dispersion for both
   metrics.
3. Derive the next even confirmation count from each metric using
   `max(8, ceil(((1.645 + 0.842) * sigma / MDE)^2))`.
4. Use the largest required count across every required row and both metrics, bounded by the
   preregistered `--max-pairs` budget.
5. Collect a fresh, even, order-balanced AB/BA confirmation schedule. Calibration and exploratory
   observations are never reused as confirmation evidence.

If A/A is unstable or the derived count exceeds the fixed budget, the result is inconclusive. The
harness does not weaken thresholds, extend the budget after seeing results, or stop when an interval
first becomes favorable.

The generic A/A prerequisite still requires identity, order effects and the sampling budget to
pass separately for both metrics. That is stronger than proving the complement of a joint
regression, which needs only one qualified upper bound below its threshold. The classifier uses
that documented complement; it does not relax A/A, sample counts, confidence adjustment or
improvement admission. Historical receipts retain their original method and outcomes. Any future
release-specific calibration protocol must be versioned and preregistered before fresh A/A and A/B;
old observations cannot be reclassified as new confirmation.

One normalized Criterion point estimate per side within an AB/BA pair is one independent
observation. Corpus schema-v2 lane metadata owns the public operation and normalization. Current and
registered historical selectors can therefore confirm the same operation even when their Criterion
group names differ. An explicit `--base-logical-operations` or `--head-logical-operations` value must
match the registered divisor. Legacy corpus schema-v1 evidence has no such attestation: confirmation
requires exact benchmark identity and divisors of one, while any override remains diagnostic-only.

### Statistics and decisions

For a release-distance question, opt into `--confirmation-protocol joint_non_regression_v1`.
The default `generic_v2` and low-latency improvement contracts retain their existing A/A rules.
This separate protocol confirms the complement of a joint regression; it never assesses candidate
improvement or admission. This version fixes the confidence level at 0.95 and the thresholds
at 10% and 1000 ns; every selected row
must be a comparable registered public operation. It cannot run in diagnostic, discovery-only, dirty or
low-latency mode. Run fresh discovery, A/A calibration and A/B observations for each confirmation.

For each row and metric, both runners must complete eight A/A pairs with identity and order-effect intervals
inside that metric's margin. The maximum untruncated required pair count across the two runners
must fit the cap. Among qualified metrics, select the smaller required count, breaking ties in
favor of relative movement. Freeze this selection before A/B. The cohort count is the maximum
selected count over every row. If any row has no qualified metric, block A/B globally and retain
every row as inconclusive. Runner, identity and coverage failures remain evidence-contract failures.

Before the first A/B observation, create an exclusive `*.calibration-plan.json` beside the JSON
report. It records the complete fixture scope, frozen runners, A/A results, each metric's
qualification/reasons/required count, selected certificates, thresholds, confidence recipe and
fixed schedule. An existing or unwritable plan blocks A/B. The final verification checks that the plan
and its SHA match the frozen schedule. Place evidence under ignored
`target/bench`; a plan written into tracked source is not exempt from checkout-identity checks.

Every row must complete the fixed balanced cohort schedule before any timing label is admitted.
Only its preselected metric can prove non-regression with an upper bound at or below the threshold.
Regression additionally requires both metrics qualified, enough fixed pairs for both, and both
lower bounds strictly above threshold. Other cases remain inconclusive; a truncated or failed
schedule is an evidence-contract failure. Both raw metrics and all qualification reasons remain
visible. `improvement_outcome` is null because this protocol does not assess improvement.

The decision error budget uses `2R` components for `R` selected rows. Conditional on the frozen
plan and valid paired-bootstrap bounds, a false non-regression label requires the preselected
upper bound to fail; a false regression label requires a lower bound for an actually non-regressing
metric that has not crossed its threshold to fail. Allocating `alpha/(2R)` per tail bounds these
two decision errors across the cohort by alpha. This is not a claim that all displayed upper and
lower endpoints simultaneously cover
their estimands. Bootstrap validity remains an empirical assumption about the host and noise.
Selecting a smaller A/A variance can underestimate sampling needs; the pair-count formula is a
planning heuristic, not a guarantee of 80% power. No post-A/B metric switch, budget extension,
early favorable stopping or historical receipt reclassification is allowed.

The canonical signed metrics are:

- relative movement: `r = log(head / base)`;
- absolute movement: `d = head - base` in nanoseconds.

Positive values mean regression. The report retains every raw pair and computes deterministic
paired-bootstrap bounds with suite-level 95% simultaneous coverage. For `R` comparable rows, the
confidence family contains `2R` components: relative and absolute movement for every row. A
Bonferroni adjustment gives each component confidence `1 - 0.05 / (2R)`.

The comparator's default screening budget is relative movement greater than 10 percent **and**
absolute movement greater than 50 us. `--absolute-threshold-us 50` is an equivalent convenience
form of `--absolute-threshold-ns 50000`. This default keeps unattended reports comparable; it is
not a universal admission or rejection rule.

Every decision-grade latency or throughput candidate must preregister one materiality contract
before confirmation samples are collected:

- the default joint `10% AND 50 us` budget when it represents the public operation;
- the noise-derived low-latency formula below;
- a documented interactive, throughput, or high-volume integration objective converted into
  operation-level or workload-level bounds; or
- a separate structural or memory claim whose public latency lane is explicitly a non-regression
  control rather than the claimed benefit.

A scenario-specific contract must name the public operation, expected call volume or user-visible
budget, relative and absolute A/A noise floors, and the exact confirmation statistic. Thresholds
must dominate the registered noise floor and cannot be selected or relaxed after looking at
confirmation. A large ratio on a tiny private stage remains attribution evidence, not a public
latency claim.

#### Low-latency public operations

The default absolute threshold is structurally unreachable when the complete public-operation
baseline is below 50 us and can dominate the relative threshold for every baseline below 500 us. A
candidate with a frozen baseline below 500 us may preregister this noise-derived alternative only
before collecting confirmation samples and only for a complete public operation with frozen
base/head revisions and inputs. Private stages remain attribution evidence.

Let `B` be the frozen baseline estimate in nanoseconds per logical operation. Let `e_r` and `e_d` be
the maximum absolute endpoints from the independent base and head A/A simultaneous 95% intervals
for log ratio, absolute delta, and their order effects. A/A is stable only when identity or zero is
inside every interval, `e_r <= -ln(0.95)`, `e_d <= min(1,000 ns, 0.05 * B)`, and the power-derived
pair count does not exceed the preregistered cap. Define:

```text
T_r = max(-ln(0.90), 3 * e_r)
T_d = max(1,000 ns, min(50,000 ns, B * (1 - exp(-T_r))), 3 * e_d)
```

Fresh confirmation accepts the performance claim only when the simultaneous one-sided 95% upper
bounds for `r = ln(head/base)` and `d = head-base` are below `-T_r` and `-T_d`. Derive one fixed even
AB/BA pair count from both thresholds using the normal power procedure, with at least eight pairs
and a preregistered cap no greater than 64. Do not reuse discovery or A/A observations, remove rows,
change the lifecycle, or extend the budget after seeing confirmation.

Use `--low-latency-baseline-ns FIXTURE=NS` once for every selected row to opt into this contract.
The value is the baseline estimate frozen in the experiment ledger before confirmation, not an
estimate selected from A/A or fresh A/B. For example, if the preregistered complete-operation
baseline for `kanban_medium` is 40,000 ns:

```bash
python3 tools/bench/bench.py ab \
  --base-dir ../merman-base --head-dir . \
  --base-features all-diagrams,svg,compat-json,editor-facts,layout-dagre \
  --head-features all-diagrams,svg,compat-json,editor-facts,layout-dagre \
  --group end_to_end --filter '^end_to_end/kanban_medium$' \
  --evidence-mode confirmation --preset long \
  --low-latency-baseline-ns kanban_medium=40000 \
  --calibration-pairs 8 --max-pairs 32 \
  --out target/perf/kanban-confirmation.md \
  --json-out target/perf/kanban-confirmation.json
```

This mode requires clean frozen revisions and registered public-operation lane metadata on both
sides. It rejects baselines at or above 500 us, a cap above 64, incomplete row registration, and
manual threshold overrides. A/A uses a simultaneous confidence family covering both runners,
identity and order effects, and both metrics (`8R` components for `R` rows). Its identity-zero and
noise checks are stricter than generic equivalence calibration; generic confirmation keeps its
existing behavior. After both A/A schedules finish, the report records each frozen baseline,
noise floor, derived thresholds, and fixed power count before fresh balanced A/B starts. If any
selected row fails calibration, fresh A/B is blocked for the entire selected set. No rows are
removed to continue confirmation. JSON `method.materiality_contract` and the Markdown
"Low-latency Materiality" table own the effective gate; generic threshold fields are unset.

The low-latency gate handles measurement scale; it does not erase engineering cost. A candidate
that adds persistent state or a large owner implementation must also preregister and pass a concrete
memory, throughput, or documented integration objective, keep cold and unaffected paths within
their noise budgets, and remove the superseded production path and temporary oracle before
acceptance.

Conversely, an owner-local change may be accepted under a structural or memory claim without a
latency label when it removes reachable repeated work, improves a user-controlled complexity bound,
or reduces allocation/retained state with exact counters and semantic parity. The smaller and more
obvious the implementation, the less additional machinery it should require. New caches, protocol
machinery, persistent state, or broad abstractions require correspondingly stronger public
latency, throughput, or memory evidence.

Confirmation fixes `--bootstrap-resamples` at a decision-grade minimum of 10,000. Smaller values
are permitted only for diagnostic exploration and can never produce a confirmation outcome;
values above 100,000 are rejected to keep evidence generation bounded.

Per row:

- `confirmed_regression`: both regression lower bounds clear their positive thresholds;
- `confirmed_non_regression`: either regression upper bound cannot clear its threshold;
- `inconclusive`: the interval crosses the joint decision boundary, A/A is unstable, or the fixed
  measurement budget is insufficient.

Candidate admission is a different question from regression gating. It uses the mirrored
improvement view of the same paired bounds. A candidate is performance-confirmed only when both
mirrored improvement bounds clear the relative and absolute thresholds. It is accepted only when
that improvement and all applicable semantic, error, resource, memory, security, host, and control
gates pass. A suite exit of `0` is not proof of improvement.

Suite exit precedence is fixed:

| Exit | Meaning |
| ---: | --- |
| `2` | Evidence-contract failure, including invalid recipes, byte/capability gaps, runner errors, or no comparable required row. |
| `1` | At least one confirmed regression. |
| `3` | At least one required confirmation row is statistically inconclusive. |
| `0` | Complete diagnostic advisory, or suite-wide conclusive non-regression. |

The precedence is `2 > 1 > 3 > 0`. Candidate accepted/rejected/inconclusive state must be read from
the mirrored per-row fields and mandatory non-performance gates, never inferred from this process
code alone.

### Structural complexity and resource amplification

The latency gates above are not universal admission criteria. A separately preregistered
structural lane may accept a repair without a measurable ordinary-fixture speedup when an
externally reachable input controls repeated work, the old implementation contains a higher-order
or unbounded term, and the candidate removes or caps that term.

Freeze the evidence model before implementation:

- name the independent input variables and the resource limit that bounds each one;
- state the reachable pre-change and post-change time and added-space bounds, distinguishing
  worst-case bounds from expected hash-table behavior;
- identify the exact loop, lookup, rebuild, or expansion that realizes the old term;
- choose code-path proof plus an exact test-only work counter, or a preregistered multi-scale curve
  when the bound cannot be established directly;
- preserve semantic output, structured errors and precedence, security policy, resource charges,
  cancellation, and deterministic ordering;
- run the existing representative public lane as a non-regression control when practical, and add
  a memory bound when the data-structure change adds scale-sensitive state.

Acceptance requires a reviewed causal derivation and passing correctness/resource gates. A noisy
or sub-threshold clock result does not invalidate a proven removal of attacker-controlled
quadratic work, but a confirmed material public-workload regression still requires rejection or an
explicitly approved tradeoff. Describe the result as, for example, `O(A^2)` to expected `O(A)` or
`O(T * (V + E))` to `O(T + V + E)` with its added-space bound. Do not call it a latency speedup
unless a separate public-operation timing confirmation passes.

Prefer existing correctness suites plus one combined adversarial scale test over a permanent
fixture-specific benchmark. Delete candidate-only counters, lanes, contracts, and generators after
rejection; retain infrastructure only when it serves a stable recurring contract.

### Layout-work policy calibration

A public layout-work ceiling is a host-safety policy, not a latency benchmark and not a Mermaid
semantic constant. Calibrate it from a closed, hashed corpus plus a rule registered before the
decision. The rule must state its absolute headroom, relative headroom, and rounding quantum so the
selected ceiling is reproducible rather than chosen after observing the maximum fixture.

The release probe and wrapper must establish all of the following:

- a clean source revision, exact release feature closure, toolchain, executable hash, corpus
  manifest hash, and aggregate member hash;
- multiple fresh full-process runs with byte-identical reports;
- the maximum corpus member, its source/output hashes, and the calculated headroom;
- exact adjacent `W/W-1` admission behavior with a structured rejection payload;
- a preregistered model-cardinality curve whose first rejection is established either by a
  complete sequential accepted-prefix scan or by a reviewed monotonicity proof, scoped only to
  that curve;
- isolated semantic, layout, SVG, end-to-end, and failure probes, each in a fresh timeout-bound
  managed process group with exit status and peak RSS; cleanup must not infer group exit from the
  leader or inherited pipe state, and its regression suite must run in the performance-contract CI;
- a committed evidence manifest that binds ignored raw reports and timing output by byte length and
  SHA-256.

These observations can admit an `accepted-structural` resource-policy decision. They do not admit a
speedup, memory improvement, portable memory limit, or host SLO without a separate candidate-local
A/B or memory contract.

### Report schema

Self-comparison reports use schema version `2`. The JSON is the audit artifact; Markdown and PR
comments are projections of it. It records:

- both full runner recipes and revisions;
- Cargo lock, manifest, toolchain, build-command, target, environment, host, and executable-digest
  provenance;
- corpus and fixture paths, byte lengths, hashes, capability discovery, and coverage gaps;
- evidence mode, thresholds, confidence settings, bootstrap seed/resamples, pair budgets, and the
  declared/effective lane contract and logical-operation divisor;
- base/head A/A calibration, order/stability diagnostics, derived power requirement, and every raw
  pair;
- fresh AB/BA confirmation pairs, one-sided relative and absolute bounds, regression state, mirrored
  improvement state, aggregate outcome, and exit code.

`--out` and `--json-out` must resolve to distinct files, including after resolving `..` components
and symbolic links. The runner rejects aliases before building either candidate.

Consumers must fail closed on an unknown schema. Schema version `1` can be displayed only as legacy
diagnostic evidence and cannot support a current admission decision. The comment renderer always
writes an available failure projection, including for nonstandard signal/OOM exit codes, then exits
`2` when its consumer contract fails. CI combines that status with the producer exit code.

## Native allocation and peak-live evidence

Run the isolated native `System` allocator lane explicitly:

```bash
python3 tools/bench/bench.py memory \
  --json-out target/bench/native_memory.json
```

The driver builds a dedicated instrumented executable once, freezes its SHA-256 digest, and then
launches one fresh process for every operation or matched zero-work sample. The renderer is prepared
before the allocator snapshot, so `fresh-process` describes sample isolation while `reused-engine`
describes the measured public operation. This executable is never used for latency evidence.

For lanes with identical Cargo recipes, preflight them together and build once; each still receives
its own full report and fresh subprocess schedule, and must be frozen separately:

```console
python tools/bench/bench.py memory --lanes parser-sequence-typed-memory parser-state-typed-memory --json-out-dir target/bench/parser-memory
```

Batch execution retains the actual shared Cargo build receipt and rejects source, build-environment,
tool-version, recipe, or executable changes before reuse. It cannot use `--contract` or `--executable`.

The registered contract fixes six scales (`1, 2, 4, 10, 32, 100`), one seed across the matrix, at
least five repeats, and alternating operation/zero order. Strict one-line JSON records executable and
invocation identity, generated node/edge counts, deterministic SVG identity and dimensions,
allocation count, allocated bytes, and peak live-byte growth. The analysis subtracts each matched
zero sample, retains per-scale medians, and uses deterministic matched-vector bootstrap bounds for
the OLS slope and scale-100 median.

The Flowchart generator uses three nodes and four edges per scale unit, so its `100x` boundary stays
inside the default layout-work policy while still exercising 100 clusters, 300 nodes, and 400 edges.
Protocol smoke runs matched operation/zero pairs at both `1x` and `100x`; this catches setup leakage
and resource-budget drift before a full 60-process matrix.

An upper bound at or below the cap passes; a lower bound above the cap fails; a crossing interval is
inconclusive. Exit precedence is the same `2 > 1 > 3 > 0` contract used by latency evidence. The
checked-in `infrastructure-smoke` owner contract contains broad harness-safety bounds only and has
`candidate_admission: false`. Every optimization owner must preregister tighter adjacent-candidate
memory bounds and combine them with its semantic and latency gates.

Run the memory lane explicitly.

```bash
python3 tools/bench/bench.py memory
```

A full native-memory report records the Git commit/tree and dirty disposition, workspace/package
manifest and lock digests, build environment, requested and actual Rust toolchain, executable digest,
and the complete raw schedule. Full evidence rejects a dirty worktree by default. `--allow-dirty`
exists only for investigation, forces `candidate_admission` false, and cannot create a durable
receipt. Protocol smoke is likewise never admission evidence.

Keep the complete native-memory report under `target/bench/` or attach it to the CI run.
A dated decision receipt links the report and records its source revision, feature recipe,
executable digest and outcome. Preserve the raw samples with the report for replay.

## Cross-renderer comparison

Use the same frozen workload plan to compare the pinned Pele and Mermaid v13 products. `bench.py
node` retains coverage, output checks, and warm samples per product. A variant can select a local
Merman entry and an explicit site layout. Comparable variants pass the plan's diagram ID on every
request and require explicit capability evidence for a declared layout or an intrinsic Sequence
layout. A named Merman no-options
diagnostic records its direct calls and has no comparison ratio. Published round comparisons check
the original worker rows against the frozen plan and replay eligible pairs from their measured
batches. Cold CLI, native Rust, Node N-API, Node WASM, and browser results have distinct execution
and output boundaries. Use the runner for each boundary and report only qualified matching
requests. [Node reference workflow](RUNBOOK.md#run-an-ordered-workload)

## Browser comparison with Mermaid.js

Use the generated family-baseline corpus for the web-to-web comparison. The full command builds the
Playground with the prepared Web package and measures one baseline from each supported generated-example family:

```bash
npm --prefix playground run benchmark:corpus -- \
  --iterations 6 \
  --warmups 2 \
  --out target/bench/browser-family-corpus.json
```

After an already verified playground build, select fixtures for a diagnostic rerun with:

```bash
npm --prefix playground run benchmark:corpus:built -- \
  --fixtures c4-system-context,mind-map,requirement-trace \
  --iterations 6 \
  --warmups 2 \
  --out target/bench/browser-family-targeted.json
```

The corpus runner derives its inventory from generated examples whose evidence role is
`family-baseline`. It starts a fresh Chromium process for every selected fixture to bound retained
renderer state, and each page accepts exactly one fixture id plus its precomputed cold and warm
seeds. Within that fixture, Merman and Mermaid.js still use the same page, source bytes, theme,
viewport, warmup policy, iteration count, and deterministic AB/BA sample plan. Independent cold and
warm controller runs report first-publishable SVG and reused-engine publishable SVG separately. Both
engines pass through the production browser adapters and the same strict SVG evidence checks.
For warm rendering, `warmBudgetedSvgMs` ends when the adapter has returned an SVG and the output
byte check has passed. The fixture then inserts the SVG, checks its layout box and waits for one
`requestAnimationFrame` callback. `warmPublishableSvgMs` includes that wait, response delivery and
validation on the parent page. Read the two medians separately. The adapter can itself perform DOM
layout and the frame callback does not timestamp pixels on the display.
Before sampling, the runner checks the built page's Merman version against the selected Web
package manifest. Each successful mode also checks the versions returned by the running engines.

Corpus schema `2` separates the single-fixture page envelope from the aggregate corpus report. A
page returns exactly one success or structured failure row plus stable generated-catalog identity;
it does not fabricate `pending` or `not-selected` rows for the rest of the catalog. The CLI converts
page crashes, early closes, browser disconnects, timeouts, invalidation, and cancellation into the
same fixture envelope, then one shared linear assembler validates order, seeds, options, catalog and
version identity while deriving coverage, failures, and terminal status. The aggregate embeds raw
schema-`7` controller reports, fixture byte counts and SHA-256 digests, actual execution order, and
fresh-process isolation. It never overwrites an existing output. A whole-corpus budget rejects plans
that would retain more than 4,096 raw samples before the first fixture runs. In each successful
fixture mode's `report.aggregates.ratios`, the numerator is Mermaid.js and the denominator is Merman,
so values greater than `1` favor Merman.

An even `--iterations` value is required by the balanced plan. A two-iteration, zero-warmup run
is useful only to verify coverage and discover large candidates. The default six iterations and two
warmups provide better diagnostic ranking, but they are not a universal admission threshold. Use a
targeted rerun, inspect raw samples, variance, and absolute milliseconds, then confirm any production
change against the same Merman base/head public operation before accepting it.

## Stage and render diagnostics

Run `bench.py stages` after observing a public-operation signal. `--with-memory` runs the independent
allocation probe with the same inputs. `--chrome-trace` captures timing scopes for Perfetto.

```bash
python3 tools/bench/bench.py stages --plan target/bench/plan.json --layouts dagre,elk,shoal \
  --with-memory --chrome-trace target/bench/stages.trace.json --out target/bench/stages.json
```

Direct `merman-render` callers can enable `SvgDebugOptions::include_timing_diagnostics` through a
`*_with_debug` entry point. Keep those diagnostics outside the timed production path.

Criterion's native stress groups batch small operations. Select a group before registration so
it runs only its own probes.

```bash
MERMAN_BENCH_PIPELINE_LANE=flowchart-stress cargo bench --locked -p merman \
  --features svg,compat-json,layout-dagre --bench pipeline -- --noplot
```

The additional lane values are `architecture-stress`, `architecture-layout-stress`,
`mindmap-layout-stress`, `text-stress`, `fonts`, and `export`. The enabled Cargo features select
which lanes are present. Record the fixed-repeat divisor and confirm the corresponding public
operation before accepting a code change.

## Host discipline

- Compare clean committed revisions on the same host.
- Keep the toolchain, target, power mode, thermal state, and background load stable.
- Prebuild serially and do not run Cargo while the paired schedule is sampling.
- Freeze the experiment configuration before viewing confirmation data.
- Retain the schema-v2 JSON report even when the result is rejected or inconclusive.
