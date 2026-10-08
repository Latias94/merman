# Theme ownership refactor: confirmation and partial closeout - 2026-10-07

## Decision

Accept the measured operation-ownership and SVG-sealing changes on macOS ARM64
within the verification scope below; full plan completion is not established. The
adjacent clean source baseline is `97ed7fcb9ab97538384ebcd0aa5a808cf01cd258`;
the measured candidate is `ab91b43963088734e163b667a07d0a49e2305ff2`
on `refactor/theme-ownership-performance`. Four public end-to-end workloads
confirm latency non-regression; three also confirm improvement. No pull request
is part of this report. These historical measurements apply only to the stated
revisions, not to subsequent review fixes or the current working tree.

The operation builder now owns appearance selection, theme materialization and
the final effective configuration. Detector checkpoints retain an old value
only when replay needs it. Explicit same-value and nested writes retain their
ownership; initialized values are not misclassified as detector writes. SVG
artifacts have one consuming seal boundary after postprocessing, prepared-text
partitioning and validation. The final native bytes receive one resource
fingerprint. Class node expectations are retained in the completed receipt,
while typed marker expectations are built only when the resolved stroke plan
uses them. The public/native projections, security checks, error order and
ordinary source/style/marker evidence are covered by the named historical tests
below; this does not establish the unrun full verification matrix.

## Implementation scope and review follow-up

- U1 provides test-only counts for COW, appearance, materialization, replay and
  resource fingerprinting. It does not provide copied-byte, prepared-text scan
  or Class expectation allocation counters. No allocation or scan-byte reduction
  is established by this report.
- U3 tracks mutation epochs/path stamps and uses checkpoint snapshots and diffs.
  It does not implement the plan's ordered setter journal. The ownership and
  replay changes should not be described as delivery of that proposed mechanism.
- U5 retains Class expectations in requested receipts and builds typed marker
  expectations when needed. It does not introduce the proposed cross-family
  operation-level evidence-obligations request. Receipt reuse and conditional
  construction are narrower than completion of that design.
- U4's consuming seal, final-byte fingerprint boundary, single validation of
  identical public/native projections, and centralized finalization-report
  ownership are implemented and covered by focused regression tests. The new
  review fix is not included in the historical end-to-end performance results
  below.
- U8 remains partially verified: the full render integration inventory and
  workspace-wide strict checks were not run. The report records measured results
  and known gaps, not blanket completion of the plan's Definition of Done.

These are implementation boundaries, not a mandate to introduce a journal or
cross-family evidence framework solely to match the original plan.

## Confirmation: native end-to-end latency

The checked-in [confirmation summary](evidence/theme_ownership_refactor_2026-10-07.json)
provides the revisions, scope, workload results and archive integrity metadata.
The [lossless raw receipt archive](evidence/theme_ownership_refactor_2026-10-07.json.gz)
contains the frozen executable digests, A/A calibration, ordered samples,
simultaneous confidence bounds and exact output controls. Both executables
passed eight A/A calibration pairs. The confirmation uses eight fresh balanced
AB/BA pairs per fixture (minimum eight, cap 64), the `long` preset, 10,000
bootstrap resamples and simultaneous 95% bounds across four fixtures and two
metrics (Bonferroni). The preregistered joint gate is 10% relative and 50 us
absolute. Inputs, SVG byte counts, element counts and SHA-256 outputs match.

| Public operation | Base (us) | Candidate (us) | Paired delta (us) | Candidate/base | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| `class_medium` | 1324.250 | 1144.312 | -179.938 | -13.59% | confirmed improvement |
| `mindmap_medium` | 383.485 | 378.540 | -4.945 | -1.29% | non-regression, no confirmed improvement |
| `flowchart_medium` | 1666.612 | 1502.675 | -163.938 | -9.84% | confirmed improvement |
| `sequence_medium` | 682.491 | 514.785 | -167.706 | -24.57% | confirmed improvement |

