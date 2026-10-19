# Root viewport state after emission — macOS, 2026-10-18

This checkpoint removes ownership of unused root viewport strings after root attributes have
been written. Its adjacent base is `9db3619a5` (runtime equal to `d4b495cea`). Admission is exact
public-operation allocation reduction with same-output and resource controls, not a calibrated
latency claim.

## Ownership boundary

The complete `RootViewportPlan` remains the pre-write plan: it validates geometry, family and id,
selects responsive/intrinsic/fixed sizing, supplies root attributes and supports ER title placement.
After ordinary writing, the old `Ready` state cloned that entire plan, including diagram id,
max width and optional width/height/style Strings. It then moved the plan into `RootedSvg`.
All subsequent consumers used only `view_box()` for root canvas preparation, length preflight
and final serialization; the accessor simply copied the optional geometry field.

`Ready` and `RootedSvg` now retain only `Option<ViewBox>`. That preserves `None` and the complete
original origin/width/height, without reconstructing geometry from SVG text. The deferred path
still constructs the complete plan and finishes all sizing/placeholder/snapshot checks and bounded
replacements before retaining `Some(view_box)`. It already moved its plan without cloning, so the
change does not claim an equivalent allocation saving on that path.

Root snapshots, family/id provenance, incomplete-document checks, deferred replacement ordering,
geometry validation, theme resource-id collision checks, exact byte preflight, cancellation and
sticky first errors remain. The existing style clone for custom-property composition remains
separate. No output fragments, XML encoding, resource observation or public field changes.
The single affected test is mechanically adapted to the private field representation; no assertion
or test is removed or weakened, and no new test matrix is added.

## Fresh attribution and registered controls

The unchanged current `profile_render` example links against verified optimized libraries.
A 12-second Zenuml render process, batch 100, receives a six-second macOS `sample` capture at 1 ms.
Only the `emit_prepared_svg` subtree is emission; whole-process render includes fresh preparation.
The example uses its deterministic interactive resource profile, distinct from the pipeline's
trusted-native timing recipe.

Root protocol frames contain 349 of 2,782 emission-subtree samples (12.54%); plan clone frames
contain 21 (0.75%), already included in the root category. These inclusive percentages are not
removable CPU estimates or a causal decomposition of the v0.9.0 gap. They support checking a
small concrete ownership reduction rather than promising a large public latency recovery.

Before production edits, the public corpus extends the preceding 86 cases with all 76 native
pipeline SVG inputs, 12 solid/transparent canvas cases across ordinary/deferred/no-viewBox families,
and four `useMaxWidth:false` site configurations: 178 inputs total. Reused frozen baseline probes
and the selected current source/artifact hashes are recorded. The canvas authoring definitions
and source/site-config hashes are frozen before any production edit. Fixed-configuration rows
record the rendered result rather than assuming every family honors the same configuration key.

The initial extra-family capture reveals pre-existing cumulative-allocation variation in a
Flowchart case: 3,638,846 versus 3,638,855 bytes across five captures, with unchanged count/peak/
retained. Before editing production, the ledger registers how to handle this new broad lane:
the established 86 stable-input gates remain unchanged; every new case keeps all five samples,
its median and a stability flag, and variable memory rows remain inconclusive. No unstable row
is dropped or retried to reach stability. All 178 SVG/report/custom-measurer comparisons remain
mandatory. The first stopped capture log was overwritten by the subsequent complete baseline. Only its
observed byte values and failure description survive in the ledger; all five samples per input
from the completed baseline are preserved.

The frozen public allocation probe performs five complete operations per input. Across the 178
paired inputs, all SVG bytes, resource/theme reports and ordered custom-measurer traces match.
The independent pipeline discovery also matches all 152 render/end-to-end SVG identities.

171 inputs have identical memory samples within each executable: 156 reduce allocation count and
cumulative bytes, 15 remain unchanged. Of those stable rows, 56 reduce peak growth and 115 leave
it unchanged; all retain exactly the same bytes. The established 86-input subset stays fully stable
and passes its original nonincrease gates (85 reduce allocation, one is unchanged).

| Stable public operation | Allocations, base → head | Cumulative bytes, base → head | Peak-growth delta | Retained delta |
| --- | ---: | ---: | ---: | ---: |
| Default Zenuml | 471 → 467 | 152,840 → 152,794 | 0 | 0 |
| Packet | 134 → 130 | 19,357 → 19,315 | -42 | 0 |
| Timeline | 795 → 791 | 117,794 → 117,754 | -40 | 0 |
| Kanban | 665 → 661 | 116,170 → 116,130 | -40 | 0 |
| Sequence | 4,640 → 4,636 | 747,693 → 747,651 | -42 | 0 |
| ER | 6,877 → 6,873 | 1,227,786 → 1,227,740 | -46 | 0 |
| Journey | 457 → 452 | 80,693 → 80,648 | -45 | 0 |
| State / Block / Class medium | unchanged | unchanged | 0 | 0 |

