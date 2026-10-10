# Theme path retirement: local verification and measurements

The unified terminal-theme implementation is complete locally. It removes the
production `SvgTheme`/`MermaidThemeAdapter` closure and historical retirement
authorization machinery. Mermaid input and derivation remain in the core;
renderer-owned family artifacts bind winning visual properties before emission.
This is an ownership and maintenance refactor. These measurements do not establish
a latency, allocation or artifact-size improvement.

The [implementation plan](../plans/2026-10-09-1500-refactor-theme-path-retirement-plan.md)
remains open for the affected remote platform CI matrix. Local tests do not attest
other operating systems, SDKs or packaged bindings. No branch was pushed for this
verification, and no release was performed.

## Revisions and controls

- Baseline: `f8160267a3e905eb2d9d6c0b0c980d9fa1f07657`.
- Measured head: `17da680e9871e7259f71f9485c5738dd3056302d`.
- Runtime implementation: `3a11f46bad53209b0951abc0695bf971f8f3bc31`;
  subsequent changes repair documentation comments only.
- Host: Apple M4 Pro, macOS 27.0, arm64; Rust 1.95.0 / LLVM 22.1.2;
  Python 3.14.8. Cargo builds and performance sampling ran serially.
- Two clean ordinary clones, separate from registered development worktrees,
  provided fixed source revisions. Cargo used locked dependency inputs.
- Benchmark, corpus and fixture trees are identical. Their Git tree IDs are
  `eb60b2644508cba59d98a6a8eb46970da5b51419` (tools/bench),
  `516d57f4df88642190fed11e54dd60415159a2c0` (library benches), and
  `f11c4bf868a0dc78523227b8e2913287d7b74224` (fixtures).
- No package version or checksum changed. The exporter's obsolete optional
  `sha2` dependency edge was removed; this is a disclosed closure change, not a
  hidden reduction of supported outputs.

Raw receipts and their lossless gzip digests are indexed in
[the evidence manifest](evidence/theme-path-retirement-2026-10-10/manifest.json).
Decompress each archive to recover the original receipt bytes and schema.
The local experiment directory is
`target/bench/experiments/theme-retirement-20261010-3a11f46ba`; it also retains
frozen executables, artifact copies, build logs and unsuccessful attempts.

## Native render latency

The existing `compare_self.py` pipeline comparator measured public native
`render-svg` operations with default features disabled and
`svg,all-diagrams,layout-cytoscape,layout-elk,math` enabled. This is not CLI cold
start, typed-preset qualification, native export or browser timing.

The registered long preset uses 30 Criterion samples, two-second warmup,
three-second measurement, eight A/A calibration pairs per side, a 32-pair cap,
10,000 bootstrap resamples and seed zero. Decisions require both a 10% relative
and 50-us absolute threshold, with simultaneous 95% confidence and Bonferroni
adjustment across 18 components. Thresholds were not changed after sampling.

| Operation | Baseline | Head | Simultaneous relative bounds | Decision |
| --- | ---: | ---: | ---: | --- |
| Flowchart medium | 1.53 ms | 1.55 ms | +1.02% to +1.61% | Confirmed non-regression at registered thresholds |
| Flowchart SVG label reuse | Not admitted | Not admitted | A/A unstable | Inconclusive |
| Class medium | 1.19 ms | 1.18 ms | -0.84% to -0.05% | Confirmed non-regression |
| Sequence medium | 535.66 us | 531.90 us | -1.13% to -0.26% | Confirmed non-regression |
| Requirement medium | 405.92 us | 403.62 us | -1.01% to -0.11% | Confirmed non-regression |
| Mindmap medium | 389.01 us | 387.46 us | -0.92% to +0.21% | Confirmed non-regression |
| GitGraph medium | 80.49 us | 80.94 us | -0.09% to +1.37% | Confirmed non-regression |
| Architecture medium | 79.75 us | 80.11 us | -0.08% to +1.05% | Confirmed non-regression |
| Info medium | 19.10 us | 18.84 us | -2.09% to -0.66% | Confirmed non-regression |

