# SVG attribute validation checkpoint — 2026-09-20

## Decision and causal boundary

Reuse completed XML attribute-name validation in the private native compatibility and ordinary
resource-reference passes. Both callers first validate the exact same immutable SVG string in
full. Their second passes now disable only quick-xml's repeated lexical duplicate-name checks;
attribute parsing, value normalization, namespace resolution, target policy, resource accounting
and cancellation remain in place. No public trusted-string API, cache or dependency was added.

For an element with A attributes, this removes the second pass's O(A) duplicate-name storage.
Pinned quick-xml 0.41.0 scans prior names for the first 32 attributes, then uses a hash prefilter
with linear-scan fallback on hash hits. The initial hypothesis described the comparisons too
broadly as O(A²); source review corrected that description before admission. Larger tags normally
have linear hash work in total name bytes, with collision fallback retaining a quadratic worst
case in attribute count for bounded name lengths. The first XML pass still performs those checks,
including namespace-expanded name uniqueness. The complete operation's asymptotic complexity is
unchanged. This is a measured allocation reduction, not a claim of improved worst-case complexity
or confirmed render latency.

The preregistered primary gate requires Class medium allocation count and cumulative bytes to
decrease, with no increase in allocation count, cumulative bytes or peak growth in any of five
public-renderer controls. Output bytes and retained heap must remain unchanged. Diagnostic
timing flags a slowdown only when it exceeds both 10% and 50 µs; it cannot establish a speedup.

## Workload and provenance

The baseline checkout is clean source `4e03b93e7`. Its production source, root manifest, lockfile,
pipeline benchmark and profiling example are identical to pre-change HEAD `983ef011e`; subsequent
changes only added other benchmarks and documentation. The candidate contains the single
`final_validation.rs` change on that HEAD. This is a production-equivalent baseline, not a claim
that both executables came from adjacent clean commits.

Both use Rust 1.95.0, optimized builds, defaults disabled and features
`svg,layout-elk,layout-cytoscape` on Apple M4 Pro / macOS 26.6.2 ARM64. Cargo and measurements run
serially. Inputs are the checked-in Class tiny/medium, XY Chart medium, Flowchart medium and
Sequence medium benchmark fixtures. The reused public `Renderer` performs strict parsing,
layout and complete default SVG rendering with fixed diagram IDs and no explicit theme override.

An ignored standalone probe reuses the existing `native_memory/allocator.rs` counting allocator.
It warms rendering first, then counts allocation through output destruction for three observations
per fixture. File writing and hashing occur outside the measured operation. This is a controlled
warm-process allocation observation, not process RSS, cold start, native export or a size receipt.

## Measurements

All three observations per fixture have identical count, cumulative bytes and peak growth within
each executable. All five candidate SVG files match baseline bytes exactly. After output
destruction, each observation returns to its own pre-operation live-byte count.

| Fixture | Allocations before → after | Allocated bytes before → after | Peak growth bytes, unchanged |
| --- | ---: | ---: | ---: |
| class_tiny | 1,307 → 1,209 | 224,148 → 215,444 | 61,415 |
| class_medium | 14,182 → 13,690 | 2,357,112 → 2,316,024 | 298,270 |
| xychart_medium | 2,326 → 2,196 | 249,791 → 238,783 | 106,418 |
| flowchart_medium | 49,923 → 49,305 | 6,364,739 → 6,304,067 | 931,502 |
| sequence_medium | 2,888 → 2,675 | 515,560 → 490,408 | 110,030 |

Class medium removes 492 allocations (3.47%) and 41,088 allocated bytes (1.74%) per complete
operation. Peak growth does not decrease. This passes the allocation gate without implying an
equivalent reduction in peak memory, RSS or total execution time.

The existing pipeline benchmark also ran five fixtures in A/B/B/A order, with 2-second warmup,
3-second measurement target and 30 samples per row: 20 rows and 600 samples. The first A run was
collected before the rejected search experiment and reused; both executables and output identities
remained fixed. The table averages each executable's two run means. No control exceeds the joint
10% / 50 µs slowdown threshold.

| Fixture | Baseline mean µs | Candidate mean µs | Diagnostic delta |
| --- | ---: | ---: | ---: |
| class_tiny | 195.815 | 190.875 | −2.52% |
| class_medium | 1,397.812 | 1,365.338 | −2.32% |
| xychart_medium | 250.274 | 243.059 | −2.88% |
| flowchart_medium | 2,870.897 | 2,836.702 | −1.19% |
| sequence_medium | 482.032 | 467.926 | −2.93% |

These are diagnostic observations only. Two pairs, separated from the first baseline by
exploration and builds, do not replace balanced A/A calibration and at least eight confirmation
pairs. No statistically confirmed latency improvement is claimed.

