# Prepared rendering allocation diagnostic on Windows — 2026-10-20

## Scope and decision

This checkpoint records matched-output allocation diagnostics for the integrated render-reuse
refactor. Native State/Block admission and encoded geometry allocate fewer total bytes in these
workloads; snapshot redraw avoids the separately measured preparation cost. Small control costs
and higher allocation counts in one geometry case remain visible below. This is not a claim that
every memory metric improves, an isolated unit-level attribution, or formal performance admission.
`candidate_admission` is `false`; no timing samples or latency improvement are claimed.

The product baseline is clean `fe5de3eed7f2af1b734f161507330979634c7282` in
`E:/CodexWorkspaces/merman-memory-base-20261020`. The candidate is the dirty development
checkout in `D:/CodexWorkspaces/merman-render-reuse-20261019`, with the actual measured runtime
closure frozen in `source-freeze.json`. Its branch HEAD is not presented as a candidate commit.
Changes outside that closure, including Web test and Swift wrapper follow-ups, are not measured.

## Frozen recipe

- Windows 11 Pro `10.0.26100`, x86_64 MSVC; Intel Core i7-11700, 8 cores / 16 logical processors.
- Rust `1.95.0` (`59807616e1fa2540724bfbac14d7976d7e4a3860`), LLVM `22.1.2`.
- Native optimized release builds, debug information and incremental compilation disabled.
  Separate base/after targets are under `E:/CodexBuilds/merman-render-reuse-memory-{base,after}`.
- Defaults disabled; identical `all-diagrams,svg,native-fonts` feature selection. All 149 registry
  package/version/source identities and 162 dependency package feature sets agree.
- Deterministic runtime, Dagre, classic look, HTML labels disabled, fixed diagram ID. Native cases
  use the checked-in Cousine-Regular.ttf; font SHA-256 is
  `1da22250675fc4c42fcf3a9736c44bc0570516105331443b663fd5cfbd1412fe`.
- Six scales `1, 2, 4, 10, 32, 100`, five repetitions each, alternating base/after and operation/zero
  order. Every sample is a fresh subprocess. There is no randomized generator; the selected
  deterministic policy seed is `0x6D65726D616E0001`.

The one-off probes and driver live only under the ignored
`target/bench/experiments/render-reuse-20261020/` ledger. They reuse
[`CountingSystemAllocator`](../../crates/merman/benches/native_memory/allocator.rs); no production
manifest, dependency, observer, resource bypass, benchmark protocol or CI lane was added.
An independently generated candidate probe lock initially selected different compatible tool
dependencies. That draft lock is retained; the measured build instead seeds the baseline probe
lock and refreshes only its local-package relation offline. Both measured recipes then pass
`--offline --locked`, with matching resolved versions and features. The original workspace locks
are unchanged. The ignored SHA formatter required a compilation correction before either
measured executable; it runs outside the allocator checkpoint.

## Observation boundaries

`BindingEngine::render_svg` measures native State, native Block and a fixed Info control.
Native font acquisition, source construction, options and engine setup precede the allocator
checkpoint. State and Block scales count styled records, using accepted paint declarations.
The Info source is deliberately fixed at every scale label; it is a constant-cost control.
`BindingEngine::edge_geometry_json` measures the actual encoded geometry consumer, including
its controlled writer. Its Flowchart chain has `scale + 1` nodes and `scale` labeled edges.
Each result stays live through the checkpoint; hashing and JSON validation run afterward.

After-only snapshot samples use `Renderer` with identical source, configuration, fixed SVG ID and
640px layout request. `prepare` measures construction and retention of a usable `PreparedSource`;
its SVG is rendered only after the checkpoint to validate the retained owner. `source` measures
a complete source SVG operation. `redraw` prepares before counting and measures a fresh output
with a fresh control. This excludes a real one-time cost and is explicitly a different lifecycle.
It is not a baseline API backport or evidence of a cached layout.

All 660 process records completed. There are 30 exact output groups: 24 source/options/output
groups across base/after and six snapshot groups shared by preparation evidence, source and
redraw. Geometry also retains schema 1 and the expected occurrence count. Every zero-work sample
reports zero allocations. All five observations in every metric cell are identical; no allocator
overflow/underflow occurred. Frozen source and probe hashes were rechecked after execution.

Values below are operation-minus-matched-zero medians. Allocated bytes are cumulative allocator
requests; peak growth is live allocator bytes above the pre-operation snapshot. Neither is RSS.
Retained growth includes the still-live returned output, and is reported separately.

## Public operation results

Each metric cell is baseline → candidate.

