# SVG XML/reference single-pass candidate — 2026-09-20 UTC

## Decision

Admit the candidate implementation. It collects bounded SVG reference facts during the existing
validated XML event and attribute traversal, reusing the already normalized attribute values. On
the registered macOS ARM64 lane, Class medium complete SVG latency improves from 1,253.963 to
1,116.975 microseconds: 136.988 microseconds, or 10.925%. The simultaneous 95% bounds clear
both the 10% relative and 50 microsecond absolute improvement thresholds. Sequence medium also
clears both thresholds. Class tiny, XY Chart medium and Flowchart medium confirm non-regression.

The candidate preserves exact SVG output identities, public receipts and preset fingerprints. It
is integrated as `bdb209e11` (`perf(svg): collect reference facts during XML validation`). This
is a confirmed adjacent improvement; it does not by itself recover the published alpha.6 latency
regression or close C7a/U10.

## Scope and implementation

The measured base is clean source `f3a783c2944a77bc3b561b29f64b41af450ebe8b`; the candidate is
clean commit `0d43621e2b76b02eabf581eceb99b047069a4f68`, later integrated without source changes as
`bdb209e11`. Both use Rust 1.95.0, default features disabled and
`svg,layout-elk,layout-cytoscape` on macOS 26.6.2 / Apple M4 Pro ARM64.

The validator retains QName and namespace resolution, duplicate expanded-name checks, normalized
XML validation, malformed-document errors, raw/final resource checks and immediate cancellation.
The ordinary resource path observes validated per-element facts only after the raw element-budget
callback. Reference expansion planning still waits until the complete XML document passes. The
native safety validator remains a separate policy layer. No public API, resource limit, capability,
dependency or output contract changes.

The earlier event-only fusion is rejected at -6.234% for Class medium because it retained a second
attribute traversal. Its artifacts remain under
`target/bench/experiments/xml-reference-single-traversal-20260920/`.

## Confirmation recipe

The owner froze the shared target directory and rebuilt both sides serially with `CARGO_BUILD_JOBS=1`.
The long recipe used 30 Criterion samples, two seconds of warmup and three seconds of measurement.
Each executable received eight balanced A/A calibration pairs and eight fresh balanced AB/BA pairs.
The confirmation used 10,000 deterministic paired bootstrap resamples, Bonferroni simultaneous
95% coverage across ten relative/absolute components, and the registered 10% / 50 microsecond
joint gate. There were no contract failures, output mismatches or post-sampling identity changes.

The first owner invocation used an uncommitted dirty candidate and exited before sampling because
confirmation requires clean checkouts. That failure is retained in
`target/bench/experiments/xml-reference-single-pass-20260920/dirty-confirmation.json`; it is not
performance evidence and did not relax the harness.

| Fixture | Base µs | Candidate µs | Delta µs | Relative change | Improvement decision |
| --- | ---: | ---: | ---: | ---: | --- |
| class_medium | 1,253.963 | 1,116.975 | -136.988 | -10.925% | Confirmed |
| class_tiny | 163.125 | 140.805 | -22.320 | -13.682% | Inconclusive improvement; confirmed non-regression |
| xychart_medium | 222.884 | 193.816 | -29.068 | -13.042% | Inconclusive improvement; confirmed non-regression |
| flowchart_medium | 2,770.488 | 2,614.500 | -155.988 | -5.635% | Inconclusive improvement; confirmed non-regression |
| sequence_medium | 407.284 | 346.043 | -61.241 | -15.036% | Confirmed |

Class medium's simultaneous improvement bounds are 10.616%–11.285% and 132.999–141.538
microseconds. Sequence medium's bounds are 14.921%–15.164% and 60.685–61.800 microseconds.
The other three rows' absolute changes remain below 50 microseconds or their relative bounds do
not clear 10%, so they are not improvement claims.

## Correctness and verification

The differential oracle compares the original two-pass implementation with the fused path across
22 valid and malformed SVG fixtures and seven positive element limits, matching complete Results
and raw/final budget observations across 154 cases. Fixtures cover nested and branching references,
entities, namespace aliases, markers, effects, cycles, duplicate IDs, foreign HTML/CSS and malformed
XML. The original collector is test-only.

The release-profile SVG pipeline suite passes 247 tests, with 2,409 filtered tests. This includes
exhaustive observed-checkpoint cancellation for branching references, filters/masks/clips and
internal and external marker URLs. `cargo fmt --all -- --check` passes. The full 35-family SVG DOM
comparison with parity-root normalization and the existing browser text-layout residual policy
passes; no comparator or resource budget was changed.

Seven Codex review lenses cover correctness, project standards, testing, maintainability, security,
performance and adversarial behavior. They report no primary findings after the marker cancellation
coverage follow-up. Four initial service-routing failures were retried successfully with session
model reviewers; no external coding tool was used. Review artifacts are under
`/tmp/compound-engineering-501/ce-code-review/xml-reference-czyp1lhv/`.

Workspace strict Clippy remains an existing limitation: the scoped `merman-render` `-D warnings`
run reports 170 existing renderer errors, including the unchanged `ReferenceNode.is_style` warning
in this file; the all-targets/all-features lane has the same pre-existing warning gate. This
candidate does not alter that gate. Broader installed artifacts and host-specific evidence remain bound to their
recorded revisions and require renewal before being treated as final-candidate evidence.

## Evidence and limits

The complete machine-readable confirmation, raw pair schedule, frozen executable identities,
output hashes and ledger are under
`target/bench/experiments/xml-reference-single-pass-20260920/clean-confirmation.json` and the
same directory's companion files. The current-source native memory owner lane also passes from clean candidate `0d43621e2`:
30 fresh-process operation/zero pairs cover scales 1, 2, 4, 10, 32 and 100 with five repeats
and 10,000 bootstrap resamples. At 100x it records 1,655,556 allocations, 348,533,002 allocated
bytes and 85,121,537 peak-growth bytes; the allocation, byte and peak-growth slopes are 1.264,
1.399 and 1.467 against the owner cap of 2.0. The owner contract is infrastructure smoke with
`candidate_admission: false`, so these values are bounded current-source evidence rather than a
memory optimization or release gate.

Allocation probes show a small reduction on the measured
fixtures, but allocation is not the admission metric for this experiment. The accepted latency
improvement is adjacent-source evidence; the published alpha.6-to-current regression, cross-host
performance, cold start, first render, native export, package size and large-memory admission
remain open U10 items. No budget was relaxed, and no tag, push or publication was performed.
