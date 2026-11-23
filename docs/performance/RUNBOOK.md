# Performance Runbook

This is the operating procedure for performance work in `merman`. The statistical and report
contracts are defined in `docs/performance/BENCHMARKING.md`; the active candidate queue lives in
`docs/performance/PERF_PLAN.md`.

## Evidence classes

Use the narrowest lane that answers the current question:

- Local Criterion, `perf_runner.py`, stage spot-checks, stress benches, and one-to-four-pair
  `compare_self.py` runs are diagnostic. They identify a likely owner or reject weak hypotheses.
- `compare_self.py --evidence-mode confirmation` is the decision-grade same-host regression lane.
- Cross-renderer and browser comparisons provide product context, not causal base/head evidence.
- Allocation, peak-live-heap, RSS, and artifact-size gates remain separate from latency.

Do not carry a diagnostic timing claim into production admission. Confirmation uses fresh samples
under a frozen recipe and fixed maximum budget.

## GitHub Actions lanes

The `regression` measurement job uploads separate memory, retention and topology observations.
Labelled PRs and ordinary regression dispatches use `perf-public-workloads` and the quick
suite. Scheduled runs and full dispatches use the full suite. It compares both revisions when the
base contains the workload runner. On older bases the artifact identifies the missing observation.
The [local recipe](#run-an-ordered-workload) uses the same inputs and commands.

Scheduled regression selects the last measured ancestor from this workflow and branch with a
compatible workload receipt. The current runner measures both ends of the interval. The step
summary names the SHAs and the earlier run, attempt and artifact. On the first run, or after
all eligible artifacts have expired or become unreachable, it measures the current SHA against
itself (`bootstrap`). A change in the sentinel, measurement code, runner recipe or toolchain
starts another same-SHA baseline (`protocol-change`). `frontmatter` keeps its own parent-to-head
comparison. A scheduled receipt contains `weekly-interval.json`, the full raw observations and
`weekly-sentinel.json`, a standalone `mixed-families` plan. Check the per-side coverage in the
receipt for product errors, including default-budget refusals. An incomplete measurement is
uploaded under a separate artifact name and cannot become the next baseline.

Performance automation is isolated in the `Performance` workflow:

- `perf-contracts` checks Python syntax and benchmark-helper contracts on manual and scheduled
  runs. Every path-matched pull request retains the compiled `ascii_pipeline` list contract, even
  without a performance label; the broader compiled pipeline and native-memory smoke checks remain
  gated by `perf`.
- `perf-regression` runs schema-v2 `compare_self.py` evidence for PR or manual base/head checkouts.
  A PR with the `perf` label receives a short diagnostic AB/BA schedule. Timing movement is advisory,
  but any recipe, fixture, executable, runner, or report-contract failure still fails the lane.
  Artifacts and the sticky PR comment are written before the comparison exit code is enforced.
- The manual `ascii` lane uses `ascii_pipeline`, `tools/bench/ascii_corpus.json`, the `ascii`
  feature with default features disabled, and the `ascii_end_to_end` public-operation lane.
  Dispatch it only with a benchmark-only base backport that carries the byte-identical harness;
  ordinary PR labels do not run ASCII timing. The larger `large-closeout` suite remains an
  explicit local closeout observation rather than an unattended CI timing lane.
- A manual `perf-regression` dispatch defaults to confirmation mode and can set the corpus suite,
  both thresholds, and explicit revisions. Exit `1`, `2`, and `3` remain distinct in the job result
  and report.
- `perf-frontmatter` owns its preprocessing comparison and uses a separate sticky marker.
- `perf-reference` checks out the pinned `mermaid-rs-renderer` reference and provides cross-renderer
  context. It does not decide a Merman base/head candidate.
- Manual `reference` and `full` dispatches also run `perf-node-reference` on revisions containing
  the locked Node consumer. Its artifact contains the plan, raw product reports, coverage and
  per-scenario comparisons of the pinned npm releases. A `full` dispatch also replays the native
  `cross_family` catalog in those packages and retains a separate result bundle.

Manual confirmation example:

```bash
gh workflow run performance.yml \
  --ref main \
  -f run=regression \
  -f base_ref=main \
  -f head_ref=my-perf-branch \
  -f preset=long \
  -f suite=standard \
  -f evidence_mode=confirmation \
  -f relative_threshold_percent=10 \
  -f absolute_threshold_ns=50000
```

Manual ASCII confirmation example:

```bash
gh workflow run performance.yml \
  --ref main \
  -f run=ascii \
  -f base_ref=YOUR_BENCHMARK_BACKPORT_SHA \
  -f head_ref=my-ascii-branch \
  -f preset=long \
  -f ascii_suite=comparable \
  -f evidence_mode=confirmation \
  -f relative_threshold_percent=10 \
  -f absolute_threshold_ns=50000
```

Replace `YOUR_BENCHMARK_BACKPORT_SHA` with a revision that contains the byte-identical
`ascii_pipeline` harness, corpus, fixtures,
and preflight contract while retaining the selected base product implementation. Do not substitute
plain `main` unless it already satisfies that identity contract.

Use `-f ascii_suite=closeout` for the five-family medium closeout observation. `run=full` explicitly
selects the ordinary regression, ASCII, frontmatter, and reference lanes; the weekly schedule does
not add ASCII timing.

## Run an ordered workload

Run these commands from the checkout root. `plan` freezes request order, source bytes and SVG IDs
in one portable JSON file. Keep that exact file for every implementation in a comparison.

```console
python tools/bench/perf_runner.py workloads plan --suite quick --out target/bench/plan.json
python tools/bench/perf_runner.py workloads rust --plan target/bench/plan.json --metric memory --site-layout dagre --out target/bench/memory.json
python tools/bench/perf_runner.py workloads rust --plan target/bench/plan.json --metric work --out target/bench/elk.json
```

`mixed-families` alternates a configured Flowchart and an unconfigured Sequence four times.
Compare it with `repeat-flow` and `repeat-sequence`. To investigate site sharing, select
`--scenario retained-configs --operation prepare-retained --warmups 0 --site-css-bytes 65536`.
Repeat with `--operation prepare-drop` and with 8,192 CSS bytes. Both operations use the same
eight requests. A retained result is released after the checkpoint and before the final live-byte
reading. Use `--metric clock` for a separate time observation. Each Rust report records the
executable, embedded measurement-source and plan digests. Automatic builds use a stable per-checkout slot under the Cargo
target directory. For a historical revision, apply the identical benchmark sources in its own
worktree and keep each built executable separately. `--runner PATH --revision SHA` replays a
frozen binary. The source label is supplied by the caller.

Current Rust SVG runners return the verification render to the driver for the same text checks
used by Node and the CLI. Historical runners can replay flat-label plans with their embedded
reader. Use a current harness on both revisions for wrapped labels or alternative spellings.
`compare` classifies a different SVG reader fingerprint as a harness change.

For a single SVG returned to a caller, select a one-request scenario and verify the bytes from
each timed call. Supply the same option for both revisions. Batched clocks and the memory runner
keep the original drop policy and expose a size check for the measured products.

```console
python tools/bench/perf_runner.py workloads rust --plan target/bench/plan.json --scenario small-svg --metric clock --verify-measured-svg --check-drawings --out target/bench/single-return.json
```

```console
python tools/bench/perf_runner.py workloads compare target/bench/base.json target/bench/head.json --out target/bench/diff.json
```

The full plan includes a five-diagram project guide, Zed issue examples, configured multilingual
labels, default and explicit Dagre layouts, mixed families, two equal-length 100-message Sequence
inputs with repeated or unique labels, and 32 topology rows. Five shapes have 50, 100, 250 and
500 nodes. The `scc` and `scc-dag` pair have 25, 50, 75 and 100 nodes. The full render suite
includes the 25-, 75- and 100-node pairs, a fixed 75-node declaration shuffle and a 100-node
reversed-edge statement order for both orientations. To reproduce the original and reordered
100-node sources alongside their orientation control, freeze the desired cases in one plan.

```console
python tools/bench/perf_runner.py workloads plan --scenario scc-100 --scenario scc-100-edges-reverse --scenario scc-dag-100 --scenario scc-dag-100-edges-reverse --out target/bench/cycles.json
python tools/bench/perf_runner.py workloads rust --plan target/bench/cycles.json --metric work --out target/bench/cycle-work.json
```

For all 36 order variants use `workloads plan --suite topology-orders`. Each source is also a
single-request public render case. In a work report, `default`, `high` and
`boundary.minimum_passing_budget` describe the layout writer. `svg.default` and `svg.high`
describe actual public SVG returns, including the digest and work consumed on success or the
first observed resource error. The exact adjacent passing and failing ceilings apply to geometry.

The `config-only-gantt` and `incomplete-flow` scenarios exercise an allowed empty diagram and an
expected parser rejection. A Rust batch containing a rejected request has an untimed outcome
record. The driver writes observed failures along with subsequent scenarios. Add `--require-complete`
when the caller needs a failing exit status for a source that misses its declared outcome. With
`--metric work` this option requires both geometry and SVG to succeed at the default budget.
An incomplete collection or an inconsistent measurement fails independently of that option.

`editor-fixed-id` submits a draft, an incomplete edit and the repaired diagram with the same
SVG ID, `preview-01`. `editor-rotating-id` submits the same three source strings with new IDs.
The Node matrix preserves this order in each engine. The rejected edit stays visible in coverage
and the three-request batch has no render-speed ratio. For a custom document manifest set
`"diagram_id":"preview-01"` to reuse one SVG ID across all its requests.

The full plan also replays the pinned Mermaid examples (nine occurrences) and C4 (six occurrences)
author pages from tracked fixtures. Each manifest links to the original fences and identifies
trailing whitespace differences. To reproduce generated-site weighting for those same nine
examples, select its 18-request manifest.

```console
python tools/bench/perf_runner.py workloads plan --document tools/bench/documents/mermaid-examples-generated.json --scenario mermaid-examples-generated --out target/bench/generated-examples.json
```

The host text measurement guide supplies an integration diagram. The Sequence loop includes
1, 32 and 256 identical messages. Run the three `sequence-loop-messages-*` scenarios separately
to see how end-to-end time and output size grow with the number of messages. The guide exercises
rendering the documented interaction. Measuring callback cost requires a host that invokes it.
`--scenario fan-in-500` when creating the plan also makes that topology available as an ordinary
SVG scenario for the external adapters. A reviewed downstream or Trident document can be imported
with `--document path/to/manifest.json`. For example, a manifest can contain
`{"id":"guide","origin":"project@revision/chapter.md","files":["intro.mmd","example.mmd","intro.mmd"]}`.
Paths are relative to the manifest. Keep repeated occurrences and pin the upstream revision.
Add `"expected_labels":{"intro.mmd":["Start","End"]}` to check that file's reviewed labels
at every occurrence. Use `"expected_outcomes":{"settings.mmd":"allow-empty","incomplete.mmd":"reject"}`
for reviewed configuration or syntax cases. Imports without source facts retain their raw
observations under `unreviewed`.
`--pele-fixtures path/to/mermaid-v13/packages/bench/fixtures/pele` imports the five shared upstream
inputs. Local `.mmd` text is decoded as UTF-8 with LF line endings before hashing.

To reproduce the Node comparison from the repository root, install the locked consumer once and
choose a fresh results directory for each run. This consumer pins Merman N-API, Pele and the
Mermaid v13 package. Node selects the public `mermaid` Node export and awaits each SVG. Default
v13 plugin discovery runs in the same consumer project for the whole comparison.

```console
npm ci --prefix tools/bench/node-reference --ignore-scripts --no-audit --no-fund
python tools/bench/perf_runner.py workloads plan --suite full --out target/bench/full-plan.json
python tools/bench/perf_runner.py workloads node-matrix --plan target/bench/full-plan.json --out-dir target/bench/node-full
```

Open `node-full/summary.md` for the complete-batch medians and how many requests passed in both
products. The adjacent `coverage.json` lists distinct sources, label checks and individual
failure or stability differences. Use `--peer mermaid-v13` or `--peer pele` for a focused run.
`--merman-module PATH` selects a built local N-API or Node-WASM package entry while the peer
versions and working directory stay fixed. A worker exit or timeout leaves an unmeasured row for
that scenario. The collector continues with later scenarios and the matrix still writes comparisons
for the observed intersection. A collector-wide failure leaves an incomplete bundle and its raw
diagnostics. A diagram rejected by a functioning collector is recorded as an observed request
outcome. The bundle includes the exact npm manifest and lock
under `consumer/`. Inspect a plan or existing external comparisons with these commands.

```console
python tools/bench/perf_runner.py workloads coverage --plan target/bench/full-plan.json
python tools/bench/perf_runner.py workloads coverage --plan target/bench/node-full/plan.json --comparison target/bench/node-full/merman-vs-mermaid-v13.json --out target/bench/v13-coverage.json
```

For independent process observations, freeze a small selection and run an even number of rounds.
`--samples` controls the warm batches inside each worker. `--process-rounds` controls the
number of independent workers per product and scenario. The default collection remains a single
pass. Each numbered directory retains its raw observations, comparisons and coverage. Inspect
`process-rounds.json` for every pair and the reason for any withheld ratio.

```console
python tools/bench/perf_runner.py workloads plan --scenario family-sequence --scenario family-pie --out target/bench/round-plan.json
python tools/bench/perf_runner.py workloads node-matrix --peer pele --plan target/bench/round-plan.json --samples 8 --process-rounds 8 --out-dir target/bench/node-pairs
```

For the declared arrows, groups, label widths and Sequence message counts, install the repository's existing Puppeteer
toolchain and pass `--check-drawings`. Use `--chrome` for a known Chrome executable or set
`PUPPETEER_EXECUTABLE_PATH`. The manual Performance reference job uses Chrome from the runner
image and records its version in the audit.

```console
npm ci --prefix tools/mermaid-cli --ignore-scripts --no-audit --no-fund
python tools/bench/perf_runner.py workloads node-matrix --plan target/bench/full-plan.json --out-dir target/bench/node-drawings --check-drawings --chrome /path/to/chrome
```

`drawing-audit.json` contains the evidence for each occurrence and product. `drawings/` contains
only the selected Node outputs. Keep that directory with the raw reports to replay the checks.
The SVG CLI reports refer to files in their recorded `run_dir`. For standalone Node collection,
use `workload_node.mjs --drawing-dir DIR` with the usual plan, engine and sample arguments.

```console
python tools/bench/perf_runner.py workloads drawings --plan target/bench/node-drawings/plan.json --report target/bench/node-drawings/merman.json --out target/bench/merman-drawings.json --require-pass
python tools/bench/perf_runner.py workloads compare target/bench/cli-merman.json target/bench/cli-mmdc.json --drawing-plan target/bench/one.json --out target/bench/cli-checked.json
```

`--require-pass` returns a nonzero status for failed or unavailable facts, or for a plan with no
declared facts. The default matrix records observed differences and withholds affected ratios.
Add `drawing_facts` to an external document manifest by filename, using the same syntax as a
built-in fixture, for example `{"arrows":[{"edge":"L_A_B_0","from":"A","to":"B"}]}`.
The [drawing check contract](BENCHMARKING.md#svg-drawing-checks) describes the supported observations.

To investigate two retained SVGs with different hashes, compare their standalone appearance
in the same offline Chrome. Choose a fresh output directory. `report.json` includes decoded
pixel differences at 1× and 2× in an 800 × 600 CSS-pixel viewport, a same-file repeat, a visible
control and the SVG root bounds. An incomplete view or failed control is `inconclusive`.
Keep the byte stability result alongside the pixel observation.

```console
node tools/bench/workload_svg_pixels.mjs --left first.svg --right second.svg --out-dir target/bench/svg-pixels --chrome /path/to/chrome
```

For a breadth check, `workloads plan --scenario corpus-cross_family --out target/bench/families.json`
replays the native `cross_family` suite in catalog order across the public adapters.
`--scenario corpus-full` selects every registered native fixture. These batches retain the label
facts available for each source and report the other inputs as mechanically checked. Keep intended
error drawings and unsupported families visible in the coverage results.

To exercise the canonical playground example for every family on the Rust, Node or CLI public
entry, freeze the `families` suite. Each family has a separate scenario and can be replayed with
`--scenario family-er`, for example. The `full` suite appends the same family baselines. The
Node summary and coverage JSON report the number tried, completed and repeatable for each peer.

```console
python tools/bench/perf_runner.py workloads plan --suite families --out target/bench/family-baselines.json
python tools/bench/perf_runner.py workloads node-matrix --plan target/bench/family-baselines.json --out-dir target/bench/family-node
python tools/bench/perf_runner.py workloads rust --plan target/bench/family-baselines.json --operation svg-data --metric clock --out target/bench/family-rust.json
```

When adding a family, register its example in `playground/examples/manifest.json` and review the
expected SVG text in `tools/bench/workload_family_labels.json`. Use the fixture owned by the
playground. Include a reason for an empty label list when the source has no portable visible label.

For a package or CLI comparison, install the two releases of interest and run them on one host.
The Node adapter accepts `--module PATH` to select an explicit package entry. The CLI adapter
resolves an installed `mmdc` executable, including its npm shim on Windows. Use `--format png`
on both commands to compare file-to-PNG exports. `--font-file PATH` selects a Merman font;
`--extra-arg=ARG` records explicit CLI arguments, including renderer-specific font configuration.
Install the PNG observer with `python -m pip install -r tools/bench/requirements-png.txt`.
PNG validation decodes each delivered file and records dimensions and pixel variation after timing.
On Windows `peak_working_set_bytes` measures resident memory of the launched process and
`peak_job_commit_bytes` measures peak committed memory across its job, including browser
children. Compare each metric within its own definition.
The exporter also checks labels on an auxiliary SVG. The browser corpus
and its existing visual comparisons remain available separately.

```console
python tools/bench/perf_runner.py workloads plan --scenario small-svg --out target/bench/one.json
node tools/bench/workload_node.mjs --engine merman --plan target/bench/one.json --samples 3 > target/bench/node-merman.json
node tools/bench/workload_node.mjs --engine pele --plan target/bench/one.json --samples 3 > target/bench/node-pele.json
python tools/bench/perf_runner.py workloads compare target/bench/node-merman.json target/bench/node-pele.json --out target/bench/node-comparison.json
python tools/bench/workload_cli.py --engine merman --binary merman-cli --format svg --plan target/bench/one.json --output-dir target/bench/cli-runs > target/bench/cli-merman.json
python tools/bench/workload_cli.py --engine mmdc --binary mmdc --format svg --plan target/bench/one.json --output-dir target/bench/cli-runs > target/bench/cli-mmdc.json
python tools/bench/perf_runner.py workloads compare target/bench/cli-merman.json target/bench/cli-mmdc.json --out target/bench/cli-comparison.json
```

Reference reports show planned, completed and repeatable requests per scenario. Time ratios use
right divided by left. Complete-batch ratios require every request to complete with stable output
in both engines. The report also exposes the fixed common subset and each product's output bytes.
Explicit font settings or different rendering environments retain their observations with the
comparison qualification. Use the [measurement boundaries](BENCHMARKING.md#ordered-public-workloads)
when publishing results.

### Compare native Flowchart layouts

Freeze one-request flat Flowcharts with a reviewed `draw` outcome. `rust-layouts` builds one
release clock executable with `layout-shoal` and uses it for Dagre, ELK and Shoal on the same host.
Each backend receives the frozen source and diagram ID, the default interactive resource policy
and the same warmup and sample counts. Only the site `layout` setting varies.

```console
python3 tools/bench/perf_runner.py workloads plan --scenario small-svg --scenario fan-in-50 --out target/bench/layouts/flat.json
python3 tools/bench/perf_runner.py workloads rust-layouts --plan target/bench/layouts/flat.json --warmups 1 --samples 3 --out target/bench/layouts/comparison.json
```

Each scenario has six scheduled operations. Every backend writes public layout JSON and returns
a public SVG in separate processes. Layout verification checks the public capability plan and
the full geometry digest before and after sampling. Timed layout batches check the byte count.
SVG verification reads the completed render's required capabilities, checks the returned root ID,
and checks each timed SVG against that backend's own digest. A source override that selects
another backend records `layout-backend-mismatch` and skips timing for that batch.

Read `rows.<scenario>` for completion and median nanoseconds per operation. `matching-inventories`
requires the same node IDs, directed endpoints, painted edge occurrences, marker ends and
node/edge label text across backends. Coordinates, paths and byte counts may differ. The check
covers structural agreement on the drawing. Visual quality can be inspected separately.
`runs.<backend>.<operation>` holds the individual receipts, output digests, samples and failures.
The command exits with a failure if any scenario misses the comparison criteria. Treat these
fixed-order medians as diagnostics.

Open an offline HTML gallery to inspect the returned SVGs side by side. Local `.mmd` files add
examples without native timing. `--plan` adds the frozen cases. `--comparison` adds the
SVG medians after checking the plan, executable, backend receipts and SVG digests against the
report. Each unsuccessful backend displays its diagnostic in its own panel.

```console
python3 tools/bench/perf_runner.py workloads layout-gallery fixtures/flowchart/basic.mmd --plan target/bench/layouts/flat.json --comparison target/bench/layouts/comparison.json --out target/bench/layouts/gallery.html
```

Open `target/bench/layouts/gallery.html` in a browser. The file embeds the drawings and source.
Use **Fit** for an overview or **1:1** for synchronized scrolling at the same scale. Each panel
also offers its original SVG for download. For a visual-only comparison, pass one or more
`.mmd` files and the output path.

Add `--pele` to draw every case with the independent Pele Node.js renderer in a fourth panel.
Install the locked Node consumer once. This example includes a file and a frozen directed-cycle case.

```console
npm ci --prefix tools/bench/node-reference --ignore-scripts --no-audit --no-fund
python3 tools/bench/perf_runner.py workloads plan --scenario scc-25 --out target/bench/layouts/cycles.json
python3 tools/bench/perf_runner.py workloads layout-gallery fixtures/flowchart/basic.mmd --plan target/bench/layouts/cycles.json --pele --out target/bench/layouts/gallery-pele.html
```

Pele receives the same source and diagram ID through its public `render()` API. The gallery embeds
the SVG retained by the Node collector after validation and labels it with the installed version
from the locked consumer (`pele@1.13.0`). A render or validation failure appears in that case's
Pele panel. After the first render in each fresh Node.js worker, the collector times three calls
from source to returned SVG. The Pele panel shows their median when the collector admits the
request for timing and all four calls return the exact SVG embedded in the page. Otherwise the
panel says **Untimed reference**. Package import, SVG validation and file I/O fall outside the
Node.js interval. Medians from `--comparison` apply only to the three native panels.
Pass `--runner PATH` to reuse an existing Shoal-enabled clock executable. Opening or sharing
the resulting HTML requires only the file.

Shoal's geometry checks use the final SVG, not Mermaid 12 pixel snapshots. The default flat
fixtures cover all four directions, cycles, self-loops, parallel labels, and dense ports:

```console
cargo nextest run --locked -p merman-render --features diagram-flowchart,layout-shoal --lib -E 'test(graph_layout::validate::gallery::)' --success-output immediate
```

The larger geometry replay is opt-in. Generate the standard full workload plan, then run its
32 topologies; the `fork-join-500` input has 996 edges and must retain its parser rejection.

```console
python3 tools/bench/perf_runner.py workloads plan --suite full --out target/bench/layouts/full.json
MERMAN_SHOAL_STRESS_PLAN="$PWD/target/bench/layouts/full.json" cargo nextest run --locked -p merman-render --features diagram-flowchart,layout-shoal --lib --run-ignored only -E 'test(replay_stress_corpus)' --success-output immediate
```

To inspect the release workflow and dense hub visually without a frozen comparison report:

```console
python3 tools/bench/perf_runner.py workloads layout-gallery crates/merman-render/tests/fixtures/shoal-release.mmd crates/merman/benches/fixtures/flowchart_ports_heavy.mmd --pele --out target/bench/layouts/gallery-v2.html
```

Read fork/join offsets and return distances in SVG units. A foreign-box hit counts a painted
shaft entering a different node or another occurrence's label. Crossings between edges are a
separate measurement. Compare statement permutations within each engine using the minimum
passing budget for the **same operation**; its work units are not milliseconds and are not
interchangeable between engines. Compare end-to-end latency separately on the same host.

For a single Shoal run, `--site-layout shoal` enables the Cargo feature on either the clock or
memory executable and checks each successful SVG or layout JSON request outside measurement.
A frozen `--runner` must supply per-request capability evidence tied to the source digest and
diagram ID. Historical identity-only runners can still replay ordinary workloads. The existing
`compare` command requires identical options and verification outputs. `--metric work` probes
the default ELK layout-work ceiling and accepts only the original default layout.

```console
python3 tools/bench/perf_runner.py workloads rust --plan target/bench/layouts/flat.json --metric memory --operation svg --site-layout shoal --require-complete --out target/bench/layouts/shoal-memory.json
```

To check source precedence, run the Dagre-pinned control under a Shoal site default. The second
command exits with status 1. Inspect `layout_evidence[].backend` for `dagre` and `samples` for
the empty array in the scenario row.

```console
python3 tools/bench/perf_runner.py workloads plan --scenario flowchart-layout-dagre-classic --out target/bench/layouts/pinned.json
python3 tools/bench/perf_runner.py workloads rust --plan target/bench/layouts/pinned.json --metric clock --operation layout-json --site-layout shoal --out target/bench/layouts/pinned-result.json
```

## Publish a reader report

Use `workloads report` to turn completed benchmark evidence into a dated Markdown page and a
portable JSON receipt. [Published comparisons](reports/README.md) live in `docs/performance/reports/`.
Keep exploratory drafts and raw collector bundles in `target/bench/`. The
[Markdown template](../../tools/bench/workload_report.md.in) owns the common product descriptions,
interpretation and section order. Each section uses the host, operation and sample units of its
own inputs. Review the exact product versions and their upstream descriptions on each update.

The following recipe collects the populations used in the November 23 Windows report. Run it
from the repository root with the Node consumer, drawing checks and PNG decoder installed as
described above. Select an installed Chrome executable for the drawing observer and mmdc. The
published run used the Merman CLI 0.11.0 release binary and mmdc 12.2.0. Use a fresh raw directory
for each collection.

```console
python tools/bench/perf_runner.py workloads plan --suite full --out target/bench/readers/full.json
python tools/bench/perf_runner.py workloads node-matrix --plan target/bench/readers/full.json --samples 3 --check-drawings --out-dir target/bench/readers/coverage
python tools/bench/perf_runner.py workloads plan --scenario small-svg --scenario different-flowcharts --scenario scc-75 --scenario family-sequence --scenario mixed-document --scenario mixed-families --scenario four-node-repeat --scenario four-node-relabel --scenario four-node-edges --scenario configured-labels --scenario editor-fixed-id --scenario merman-issue-guide --out target/bench/readers/pairs.json
python tools/bench/perf_runner.py workloads node-matrix --plan target/bench/readers/pairs.json --samples 5 --process-rounds 8 --check-drawings --out-dir target/bench/readers/rounds
python tools/bench/perf_runner.py workloads plan --scenario family-flowchart --scenario family-sequence --out target/bench/readers/cli.json
python tools/bench/workload_cli.py --engine merman --binary merman-cli --format svg --samples 3 --plan target/bench/readers/cli.json --output-dir target/bench/readers/files > target/bench/readers/merman-svg.json
python tools/bench/workload_cli.py --engine mmdc --binary mmdc --format svg --samples 3 --plan target/bench/readers/cli.json --output-dir target/bench/readers/files > target/bench/readers/mmdc-svg.json
python tools/bench/workload_cli.py --engine merman --binary merman-cli --format png --samples 3 --plan target/bench/readers/cli.json --output-dir target/bench/readers/files > target/bench/readers/merman-png.json
python tools/bench/workload_cli.py --engine mmdc --binary mmdc --format png --samples 3 --plan target/bench/readers/cli.json --output-dir target/bench/readers/files > target/bench/readers/mmdc-png.json
python tools/bench/perf_runner.py workloads rust --plan target/bench/readers/full.json --metric work --out target/bench/readers/work.json
```

Use the frozen plans from the report's evidence archive for an exact replay. The original breadth
survey and subsequent process rounds used different revisions of the drawing declarations.
The 12 timed scenarios include three eight-request controls. Each has four nodes, three edges,
equal source byte lengths and the same public SVG ID. `four-node-repeat` repeats the same bytes,
`four-node-relabel` changes node IDs and labels together, and `four-node-edges` changes the labeled
edge set. The first request is identical in all three arms. These controls measure reuse-sensitive
workloads. Establish cache hits separately with implementation evidence.

For each product and scenario, the Node worker records package import and engine creation,
the first successful draw at request index zero, and the continuous interval from import start
through that call's return. Node process startup happens before this clock. Peak RSS includes
the worker runtime, renderer and in-process checks over its cold and warm batches. The worker
reads the high-water mark before fingerprinting product files and excludes validator children.
The report uses independent workers as its sample unit. A direction that repeats in all eligible
rounds describes those rounds. Identify the tested drawing properties alongside the timings.

For the independent browser comparison, use the [browser family corpus](BENCHMARKING.md#browser-comparison-with-mermaidjs)
against an identified built playground. The example below collects Flowchart and Sequence with
six measured blocks per mode. Select the browser SDK version independently of the Node binding.

For a published SDK measurement, start with a fresh export of the matching release tag.
Run `npm pack @mermanjs/web@<version>` and check the archive against `dist.integrity` from
`npm view @mermanjs/web@<version> dist.integrity`. Extract the archive under
`platforms/web/packages/full/`, stripping its leading `package/` directory. Both the Playground
package link and the benchmark's direct WASM glue import resolve there. From the exported root, run:

```console
npm ci --prefix playground
npm ci --prefix playground/tests
npm --prefix playground run build:opaque-realm
npm --prefix playground run verify:opaque-realm
npm --prefix playground run build:prepared
```

Compare the SHA-256 of `playground/dist/assets/merman_wasm_bg-*.wasm` with the WASM entry in the
package's `artifacts/provenance.json`. Record the tag, npm integrity and served WASM digest with the
raw corpus. For a source-build measurement, record the checkout and identify it as a local build.

```console
npm --prefix playground run benchmark:corpus:built -- --fixtures basic-flowchart,sequence-interaction --iterations 6 --warmups 2 --out target/bench/readers/browser.json
```

Generate a report with the measurements you collected. `--date` is the observation date. Use
`--note` to disclose a local candidate build or material host condition. `--raw-url` links to an
already published evidence bundle when available. The portable JSON stores output counts, compact
measurements and fingerprints without machine-specific installation paths. A single supplied
surface is sufficient to generate a report. Empty sections identify unmeasured operations.

```console
python tools/bench/perf_runner.py workloads report --date 2026-11-23 --node-matrix target/bench/readers/coverage --node-rounds target/bench/readers/rounds --cli target/bench/readers/cli.json target/bench/readers/merman-svg.json target/bench/readers/mmdc-svg.json --cli target/bench/readers/cli.json target/bench/readers/merman-png.json target/bench/readers/mmdc-png.json --browser target/bench/readers/browser.json --rust-work target/bench/readers/full.json target/bench/readers/work.json --out target/bench/readers/2026-11-23-windows-x64.md
python tools/bench/perf_runner.py workloads report --from target/bench/readers/2026-11-23-windows-x64.json --check
```

For publication use a new dated output basename, place its `.md` and `.json` together under
`docs/performance/reports/`, and update the [results index](reports/README.md). `--from` renders
the adjacent Markdown from that JSON, and `--from ... --check` verifies its contents across LF and CRLF checkouts without
collecting any measurements. Treat these cross-product observations as inputs for choosing a
workload to investigate. Use the calibrated same-implementation workflow below for a Merman
release performance claim.

Manual `reference` and `full` Performance runs also generate a coverage-only reader report
under `node-reference/reader/` inside the `perf-node-reference` artifact. That page links to
the run containing the raw observations. Curated snapshots enter the repository through review.

## 1. Freeze the question and revisions

Write down the claim before measuring:

- public operation and lifecycle, such as reused-engine `end_to_end/flowchart_medium`;
- exact base and candidate commits;
- fixture bytes and output/semantic gates;
- relative and absolute minimum detectable effects;
- fixed maximum pair budget;
- relevant control fixtures and memory/resource bounds.

Use adjacent clean committed revisions to prove one optimization. A broad release comparison such as
`v0.8.0-alpha.3` to current head is useful historical context, but unrelated semantic and capability
changes prevent it from proving causality.

Create or identify two non-overlapping checkouts without changing either during the run. Do not use
a dirty working tree as a candidate receipt.

## 2. Declare both runner recipes

Never assume that base and head share a feature vocabulary or benchmark configuration. Declare each
side's checkout, label, package, bench, features, default-feature policy, toolchain, target, target
directory, and corpus.

For the current full `pipeline` bench, explicitly enable
`all-diagrams,svg,compat-json,editor-facts`; selecting only an SVG timing group still requires the
compatibility and editor gates on the executable. The default facade does not enable
`compat-json`. The Performance workflow reads the base recipe from the base checkout's
`tools/bench/performance_lanes.json` and the head recipe from the head descriptor. Keep these
independent for historical refs that lack the new feature names.

The October 22, 2026 U8/U9 change is an API/recipe migration, not a new measurement or a
performance pass. Current contracts refresh source input hashes and required features while
retaining all metric thresholds, candidate-admission flags, and semantic output digests.
Preserve ignored historical reports and frozen baselines. After the final
`crates/merman/Cargo.toml` or `Cargo.lock` edit, refresh their `probe.inputs` hashes in
`flowchart-svg-label-artifact-memory-v1.json`, `sequence-message-repeated-memory-v1.json`, and
`sequence-message-unique-memory-v1.json` before checking current contracts.

For the alpha.3 historical context, the essential capability difference is explicit:

```bash
python3 tools/bench/compare_self.py \
  --base-dir ../merman-alpha3 \
  --head-dir . \
  --base-label v0.8.0-alpha.3 \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench pipeline \
  --head-bench pipeline \
  --base-features render \
  --head-features all-diagrams,svg,compat-json,editor-facts \
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

Default features are enabled independently for both sides. Use `--no-base-default-features` or
`--no-head-default-features` only when the selected suite does not require a disabled family;
missing capability for a mandatory fixture is a contract failure. Corpus schema-v2 lane metadata
validates the public operation, lifecycle, selector history, and normalization divisor. An explicit
`--base-logical-operations` or `--head-logical-operations` value must match that metadata. Legacy
schema-v1 confirmation keeps both divisors at one and requires exact benchmark identity. Use the
same recipe shape for an adjacent candidate comparison. For two current revisions, explicitly set
both feature lists to `all-diagrams,svg,compat-json,editor-facts`; older bases retain their own
feature vocabulary.

## 3. Establish the evidence contract

Before accepting a timing sample, verify that the report proves all of the following:

1. Both Cargo builds were locked, forced to `CARGO_BUILD_JOBS=1`, recorded that environment in each
   runner receipt, and completed before sampling.
2. Each Cargo JSON stream resolved exactly one requested Criterion executable.
3. Executable digests remained unchanged through discovery and sampling.
4. The exact benchmark exists on both sides.
5. Both corpus files were loaded independently.
6. Each required fixture is available on both sides and byte-identical.
7. Both sides measure the same logical operation and each side's normalization divisor is accurate.

The harness invokes the frozen executables directly for discovery and every timed pair. Cargo must
not run in the background while sampling. A missing or mismatched item above is exit `2`; do not
interpret it as a performance outcome.

Distinct-target and shared-target prebuilds both force one Cargo build job. If both recipes
deliberately share one target directory, pass `--freeze-shared-target`. That mode additionally runs
`cargo clean --profile bench` before each side, rebuilds it, and freezes the resulting executable
before the next reset. The profile reset is part of the provenance contract and prevents Cargo from
reusing an executable built from the other path-workspace checkout. It does not clean the debug
profile. Ensure no other Cargo process is using the shared target for the whole run.

## 4. Use diagnostic mode for triage

Run a one-to-four-pair alternating AB/BA schedule when the goal is only to decide where to
investigate. Even pair counts are order-balanced:

```bash
python3 tools/bench/compare_self.py \
  --base-dir ../merman-base \
  --head-dir . \
  --base-label base \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench pipeline \
  --head-bench pipeline \
  --base-features all-diagrams,svg,compat-json,editor-facts \
  --head-features all-diagrams,svg,compat-json,editor-facts \
  --base-toolchain 1.95.0 \
  --head-toolchain 1.95.0 \
  --base-target-dir target/performance/base \
  --head-target-dir target/performance/head \
  --base-corpus tools/bench/corpus.json \
  --head-corpus tools/bench/corpus.json \
  --suite canary \
  --evidence-mode diagnostic \
  --pairs 4 \
  --out target/performance/diagnostic.md \
  --json-out target/performance/diagnostic.json
```

Timing movement in this mode is advisory and returns `0`. Contract failure still returns `2`.
Do not accept, reject, or retain candidate code from diagnostic evidence.

When attribution is needed, run owner-local stages only after the public lane moves:

```bash
python3 tools/bench/stage_spotcheck.py \
  --preset long \
  --fixtures flowchart_medium,class_medium \
  --out target/bench/stage_spotcheck.md
```

Use flamegraphs after stage attribution identifies a CPU owner:

```bash
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --profile bench \
  -p merman --features svg --example profile_render \
  -o target/bench/flamegraphs/profile_render_architecture_medium.svg -- \
  --input crates/merman/benches/fixtures/architecture_medium.mmd \
  --stage render --seconds 20
```

The example's `render` stage parses once and prepares a fresh artifact before each SVG emission,
since rendering consumes the artifact. Whole-process flamegraphs, macOS `sample` output, Instruments
profiles, and the reported elapsed time therefore include preparation. For emission attribution,
inspect the subtree rooted at `profile_render::emit_prepared_svg`; this non-inlined frame contains
SVG emission and output consumption, while preparation stays outside it. That is the operation
boundary timed by Criterion's `render/*` group. Compare emission costs within that subtree rather
than treating the whole-process profile as an emission-only measurement.

## 5. Run confirmation without adaptive sampling

This section admits latency and throughput claims. For a structural complexity or
input-amplification repair, use the evidence model in `BENCHMARKING.md`: preregister named input
variables, reachable old/new time and space bounds, and an exact work counter or scale curve. Keep
public timing as a non-regression control and do not manufacture a speedup claim from an
asymptotic proof.

Freeze the confidence level, bootstrap seed/resample count, minimum detectable effects, calibration
count, and maximum pair budget before looking at the result. The default contract is:

- at least eight balanced base A/A pairs and eight balanced head A/A pairs;
- two-sided A/A identity and order-effect intervals contained by the preregistered equivalence
  margins;
- a power-derived even AB/BA count, at least eight and no greater than the registered cap;
- fresh confirmation observations that do not reuse diagnostic or calibration data;
- suite-level simultaneous 95% paired bounds for `log(head/base)` and `head - base`, using
  Bonferroni component confidence across both metrics and every comparable row;
- one preregistered materiality contract: the default joint `>10% AND >50 us` reporting budget, the
  low-latency public-operation formula from `BENCHMARKING.md`, or a documented interactive,
  throughput, high-volume, memory, or structural objective whose bounds dominate A/A noise.

Use `--confidence-level`, `--bootstrap-seed`, and `--bootstrap-resamples` when an experiment needs
explicit reproducibility values. Confirmation requires at least 10,000 bootstrap resamples;
smaller values are diagnostic-only, and values above 100,000 are rejected to keep evidence
generation bounded. Schema-v2 confirmation resolves current and registered historical selectors to
one public operation and takes its divisor from lane metadata. Legacy schema-v1 confirmation instead
requires exact benchmark identity and divisors of one. If A/A is unstable, the required count
exceeds `--max-pairs`, or a decision interval crosses the registered materiality boundary, record
the result as inconclusive. Do not add samples after seeing the classification or relax the
registered contract. Exploratory owner-local results may motivate the next experiment's contract,
but they must be disclosed and cannot be reused in A/A calibration or fresh public confirmation.

Structural and memory candidates follow the same preregistration discipline but do not manufacture
a latency claim. Exact removal of repeated work, improved reachable complexity, or reduced
allocation/retention may be accepted with semantic parity and public non-regression controls. New
caches, persistent state, protocol machinery, or broad abstractions require stronger measured
benefit proportional to their maintenance and correctness risk.

## 6. Run semantic and resource gates

Focused owner tests are appropriate during iteration. Before accepting shared-path code or a durable
checkpoint, run the repository's relevant complete gates serially. The broad parity commands are:

```bash
cargo run -p xtask -- verify --strict
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout
```

The exact gate set depends on the owner, but must include applicable semantic/model, layout, SVG,
error precedence, resource-limit, security, host-callback, deterministic-output, and control-fixture
contracts. Native allocation count/bytes and peak live growth use the separate instrumented memory
harness; do not place allocator instrumentation in the latency executable.

### Calibrate a layout-work ceiling

Any change to a public layout-work ceiling requires a clean release build and the fail-closed
calibration wrapper:

```bash
CARGO_BUILD_JOBS=1 cargo build --locked --release -p merman \
  --example layout_work_calibration --features svg,compat-json,layout-cytoscape,layout-elk,math

python3 tools/bench/run_layout_work_calibration.py \
  --authoritative-date YYYY-MM-DD \
  --resource-profile trusted-native --theme default \
  --out-dir target/bench/layout-work-calibration-YYYY-MM-DD-default \
  --timeout-seconds 300 \
  --full-repeats 5
```

Run a second independent output directory with `--theme dark`. The probe defaults to the
historical interactive profile; select the profile being calibrated explicitly. A candidate
`--layout-work-limit N` overrides only that dimension and is recorded in every report. Headroom is
`W + max(100,000, ceil(W / 10))`, rounded upward to 100,000 units for interactive or 1,000,000 units
for trusted-native. This is a policy margin over the closed corpus, not a bound for arbitrary graphs.

The native `pipeline` layout, render, and end-to-end benchmarks explicitly use trusted-native,
including the matching input policy for the public end-to-end operation. Older receipts using
interactive remain historical; compare the same selected policy when claiming latency changes.
Successful SVG bytes are unchanged by this policy choice. The full/stress suites include the two
nested-Class controls; the canary, quick, standard, and cross-family selections are unchanged.

Windows uses a Job Object with a gated launcher for descendant cleanup and reads the target
process's actual peak working set while its handle remains open. The legacy
`maximum_resident_set_size_bytes` field is labeled `windows-peak-working-set` in that lane;
do not compare it directly with Linux/macOS RSS. Outer elapsed time includes launcher overhead.

The source probe owns the closed corpus and registered headroom rule. The wrapper must bind five
byte-identical full reports, isolated semantic/layout/SVG/end-to-end paths, the exact corpus
maximum at `W` and `W-1`, and a deterministic node/edge boundary proven by a complete sequential
accepted-prefix scan or reviewed monotonicity proof. It must use a managed process group so timeout
terminates descendants independently of leader exit or pipe closure, and the timeout regression
suite must pass in the performance-contract CI. Require an empty evidence directory and bind typed
failure payloads, timing-file byte lengths, peak RSS, and output hashes. Commit a compact evidence
manifest under `docs/performance/evidence/` that binds the ignored raw summary by byte length and
SHA-256, then link a dated decision receipt from `PERF_PLAN.md`. Record the result as
`accepted-structural` only; stage elapsed and RSS observations do not create latency, memory, or
host-SLO claims.

The current policy receipts are the [interactive calibration](interactive_layout_work_calibration_2026-10-01.md)
for the general library/Web default and the [native calibration](native_layout_work_calibration_2026-10-01.md)
for CLI/rustdoc. The [2026-08-07 interactive receipt](interactive_layout_work_calibration_2026-08-07.md)
remains historical for its recorded backend and corpus; it does not establish current Mermaid 12
coverage at 800,000.

Run the native memory harness only when the owner unit registers that gate:

```bash
python3 tools/bench/run_native_memory.py \
  --json-out target/bench/native_memory.json
```

It prebuilds one instrumented executable, then launches 60 fresh subprocesses by default: matched
operation/zero samples across six fixed scales and five repeats. Treat exit `2` as a protocol or
provenance failure, `1` as a failed bound, `3` as inconclusive, and `0` as passing only the declared
owner contract. The checked-in Flowchart contract is explicitly an `infrastructure-smoke` contract
with broad safety limits and cannot admit a candidate. A candidate unit must freeze tighter bounds
before collecting its adjacent A/B evidence. Use `perf_runner.py --include-native-memory` only when
this explicit cost is intended; no normal latency profile enables it automatically.

For a durable baseline, run the full harness from a clean committed checkout, then freeze its JSON
as the authoritative evidence envelope:

```bash
python3 tools/bench/freeze_perf_baseline.py freeze \
  --native-memory-report target/bench/native_memory.json \
  --out docs/performance/baselines/runtime-<source-commit>.json
```

The new manifest makes the checkout dirty only after it has captured the clean source. Commit the
manifest separately. To verify later, use a clean checkout of the manifest's recorded source commit
as `--repo-root` and pass the manifest from the later receipt commit. Verification rehashes the
source, fixtures, report, executable, and any ordered patch stack, then recomputes native-memory
analysis rather than trusting persisted classifications.

## 7. Interpret schema-v2 evidence

Read the JSON artifact before the Markdown summary. Confirm that it retains:

- both runner recipes, revisions, locks, manifests, toolchains, targets, build commands, host state,
  and executable digests;
- two-sided corpus/fixture byte provenance and capability coverage;
- method, thresholds, logical-operation divisor, pair budgets, confidence method, and deterministic
  bootstrap settings;
- every ordered base/head A/A and fresh AB/BA pair;
- stability and order diagnostics, power-derived required count, paired one-sided relative and
  absolute bounds, and both regression and mirrored improvement classifications.

Interpret process exits with fixed precedence:

| Exit | Action |
| ---: | --- |
| `2` | Repair the evidence contract; there is no performance conclusion. |
| `1` | Treat the suite as a confirmed regression and investigate or revert the candidate. |
| `3` | Preserve the receipt and retest conditions; do not retain unproven candidate code. |
| `0` | Diagnostic completed, or confirmation established suite-wide non-regression. Inspect mirrored improvement fields before candidate admission. |

Candidate admission uses the mirrored improvement bounds, not exit `0`. Accept only when both the
relative and absolute improvement thresholds are confirmed and every mandatory non-performance gate
passes. Reject when improvement is disconfirmed or a mandatory gate fails. Keep an explicit queue
entry and next fixed budget only for a genuinely inconclusive hypothesis.

## 8. Store the receipt

- In-flight Markdown and JSON: `target/performance/` or `target/bench/`.
- Durable adjacent-candidate decision: a concise dated Markdown receipt under `docs/performance/`,
  with the schema-v2 JSON retained as a build artifact.
- Durable cross-renderer context: `docs/performance/renderer_comparison_YYYY-MM-DD.md`.

Never rewrite an old report's recipe to current feature names. Historical evidence must preserve the
exact capabilities, fixture hashes, tools, and revisions used when it was collected.

## Harness contract checks

Run these checks after changing the benchmark infrastructure:

```bash
python3 -m py_compile \
  tools/bench/compare_mermaid_renderers.py \
  tools/bench/compare_self.py \
  tools/bench/corpus_utils.py \
  tools/bench/freeze_perf_baseline.py \
  tools/bench/native_memory.py \
  tools/bench/perf_runner.py \
  tools/bench/render_perf_comment.py \
  tools/bench/run_native_memory.py \
  tools/bench/stage_spotcheck.py \
  tools/bench/test_native_memory_contracts.py \
  tools/bench/test_native_memory_driver_contracts.py \
  tools/bench/test_perf_baseline_manifest.py \
  tools/bench/test_perf_contracts.py \
  tools/bench/verify_pipeline_bench_list.py
python3 tools/bench/test_native_memory_contracts.py
python3 tools/bench/test_native_memory_driver_contracts.py
python3 tools/bench/test_perf_baseline_manifest.py
python3 tools/bench/test_perf_contracts.py
python3 tools/bench/verify_pipeline_bench_list.py
python3 tools/bench/run_native_memory.py --smoke \
  --json-out target/bench/native_memory_smoke.json
python3 tools/bench/test_native_memory_driver_contracts.py \
  --verify-smoke-report target/bench/native_memory_smoke.json
```