The comparator's mirrored improvement threshold is **base/candidate > 1.10**,
not a 10% reduction calculated in the candidate/base direction. This explains
why the Flowchart row clears its gate. These are native public-operation
results, not claims about browser/WASM, CLI startup, memory, allocations or
export throughput. The one-seal test-only counter proves removal of a second
fingerprint, but timing alone does not prove fewer allocations.

Reproduce with `tools/bench/compare_self.py` and the checked-in
`tools/bench/corpus.json`, using `merman/pipeline`, `--suite standard`,
`--group end_to_end`, the four `*_medium` fixtures above,
`--preset long --evidence-mode confirmation --calibration-pairs 8
--max-pairs 64 --relative-threshold-percent 10 --absolute-threshold-us 50
--bootstrap-resamples 10000`. Both revisions use `--no-default-features` and
`svg,all-diagrams,layout-cytoscape,layout-elk,math` with a shared target that
is frozen into two separate executables before sampling. The local experiment
and readable comparator output remain in
`target/bench/experiments/theme-ownership-20261006/`. The complete raw receipt is
tracked as deterministic gzip (level 9, timestamp zero, no original filename),
reducing 1,099,801 raw bytes to 88,241 bytes. Its uncompressed SHA-256 remains
`e2e708b6a37df34bbbc292d81588d839111067d527f033de294142723435fe88`;
its archive SHA-256 is
`985aadd3313ce4a60b90c629a73e8c1236d55ea5f7c95054f743326de91b0f2d`.
The original JSON path now contains a summary, **not** a `compare-self-v2`
receipt. Decompress the archive before passing historical evidence to any
consumer that expects that receipt schema; no comparator changes are required.
From the repository root, this restores and checks the exact original bytes:

```python
from pathlib import Path
import gzip
import hashlib
import json
import tempfile

summary_path = Path("docs/performance/evidence/theme_ownership_refactor_2026-10-07.json")
summary = json.loads(summary_path.read_text())
metadata = summary["archive"]
archive = (summary_path.parent / metadata["path"]).read_bytes()
assert len(archive) == metadata["bytes"]
assert hashlib.sha256(archive).hexdigest() == metadata["sha256"]
raw = gzip.decompress(archive)
assert len(raw) == metadata["uncompressed_bytes"]
assert hashlib.sha256(raw).hexdigest() == metadata["uncompressed_sha256"]
receipt = json.loads(raw)
assert receipt["schema_version"] == metadata["receipt_schema_version"]
output = Path(tempfile.gettempdir()) / "theme-ownership-confirmation-full.json"
output.write_bytes(raw)
print(output)
```

The tracked receipts replace local absolute worktree prefixes with
`<local-worktrees>`; samples, hashes and measurement settings are unchanged.
Compression preserves the raw receipt byte for byte, including those existing
redactions, rather than rewriting or resampling the historical evidence.

## Artifact size and dependency attribution

The [CLI size receipt](evidence/theme_ownership_cli_size_2026-10-07.json)
compares the same `cli-release` dist-profile recipe, Rust 1.95.0 ARM64 host,
lockfile and capabilities. The source-equivalent CLI control is `5c40e666`:
its difference from `97ed7fcb9` consists only of four performance documents.
Its frozen CLI artifact has SHA-256
`034300f5b652a18f120d0d60a626aaf01d7dc3be36cd11b5202b8c014f552628`.
The candidate CLI artifact SHA-256 is
`c613a36f951cbd641172f1e31f17c081a028e1004b1520de3debda23a07ce8e2`.

| ARM64 dist CLI | Source-equivalent base | Candidate | Delta |
| --- | ---: | ---: | ---: |
| Raw bytes | 53,462,736 | 53,453,600 | -9,136 |
| Raw gzip-9 bytes | 22,054,346 | 22,044,537 | -9,809 |
| `strip -x` bytes | 46,613,400 | 46,613,400 | 0 |
| `strip -x` gzip-9 bytes | 20,151,439 | 20,142,472 | -8,967 |