All nine discovery outputs have equal SVG identities. The eight stable rows each
received 14 fresh balanced AB/BA pairs. Label reuse failed absolute-margin A/A
identity and order calibration and received no admissible A/B conclusion.
The overall result is **inconclusive, exit 3**, with zero contract failures,
zero confirmed regressions and zero confirmed improvements. A sub-threshold
increase is not a speedup; these results do not certify every workload.

Frozen executable SHA-256:

- Base: `fad82a724015f59e9478137b24147297cbbd3821369a06a6015e85068637571c`.
- Head: `13fac68811e4a0675a8da492e71b848781ca393825feb6b4b8977d6b67cda3dd`.

An initial confirmation command combined mutually exclusive discovery-reuse and
target-freezing arguments and exited before sampling. The corrected command
reused the already verified frozen runners. Both attempts remain recorded.

## Native allocation observations

The existing `flowchart-end-to-end-memory` owner completed all six scales with
five repetitions and matched zero-operation controls on each side. Both satisfy
the existing infrastructure smoke caps; `candidate_admission=false`. This is
allocator evidence for that operation, not a formal relative optimization
admission or a measure of total WASM heap.

| Scale | Allocated bytes base -> head | Allocation count base -> head | Peak live growth bytes base -> head |
| ---: | ---: | ---: | ---: |
| 1 | 1,357,333 -> 1,378,666 | 8,596 -> 8,629 | 286,293 -> 286,293 |
| 2 | 1,974,019 -> 2,011,946 | 12,930 -> 12,975 | 415,634 -> 419,538 |
| 4 | 3,340,653 -> 3,422,042 | 22,357 -> 22,446 | 674,752 -> 680,406 |
| 10 | 7,329,685 -> 7,625,016 | 50,392 -> 50,601 | 1,460,446 -> 1,470,838 |
| 32 | 60,980,961 -> 61,675,372 | 334,732 -> 335,317 | 14,001,153 -> 14,031,373 |
| 100 | 349,889,357 -> 352,366,576 | 1,661,943 -> 1,663,643 | 85,301,369 -> 85,398,461 |

Allocated bytes increase by 0.71% to 4.03%; peak live growth increases by at most
0.94%. Prepared terminal records have a cost. These observations do not isolate
which record causes each delta and do not establish allocation reduction.

The additionally registered `sequence-message-repeated-memory` lane rejected
both revisions before compilation: `owner contract probe input digest differs
at index 0`. No Sequence samples or theme-memory comparison were produced.
The probe and driver are identical across these revisions. Its September 26
contract predates October 2/5 probe edits and changed manifest/lockfile inputs.
Moreover, an August 13 change replaced prepared-render measurement with semantic
artifact construction; the current probe does not execute layout or SVG.
Refreshing hashes alone would not restore the historical measurement meaning.

A separate follow-up should give the semantic lane an accurate name and contract,
retain old candidate budgets as historical evidence, and add a public Sequence
`render-svg` allocator workload if theme-memory claims are needed. It should reuse
the current counting allocator and process protocol, without a source-analysis
framework or automatic contract authorization.

## CLI artifact size

Both sides used the same `cli-release` artifact feature closure, Rust 1.95.0,
`aarch64-apple-darwin`, `dist` profile and thin LTO. Raw binaries were copied;
`strip -x` operated on copies, followed by gzip level 9 with `mtime=0`.
The full command and hashes are in `cli-receipts.json` in the evidence archive.
The common explicit features are `all-diagrams,analysis,ascii,icons,jpeg,
layout-cytoscape,layout-elk,markdown,math,network-icons,parallel-markdown,pdf,png,
rustdoc,shell-completions,svg,system-clock,system-random,system-timezone,system-timing`;
default features are disabled.

| Metric | Baseline bytes | Head bytes | Change |
| --- | ---: | ---: | ---: |
| Raw | 53,664,496 | 54,205,056 | +1.01% |
| Stripped copy | 46,796,752 | 47,277,648 | +1.03% |
| Gzip-9 of stripped copy | 20,228,933 | 20,412,009 | +0.91% |

The normalized normal package/version closure is identical: 441 unique rows.
No dependency addition explains this increase. This comparison cannot attribute
all bytes to a particular terminal record without symbol-level analysis.
There is no checked-in CLI byte ceiling in this experiment and no size-reduction
claim.

