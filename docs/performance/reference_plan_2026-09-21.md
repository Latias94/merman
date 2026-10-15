# SVG reference-plan allocation checkpoint — 2026-09-21

## Decision and scope

Evaluate the SVG reference dependency graph once, then add conservative effect contributions
before propagating source-element occurrences. This removes the second graph evaluation and its
temporary vectors without changing the resource model. It is an allocation reduction, not a
confirmed latency improvement or an asymptotic-complexity change.

The preregistered primary gate requires Class medium allocation count and cumulative bytes to
decrease. All five controls must preserve output identity, avoid increases in allocation count,
cumulative bytes and peak growth, and return to their own pre-operation live-byte count. Timing
is diagnostic: flag a slowdown only when it exceeds both 10% and 50 microseconds.

## Equivalence argument

The XML owner places every real node under the single SVG root. The root cannot be a filter,
mask or clip-path definition. The initial traversal detects cycles and calculates each node's
expanded subtree size and depth. Adding edges from the root to effect definitions therefore
cannot change non-root costs or the existing topological order.

Freeze the original root size as the application multiplier. Add each effect's weighted cost
and depth contribution to the root, seed that effect's occurrence count with the multiplier,
and propagate occurrences once in the original order. This preserves the old extra-root-edge
calculation, including shared descendants, nested effects, virtual duplicate-ID candidate
groups and saturating arithmetic. The generic graph entry point supplies no effects and keeps
its existing behavior. XML validation, CSS-independent conservative charging, limits, error
classification and cooperative cancellation remain enforced.

The change adds no public API, cache, dependency, fixture-specific branch or budget adjustment.
For V dependency nodes and E edges, both graph evaluators remain O(V + E) time and O(V + E)
space; the improvement removes one evaluation's transient storage and traversal.

## Workload and provenance

The candidate is the `final_validation.rs` patch on `7596b61ad`. The frozen baseline executable
comes from clean measurement revision `e4db6ff9c6e9764132116ce10a6d011af1a24107`, whose production
crates, root manifest and lockfile match that baseline HEAD. Its untimed benchmark digest
formatting differs from the working branch; the timed public operation is unchanged.

Both executables use Rust 1.95.0, optimized builds, default features disabled, and
`svg,layout-elk,layout-cytoscape` on Apple M4 Pro / macOS 26.6.2 ARM64. Builds and measurements
run serially. The fixed workload is the public reusable `Renderer`, strict parse, layout and
complete default SVG output for the five checked-in benchmark fixtures below.

The standalone probe reuses `native_memory/allocator.rs`. Each fixture has three warmed
allocation observations, including output destruction; file writes and hashes are outside the
measured interval. This is not RSS, cold startup, installed-package size or native PNG/PDF memory.

## Measurements

All three observations agree within each executable. All five SVGs and full public admission
receipts, plus all ten preset fingerprints, match exactly. Retained growth is zero in every
observation; this does not require identical process-global pre-operation heap totals.

| Fixture | Allocations before → after | Allocated bytes before → after | Peak growth bytes before → after |
| --- | ---: | ---: | ---: |
| class_tiny | 1,209 → 1,198 | 215,444 → 211,205 | 61,415 → 60,608 |
| class_medium | 13,690 → 13,678 | 2,316,024 → 2,294,888 | 298,270 → 293,086 |
| xychart_medium | 2,196 → 2,186 | 238,783 → 234,837 | 106,418 → 106,418 |
| flowchart_medium | 49,305 → 49,293 | 6,304,067 → 6,279,400 | 931,502 → 931,502 |
| sequence_medium | 2,675 → 2,663 | 490,408 → 482,202 | 110,030 → 110,030 |

Class medium removes 12 allocations (0.088%), 21,136 cumulative bytes (0.913%) and 5,184 bytes
of peak growth (1.738%) per complete operation. These results pass the registered memory gates.

The existing pipeline benchmark runs A/B/B/A over all five fixtures, with 30 samples, a two-second
warmup and a three-second measurement target: 20 rows and 600 samples. Each row passes preflight
and postflight output-identity checks. The table averages the two run means for each executable.

| Fixture | Baseline mean µs | Candidate mean µs | Diagnostic delta |
| --- | ---: | ---: | ---: |
| class_tiny | 154.330 | 156.605 | +1.47% |
| class_medium | 1,201.500 | 1,205.450 | +0.33% |
| xychart_medium | 212.570 | 214.550 | +0.93% |
| flowchart_medium | 2,665.000 | 2,683.150 | +0.68% |
| sequence_medium | 387.805 | 390.720 | +0.75% |

No row triggers the joint regression threshold. These two diagnostic pairs neither establish a
speedup nor replace calibrated confirmation. The published-alpha.6 Class regressions in the
[matched renewal](alpha6_current_class_2026-09-21.md) remain open.