| Operation | Scale | Allocation count | Allocated bytes | Peak growth bytes | Retained growth bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native State SVG | 1 | 11,932 → 10,253 | 3,089,274 → 2,598,049 | 405,738 → 270,896 | 32,088 → 32,088 |
| Native State SVG | 2 | 16,981 → 15,248 | 4,445,989 → 3,947,519 | 413,379 → 280,678 | 32,088 → 32,088 |
| Native State SVG | 4 | 27,150 → 25,309 | 7,226,863 → 6,713,867 | 429,661 → 291,834 | 32,088 → 32,088 |
| Native State SVG | 10 | 57,533 → 55,367 | 15,598,798 → 15,035,996 | 478,851 → 341,661 | 32,088 → 32,088 |
| Native State SVG | 32 | 173,083 → 169,730 | 46,989,043 → 46,269,743 | 686,983 → 688,559 | 63,832 → 63,832 |
| Native State SVG | 100 | 530,072 → 523,048 | 143,776,229 → 142,587,261 | 1,554,947 → 1,556,523 | 127,320 → 127,320 |
| Native Block SVG | 1 | 3,007 → 1,247 | 803,510 → 300,392 | 237,045 → 74,842 | 8,280 → 8,280 |
| Native Block SVG | 2 | 4,028 → 2,214 | 1,070,333 → 562,549 | 240,674 → 76,509 | 16,216 → 16,216 |
| Native Block SVG | 4 | 6,068 → 4,146 | 1,581,024 → 1,063,892 | 247,932 → 78,119 | 16,216 → 16,216 |
| Native Block SVG | 10 | 12,177 → 9,927 | 3,137,081 → 2,588,401 | 271,530 → 82,071 | 32,088 → 32,088 |
| Native Block SVG | 32 | 35,262 → 31,822 | 9,593,013 → 8,936,589 | 353,976 → 121,519 | 63,832 → 63,832 |
| Native Block SVG | 100 | 106,601 → 99,485 | 29,349,187 → 28,346,331 | 615,884 → 265,759 | 127,320 → 127,320 |
| Encoded edge geometry | 1 | 3,143 → 2,796 | 643,899 → 493,187 | 260,766 → 206,056 | 856 → 856 |
| Encoded edge geometry | 2 | 4,052 → 3,708 | 723,101 → 572,169 | 263,734 → 225,782 | 856 → 856 |
| Encoded edge geometry | 4 | 5,139 → 4,799 | 855,897 → 702,666 | 283,624 → 266,158 | 1,368 → 1,368 |
| Encoded edge geometry | 10 | 8,196 → 7,877 | 1,321,044 → 1,163,164 | 366,928 → 368,504 | 4,440 → 4,440 |
| Encoded edge geometry | 32 | 19,022 → 18,784 | 3,210,577 → 2,914,877 | 839,220 → 840,796 | 8,536 → 8,536 |
| Encoded edge geometry | 100 | 51,796 → 51,825 | 8,051,683 → 7,059,449 | 1,817,470 → 1,819,046 | 33,112 → 33,112 |
| Fixed Info control | all six | 176 → 178 | 25,846 → 26,010 | 10,766 → 12,342 | 4,312 → 4,312 |

All native rows reduce allocation count and allocated bytes. Block peak growth also decreases.
State at scales 32/100 and geometry at scales 10/32/100 instead add 1,576 peak-live bytes.
Geometry at scale 100 adds 29 allocation requests while removing 992,234 cumulative allocated
bytes. The Info control adds two allocations / 164 cumulative bytes and 1,576 peak-live bytes.
Returned-output retained growth is unchanged in every base/after row. These costs prevent a
blanket memory non-regression claim. The integrated A/B does not identify one unit as their sole
cause; further owner-level attribution would need a separate frozen experiment.

## Snapshot preparation and redraw

Triple cells are prepare / source SVG / prepared redraw. Preparation retained bytes belong to the
snapshot; source/redraw retained bytes belong to their SVG outputs. Those lifetimes overlap in
an application, so they must not be added or compared as interchangeable result owners.