The first head attempt reused the base target cache and failed on stale internal
API metadata. An independent clean head target built successfully; only that
artifact is measured. Both build logs are retained. The source was not changed
to accommodate the cache failure.

## Browser WASM size

The two clean clones use the actual `build-wasm.mjs --package full` owner,
independent target directories, Rust 1.95.0, wasm-pack 0.15.0 and the CI-pinned
wasm-opt 131. Both disable default features and enable the same `web-full`
features: `all-diagrams,analysis,ascii,editor,layout-cytoscape,layout-elk,math,svg`.
These are wasm-bindgen web artifacts, not raw Cargo WASM or Typst modules.

The size comparison uses wasm-tools 1.253.0 and Brotli 1.2.0. It follows
`wasm-tools strip --all`, Python gzip-9 with
`mtime=0`, and Brotli CLI quality 11 / window 22 on stripped copies. This is
distinct from the release owner's Rust compression implementations; release
budget acceptance is recorded separately below.

Independent size measurements are recorded in `web-receipts.json`.

| Metric | Baseline bytes | Head bytes | Change |
| --- | ---: | ---: | ---: |
| Raw | 16,335,330 | 16,418,678 | +0.51% |
| Stripped copy | 16,335,065 | 16,418,413 | +0.51% |
| Gzip-9 of stripped copy | 6,098,284 | 6,127,313 | +0.48% |
| Brotli-11 of stripped copy | 4,509,566 | 4,529,971 | +0.45% |

The independent head artifact remains below every registered web-full byte
ceiling. It differs from the previously rebuilt workspace package reported below;
the independent clone comparison is the size-delta evidence. This is not a
byte-for-byte reproducible-build assertion, and the workspace package's smaller
bytes must not substitute for the independent head result.

The already rebuilt assembled head package passed the unchanged official owner:

```text
target/release/xtask wasm-size-matrix --artifact-profile web-full \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

Its raw/stripped/gzip/Brotli bytes were
16,314,460 / 16,314,195 / 6,098,623 / 4,510,303, below the existing
16,804,000 / 16,804,000 / 6,282,000 / 4,646,000 ceilings. The budgets were
not widened. This verifies `web-full`, not all six release profiles; complete
release profiling remains a publication requirement.

## Behavior and completion boundaries

The plan records all final local owner receipts: 12,241 workspace tests,
4,864 renderer tests, 45 feature builds and isolated consumers, native export,
private acceptance, strict Clippy, docs, dependency/legal owners and public
examples. All 77 fresh Chromium theme/workspace tests passed. Optimized evidence
and qualification suites passed, as did full SVG structure/parity/parity-root.

The final root-containment recheck covers 3,712 SVGs with zero blocking failures,
54 browser-owned diagnostics, 2,344 upstream-inherited cases, one existing exact
residual and zero unused residuals. The original full run had an external-image
decode failure; focused successful decoding and the complete unchanged recheck
are retained alongside it. No comparator, residual or source fixture was changed
to make this pass.

The final static audit covers all 35 writer families, including Error, with no
remaining late production raw-theme winner selection. Serialization, CSS/URL
safety, geometry-dependent effect realization, emission evidence and native
receipts remain live contracts. The current matrix invariant traverses actual
classification; no universal source-scanning gate was added.

Local implementation and evidence are ready for review. Overall Goal completion
still requires the affected remote feature/platform CI matrix. A/A uncertainty,
the unavailable Sequence memory lane and unmeasured release profiles remain
explicit limitations rather than successful checks.

As a final environment probe, the CI script discovery command ran 704 tests
locally. Five release-registry tests could not bind their loopback HTTP fixture
(`PermissionError: Operation not permitted`); the failures occurred before their
assertions and are caused by this sandbox's socket policy. The prior repository
script gate receipt remains 582/582 passed. This probe is retained as an
environment limitation and is not presented as a green CI result. After socket
permissions were restored, the same unchanged command passed all 704 tests in
13.021 seconds. Both logs are archived. Git write permission was also restored;
the earlier inability to stage this report is no longer a blocker.