Version and capabilities outputs match. Base, candidate and stripped candidate
produce byte-identical SVG for Class, Mindmap, Flowchart and Sequence medium
under the same deterministic CLI config. This demonstrates size non-regression
for this exact native CLI recipe, not a measured decrease in stripped size or
an equivalent claim for WASM. All manifests, `Cargo.lock`, build scripts,
Cargo settings and capability descriptors are unchanged in this comparison;
the refactor adds no dependency or feature edge. Test-only counters are not
part of the production binary.

The earlier [theme integration attribution](theme_main_integration_attribution_2026-10-06.md)
compared main `733fd2fa` with pre-refactor `5c40e666` and found substantially
larger theme-integration growth. It attributed that earlier delta mainly to
newly reachable renderer/theme work and generic code generation, not simply
the one new external dependency. Its baseline is **not** the baseline for the
small refactor deltas above. Symbol extent is not a counterfactual measure of
removable binary size.

## Correctness and boundaries

The following results are historical verification of the measured refactor.
They do not include a latency measurement for the focused U4 review fix.

- Core `all-diagrams`: 1,900/1,900 nextest passed.
- Render full-feature unit tests plus Flowchart SVG, root canvas and typed-family
  integrations: 3,093/3,093 passed, two skipped.
- Facade operation-isolation, diagram-theme-layering and render-operation tests:
  51/51 passed. Reused engines retain exact effective config, SVG, native
  resource fingerprint and document identity across Class, Flowchart, Sequence
  and Mindmap with default, source-base, string-null and JSON-null themes.
- `cargo fmt --all -- --check`, `git diff --check`, `xtask verify-generated`,
  production-feature Clippy and targeted Class/Mindmap/Flowchart/Sequence SVG
  DOM parity passed. Browser text-layout residuals remain bounded and accepted.

The reduced `all-diagrams`-only render run passed 4,422 and failed six tests
whose ELK expectations are incompatible with its Dagre fallback; the unchanged
tests and backend paths pass under the complete layout feature set. Three
separate Flowchart/ER/GitGraph facade theme-coverage assertions are historical
baseline debt documented in
[`2026-09-21-embedded-font-retirement.md`](../knowledge/engineering/verification/2026-09-21-embedded-font-retirement.md).
The entire render integration inventory and workspace-wide strict verification
were not run in this disk-constrained lane; the named affected suites, generated
checks and targeted DOM parity above are the verified scope. The non-empty
prepared ledger seal test exercises a synthetic internal projection: it does
not claim asset-free family renders normally produce a non-empty ledger.

U6 strict-validation traversal fusion remains a separate experiment: changing
traversal order needs independent syntax/error/resource evidence. U7 theme
compiler movement or type erasure is deferred because the unchanged dependency
boundary and equal-capability size control do not support a migration or a
size-win claim. Neither experiment is a prerequisite for the core ownership
changes, but their optional status does not close the U1/U3/U5 scope differences
or the U8 verification gaps above. The remaining optimization frontier requires a
separate registered workload, not a speculative family fast path or cache.


## PR review follow-up: explicit ownership lookup

The follow-up to baseline `6ef39febc` removes the full scan of explicit config
paths from each ownership query. The existing ordered set now supplies exact
and ancestor lookups, followed by a bounded descendant-prefix lookup. For `P`
explicit paths, query depth `D`, maximum path length `L`, and `E` Mindmap edges,
the repeated lookup bound changes from `O(E * P * L)` string work to
`O(E * (D + 1) * log(P + 1) * L)`. Each query allocates at most `O(L)` temporary
prefix space; there is no persistent index or dependency change.

The test-only probe counter observes at most 400 set operations for 100 absent
two-segment queries at each of 100, 1,000, and 10,000 unrelated explicit paths.
The old scan fails that gate at the first scale with 10,000 candidate visits.
This is structural complexity evidence, not a latency or allocation benchmark;
the counter does not count the ordered set's internal string comparisons.
Boundary tests preserve empty-path, ancestor/descendant, separator, and Unicode
semantics. Mindmap palette preparation also checks cancellation while traversing
nodes and edges, including the unthemed baseline. These changes are not included
in the historical latency and binary-size measurements above.
