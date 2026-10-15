# SVG reference attribute-value experiment — 2026-09-20 UTC

## Decision

Reject the isolated latency candidate. Class medium's complete public SVG operation changes
from 1,184.95 to 1,175.05 microseconds in diagnostic screening: 9.90 microseconds, or 0.84%.
This is below both preregistered requirements, greater than 10% and greater than 50 microseconds.
No candidate production code is integrated. The alpha.6-to-current Class regression remains open.

The experiment date is UTC; the host was already on September 21 in Asia/Shanghai.

## Hypothesis and scope

Complete XML validation normalizes every attribute value before the resource collector reads the
same immutable SVG. The candidate keeps that first validation, namespace handling and cancellation
checkpoints, but omits the collector's repeated normalization for attributes outside its positive
read set: `id`, `href`, `marker-start`, `marker-mid`, `marker-end`, `d` and `points`.
The native safety validator is unchanged. No resource limit or output contract changes.

An untimed inventory of the frozen Class medium SVG finds 1,465 attributes, of which 156 belong
to that read set. Their decoded values account for 24,364 of 44,235 bytes. This inventory describes
the sample; it is neither an execution counter nor a latency estimate.

## Provenance and diagnostic method

The isolated checkout starts at `554a46cec9ba61b038b8d3d5534c9d09f0d6acce`. Rust sources,
`Cargo.toml` and `Cargo.lock` are unchanged from the accepted `b707f6e5b` graph-reuse repair.
The frozen baseline executable is that repair's accepted candidate, not its older baseline.
Both use Rust 1.95.0, macOS 26.6.2 / Apple M4 Pro ARM64, default features disabled and
`svg,layout-elk,layout-cytoscape`. Cargo and all measurements run serially with one build job.

- Baseline executable SHA-256: `e5bf72f9088d50d7ffdd9e9ce89738546fc267fc4275d46f4a2068af2aa559e6`.
- Candidate executable SHA-256: `9b97585dd616ea3929e1f45af7fb259653b9c419261ee248dd99273551d8b5b9`.
- Ledger, candidate patch, input/output hashes, logs and driver:
  `target/bench/experiments/reference-attribute-values-20260920/`.

The existing pipeline benchmark measures complete source-to-SVG rendering through the reusable
public renderer. Five fixtures run in A/B/B/A order, with 30 samples, two seconds of warmup and
three seconds of measurement per row: 20 rows from 600 requested samples. Each row passes both
preflight and postflight output identity checks. The table averages two run means per executable.

| Fixture | Baseline µs | Candidate µs | Delta µs | Delta |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 1,184.950 | 1,175.050 | -9.900 | -0.84% |
| class_tiny | 153.075 | 151.340 | -1.735 | -1.13% |
| xychart_medium | 212.295 | 206.570 | -5.725 | -2.70% |
| flowchart_medium | 2,636.950 | 2,603.300 | -33.650 | -1.28% |
| sequence_medium | 385.020 | 374.260 | -10.760 | -2.79% |

All 20 console records are retained. The existing diagnostic command uses `--discard-baseline`,
so individual raw Criterion samples are not archived. These two pairs do not establish a
confirmed speedup or non-regression. No A/A calibration or decision-grade confirmation follows
because the primary screening result is too small. The admission lane remains latency; incidental
allocation observations do not replace the failed latency gate.

## Correctness and limits

All 245 existing SVG pipeline tests pass, including XML, namespace, reference expansion and
cancellation cases. An independent untimed public-renderer probe also preserves all five SVGs,
complete admission receipts and all ten preset fingerprints byte-for-byte against the frozen
accepted artifacts. Whitespace checking passes.

The initial test command passed the umbrella `svg` feature to `merman-render`; Cargo rejected it
before compilation or tests. The corrected command uses that crate's own layout features. Both
logs are retained. This is not a test failure concealed by changing code or expectations.

The candidate is rejected before full parity, strict verification and new candidate-specific
adversarial coverage. Existing strict Clippy failures and the broader C7a/U10 work remain open.
The isolated patch is preserved for diagnosis and does not change installed artifact provenance.
Further latency work needs a distinct measured hypothesis about XML/reference finalization;
repeating this attribute-read-set experiment does not resolve the observed regression.