## Verification

The release-profile pipeline suite passes 245 tests. Its new differential test compares the
complete plan against the previous two-evaluation algorithm across 30 combinations of effect
placement, duplicate IDs, shared references, marker multiplicity, cycles and integer saturation.
Existing pipeline tests cover malformed XML, namespace handling, native resource admission and
cancellation. A review follow-up exercises cancellation at every observed checkpoint on an SVG
with filters, masks, clipping and shared references, preserving the structured error and stopping
immediately. It does not hard-code the traversal schedule. The public render-operation and
security suites pass another 53 tests. This follow-up changes tests only; the production code
measured by the frozen executables is unchanged.

Formatting, whitespace checks and scoped release Clippy pass. Clippy reports 170 existing
renderer warnings; this is not a warning-free claim. Independent correctness, security and
adversarial reviews report no findings. The independent testing review's cancellation-coverage
gap is addressed by the passing follow-up test. Performance and project-standard checks were
performed by the parent reviewer, not additional independent agents.

The full 35-family SVG DOM run passes with `--check-dom --dom-mode parity-root --dom-decimals 3
--diagnostic-browser-text-layout`. It consumes the existing exact browser-text-layout residual
receipts (96 comparisons across eight families); no comparator or residual policy changed.
The required release-profile full `structure` comparison also passes all 35 families.

The existing native-memory smoke owner and report validator pass: two operation/zero pairs
in four fresh processes cover scales 1x and 100x. The report is `protocol_smoke_pass`, with
`candidate_admission: false`; it neither supplies the full six-scale matrix nor qualifies
large-diagram memory. The registered five-fixture allocation experiment above owns the narrow
allocation claim.

`xtask verify --strict` passes formatting and the workspace all-features check, then fails at
`cargo clippy --workspace --all-targets --all-features -- -D warnings`: 148 existing renderer
warnings become errors. The changed file's diagnostic is the unchanged `ReferenceNode.is_style`
field at line 866. All 148 diagnostic source lines match `7596b61ad`; this is a source-location
comparison, not a second full baseline Clippy run. No diagnostic points to the new graph
calculation or test. Later strict stages did not execute; the separately passing DOM run and
focused tests do not turn this into a complete strict pass. The allocation change is retained
as a locally verified slice with its full structure, resource and output gates passing. The
broader strict-verification gap remains a release-audit item; no warning gate was changed.

## Rejected predecessor

The first candidate reused the baseline plan only when no effect-root edges were needed.
Its 245 pipeline tests and output-identity checks passed, but Class tiny and medium allocations
were unchanged: their default SVGs contain both `drop-shadow` and `drop-shadow-small` filters.
XY Chart and Sequence improved, but changing the primary workload to those controls would not
satisfy the registered objective. The entire candidate and its tests were removed, with no
timing or broader verification performed after the primary failure.

The present candidate retains the same primary workload and gates and handles effect-bearing
graphs. The rejected patch, allocation observations and frozen executable remain under
`target/bench/experiments/reference-plan-reuse-20260921/`.

## Evidence and limits

The active ledger, probe, binaries, input digests, output artifacts, allocation CSVs, test logs
and raw timing rows are under
`target/bench/experiments/reference-plan-single-evaluation-20260921/`.

| Artifact | SHA-256 |
| --- | --- |
| Frozen baseline pipeline | `b49a02b83234e63296c2afa6deef570e720899e0528a5db7c0b8f24ee60fe208` |
| Frozen candidate pipeline | `e5bf72f9088d50d7ffdd9e9ce89738546fc267fc4275d46f4a2068af2aa559e6` |
| Baseline allocation CSV | `22ea0904b277689bfac09c83e944bbed368211b19e79e87a02c8b33857cf82d5` |
| Candidate allocation CSV | `59efe1efc170431384d8490a22fe9875c4226f97b072ae5cf69013bef5843e38` |
| Cargo.lock | `b5ffed4195632a60a41be06544d43103d6c33617dc4fc7f4642d9a3ea2cab8f0` |

Build the measured surface with:

```text
CARGO_BUILD_JOBS=1 cargo bench --locked -p merman --no-default-features --features svg,layout-elk,layout-cytoscape --bench pipeline --no-run --message-format=json-render-diagnostics
```

The local `measure_candidate.py` uses the checked-in Criterion command and receipt parsers,
then runs each frozen executable with an exact `end_to_end/<fixture>` selector. Exact commands
and executable hashes are retained in `diagnostic-timings.json`.

This checkpoint does not qualify additional themes or hosts, refresh installed artifacts,
close C7a/U10, or justify changing the conservative effect multiplier. Further latency work
needs its own registered experiment; final-candidate delivery evidence must use the final source.