Seven new Flowchart inputs have variable memory samples on at least one side: weave, ports-heavy,
fanout-returns and nested-clusters under Dagre Classic, plus backedges/subgraphs, long-edge-labels
and direction-conflict. Their complete samples and medians remain in the record; their memory
comparisons are inconclusive, not allocation wins. Their SVG/report/measurer checks still pass.
No variability retry was performed. All solid/transparent canvas cases remain stable and preserve
output. The fixed-setting Zenuml, ER and Timeline cases actually emit intrinsic dimensions; Packet
still emits responsive dimensions with the selected setting. Its unchanged configuration behavior
is preserved rather than counted as fixed-sizing coverage.

## Diagnostics and release context

Ten adjacent rows cover prepared SVG emission and complete public E2E for Zenuml, Packet,
Timeline, State and ER. Each row uses two AB/BA pairs, 30 Criterion samples, 2 s warm-up and
3 s measurement per invocation: 40 invocations total, all with equal preflight SVG identities
and verified postflight receipts. The fixed material-regression screen (both >10% and >1 us
on public E2E) passes every selected family. These diagnostics have no A/A calibration.

| Family | Public E2E, base → head (us) | E2E change | Prepared SVG, base → head (us) | Emission change |
| --- | ---: | ---: | ---: | ---: |
| Zenuml | 24.4630 → 24.2320 | -0.94% | 7.5661 → 7.5910 | +0.33% |
| Packet | 11.3105 → 11.2790 | -0.28% | 4.4040 → 4.4101 | +0.14% |
| Timeline | 44.0400 → 44.1680 | +0.29% | 15.1865 → 15.2330 | +0.31% |
| State | 437.0450 → 436.9900 | -0.01% | 167.9550 → 168.3300 | +0.22% |
| ER | 414.1450 → 410.7450 | -0.82% | 228.0500 → 227.1100 | -0.41% |

A separate frozen v0.9.0 Zenuml public E2E context uses two tag/head AB/BA pairs (four
invocations), with the same SVG identity: 20.2635 → 23.7765 us (+17.34%).
The release error/resource contract differs, and this is uncalibrated same-output context for one
fixture. It establishes neither a theme-only cause nor an all-family gap or calibrated recovery.
The ownership reduction is kept for exact allocation savings; these sub-percent adjacent E2E
observations do not establish a stable latency improvement.

## Verification and next owner

Independent read-only reviews cover actual consumers, correctness and adversarial security/resource
contracts, with no required changes. Existing root tests cover responsive/no-viewBox/fixed sizing,
non-finite geometry, root mutations and family provenance, deferred markers/replacements, exact
byte ceilings, canvas origin/gradient bounds and resource-id collision behavior.

- Focused optimized root/SVG selection: 36 passed.
- Optimized native tests across `merman`, `merman-render`, `merman-core`, `merman-export`,
  `merman-bindings-core` and `merman-theme-contract`: 8,252 passed, 13 skipped.
- Five affected native packages, all features/targets: Clippy passed with warnings denied.
- Private theme acceptance: 34 passed; WASM host: 47 passed.
- Minimal SVG/Zenuml/Treemap/Venn feature check and optimized full WASM build passed.
- Full WASM smoke: 37 diagrams, including deterministic and host-callback text-provider capability.
- `compare-all-svgs`: all 37 families passed structure/parity/parity-root DOM modes, with the
  existing browser-text diagnostic option. This is not browser-pixel equivalence.
- `cargo fmt --all -- --check` passed. No dependency, fixture, feature or public API changes.

A separate consumer review confirms the public facade creates a request-owned Engine and
`Operation::begin` immediately clones it again; the temporary has no later consumer. The pipeline
public E2E row traverses this path, while prepared SVG emission does not. Moving the owned value
into the operation is the next independent candidate, preserving request theme isolation, input
admission, runner/cancellation and parse/semantic error ordering. Freeze explicit/default themes,
site/source overrides, rich/data SVG and failing-input controls before that experiment.

The root style composition temporary is another independent candidate: ordinary root writing
clones style before a short-lived consumer, even without custom properties. Usecase's actual
custom-property composition, empty-style omission, formatting, validation/encoding order and
resource checkpoints must survive any borrowing change. Neither candidate is assigned the full
remaining v0.9.0 gap; this checkpoint does not implement either.

Production commit: `ca17e522871fd6110c794e0e3f5feace1ef52cb7`. Pipeline SHA-256: `8b366526ed5e0490327ad18ec2c590b897529446a4c94d4380fc4bcf4d8aa7d8`.
Preregistration, the initial-baseline correction note, source/artifact digests, sampling, completed
five-repeat allocation records, ordered measurement traces, output/report comparisons, diagnostics and
validation logs are under `target/bench/experiments/root-viewport-state-20261018/`.
Cargo, allocator capture, profiling and timing run serially on Apple M4 Pro, 48 GiB, macOS aarch64,
Rust 1.95.0. Previous XML A/A remains inconclusive and is not retried. No full strict
release/transport-matrix certification is claimed.