## Correctness

The release-profile SVG pipeline suite passes 243/243 tests, including cancellation, resource
limits, malformed XML, namespace and native compatibility contracts. The new regression test
checks ordinary duplicate attributes, aliases with the same expanded name, duplicate namespace
bindings, nested elements and HTML descendants. Both second-pass entry points return exactly the
first XML validator's error. Equal local names in distinct namespaces remain accepted.

All 20 benchmark rows retain matching preflight output hashes, lengths and SVG element counts
within each fixture, and complete postflight identity checks. This is byte-identical cross-family
output evidence for these fixtures; it does not expand the repository's family qualification
matrix or claim a full Mermaid DOM-suite replay.

`cargo fmt --all -- --check`, `git diff --check` and scoped release Clippy pass. Clippy reports
170 existing renderer warnings; this checkpoint does not claim a warning-free crate.

## Rejected hypothesis

An earlier candidate replaced single-byte KMP searches with direct scanning while preserving
checkpoint order. Its 244 pipeline tests passed, but all 15 allocation observations exactly
matched baseline count, cumulative bytes and peak growth. That failed the registered allocation
objective, so the patch was removed and further timing was cancelled. The result is consistent
with compiler elimination of the presumed prefix allocation; no assembly-level claim is made.
Do not revive that optimization solely because the generic source contains a `Vec`.

### Follow-up attribution at `2db16c6b7`

A second follow-up candidate attempted to remove each token-bearing tag's temporary String by
copying retained spans directly into the public SVG buffer. It was rejected before candidate
measurement because it cannot meet the preregistered Class-medium allocation objective:

- `BuiltinFamilyArtifact::prepared_text_label_ledger` has explicit Flowchart, Swimlane, State and
  Sequence owners; Class takes the empty-ledger branch.
- With an empty ledger and no reserved prepared-text spelling, `strip_prepared_text_label_ids`
  returns after the bounded-checkpoint substring search. It does not enter tag stripping.
- The fixed Class input/output has no reserved spelling. A profile entry for
  `partition_prepared_text_label_ids` therefore does not imply that per-tag copying occurs.
- Ordinary standalone finalization computes one resource fingerprint over the native projection
  and font context. No repeated fingerprint calculation was found on that Class path.

The attempted new test incorrectly assumed a default system-font session had a prepared-text
layout; it failed during setup with `None`. The stopped suite reported 175 passing tests, one
failing new test and 69 unexecuted tests. No successful candidate correctness or performance claim
is made. Source review had already disproved applicability, so the workload and test setup were
not changed to rescue the candidate. The entire owned patch was removed after checking the saved
original against HEAD. No production change from this experiment remains.

The rejected patch, test output, five baseline-only diagnostic rows and follow-up CPU sample are
retained in `target/bench/experiments/prepared-text-direct-copy-20260920/`. The frozen pipeline
executable is the source-equivalent `2db16c6b7` build already bound by the attribute-validation
ledger. Sampling attaches during the Class-medium benchmark loop for 10 seconds at 1 ms; this
profile's wall-clock results are explicitly excluded from latency evidence. Any future per-tag
copy optimization needs an independently registered workload that actually owns prepared text;
it must not be presented as recovery of the default Class regression.

## Evidence and remaining work

The ignored directories `target/bench/experiments/svg-byte-search-20260920/` and
`target/bench/experiments/svg-attribute-recheck-20260920/` retain preregistration, source and binary
digests, allocation CSVs, SVG outputs, test logs and raw Criterion files. The rejected patch and
its frozen executable remain in the first directory. The second directory freezes the accepted
candidate executable separately from Cargo's mutable build output.

Build the existing timing surface with:

```text
CARGO_BUILD_JOBS=1 cargo bench --locked -p merman --no-default-features --features svg,layout-elk,layout-cytoscape --bench pipeline --no-run
```

For each frozen executable and fixture, set a separate `CRITERION_HOME` and run
`--bench --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 --exact end_to_end/<fixture>`.
The standalone allocation probe source, allocator digest, linked rlib and executable digests are
bound by the two experiment ledgers. The final `summary.json` SHA-256 is
`4379cc8c59769ee199df415467d7fdfccc36e4a0a6e7398e20231e8856829136`.

The refreshed Class medium CPU sample still locates work in standalone SVG finalization,
resource validation and resource fingerprinting; prepared-text partitioning also remains visible.
Inclusive stack samples do not establish removable cost. Further work needs its own causal
hypothesis and measured gate. The confirmed alpha.6-to-alpha.7 Class latency regressions and the
broader U10 performance/footprint gate remain open; no budget, output policy or capability changed.