| Scale | Allocation count | Allocated bytes | Peak growth bytes | Preparation retained bytes | SVG retained bytes (source = redraw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 1,766 / 2,812 / 1,044 | 330,163 / 638,383 / 235,620 | 182,372 / 271,628 / 99,918 | 171,710 | 64,199 |
| 2 | 1,830 / 3,727 / 1,895 | 351,289 / 717,915 / 294,026 | 191,661 / 274,681 / 102,096 | 172,585 | 64,199 |
| 4 | 1,958 / 4,824 / 2,864 | 370,597 / 851,031 / 407,834 | 198,217 / 294,307 / 119,972 | 174,335 | 64,199 |
| 10 | 2,327 / 7,914 / 5,585 | 508,661 / 1,312,966 / 731,705 | 258,381 / 370,859 / 191,461 | 179,398 | 64,199 |
| 32 | 3,648 / 18,869 / 15,219 | 856,521 / 3,225,825 / 2,296,704 | 388,689 / 847,771 / 648,755 | 199,016 | 127,687 |
| 100 | 7,677 / 52,048 / 44,369 | 2,053,197 / 8,151,159 / 6,025,362 | 901,856 / 1,840,301 / 1,578,548 | 261,753 | 508,615 |

At scale 100, preparation retains 261,753 allocator bytes. Subsequent redraw requests allocate
6,025,362 cumulative bytes, compared with 8,151,159 for the complete source operation, while
returning identical SVG. This demonstrates the registered parse-once allocation benefit within
this workload. It does not establish a latency benefit, amortization threshold, concurrency cost
or portable memory bound. The preparation checkpoint includes first-use parser state, not only
canonical record storage.

## Work removal and correctness

Source-backed ownership explains the intended removal: native admission now calls typed
[`native_typography`](../../crates/merman-render/src/family/native_typography.rs) owners;
[`FamilyRenderArtifact::edge_geometry_json`](../../crates/merman-render/src/family.rs) uses shared
family geometry without SVG document emission; and
[`PreparedSource`](../../crates/merman/src/render/prepared.rs) shares immutable parse facts while
outputs receive independent operation state. The full-operation measurements above include all
integrated changes and are not private-stage counters or proof of a new whole-render complexity class.

This experiment checks exact successful public outputs. Error/resource/cancellation and unusual
route behavior remain covered by the implementation owners, including
[`native_font_context`](../../crates/merman/tests/native_font_context.rs),
[`edge_geometry_output`](../../crates/merman/tests/edge_geometry_output.rs),
[`edge_geometry_completeness`](../../crates/merman-render/tests/edge_geometry_completeness.rs),
[`prepared_source`](../../crates/merman/tests/prepared_source.rs) and
[`prepared_redraw_contract`](../../crates/merman/tests/prepared_redraw_contract.rs). Their test
runs and the final SVG/browser gates are separate integration evidence, not inferred from this
allocator probe. No reference baseline, normalizer, budget or production output was relaxed.

## Reproduction and evidence

From the frozen source closure, build the two ignored probe manifests serially using distinct
`CARGO_TARGET_DIR` values. Preserve their locks and use `cargo build --release --offline --locked
--manifest-path <probe>/Cargo.toml --bins`. Run the frozen driver with the three executable paths:

```text
python target/bench/experiments/render-reuse-20261020/driver.py --base-exe <base>/release/compare.exe --after-exe <after>/release/compare.exe --snapshot-exe <after>/release/snapshot.exe
```

The driver refuses to overwrite `raw.jsonl`; retain a previous run before deliberately starting
another. This is a local diagnostic recipe, not an added supported benchmark API. Raw records,
summary, complete dependency metadata, CPU/OS facts, file hashes, initial rejected lock resolution
and executable identities remain in the ignored ledger. Build and run logs are
`D:/CodexBuilds/u9-memory-{base-build,after-build,after-matched-build,run}.log`.

| Evidence file | SHA-256 |
| --- | --- |
| `source-freeze.json` | `2f628a8d2c5084b53e1564e585491046dfb2289c856f6588080ff2778ecca29a` |
| `raw.jsonl` | `bd78c34982d8d53f81a13f8de2392f6a4a9efa99c24aab532cfa3af27f878863` |
| `summary.json` | `e56e757f1badd2ad94237dcfef2d00431a501ccf6e6b6f022271100f0666f1fe` |
| `executables.json` | `b04e87847a718adc2890961a24a43abbeb08da066cc8723133e5b6817334856d` |

| Executable | SHA-256 |
| --- | --- |
| base | `03548813ad0cd1f2702fff7e915fe9a174dd0bf45df4514d730fe50b6fa2fccf` |
| after | `768e4cc50338ec58487f7a65577d9f8e4becf763421627f782246e04bcf6d2e9` |
| snapshot | `400bb05157c6680ebcc1ea6b0f0e7f968530dab22ed7fdf7988613388706d9f1` |

See the [current consumer migration](../migrations/render-reuse-host-integration.md) for the
public ownership and geometry resource changes. Dated historical reports retain their own
revisions and do not inherit these measurements.
