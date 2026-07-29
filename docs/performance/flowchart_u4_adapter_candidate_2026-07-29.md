# Flowchart U4 Adapter Candidate Receipt

Date: 2026-07-29

Status: rejected. The indexed Flowchart adapter candidate did not satisfy the preregistered
latency admission gate and leaves no production residue.

## Compared identities

- Adjacent base commit: `6b5f3e0ef2bc1b3162712b5a2de71fe8f887e213`.
- Adjacent base tree: `7e9f82864725122c37e7b8931c3bdaaf5f790a4f`.
- Candidate commit: `234ad437335db899494612b3b9f0be83fe0af954`.
- Candidate tree: `1f416a8512a783d6aa3c1926ff7845299d5d1fd6`.
- Candidate commits under test: `e4c702adeb0cd7687b86f83f2463f72a1cb6ac9b` and
  `234ad437335db899494612b3b9f0be83fe0af954`.
- Base executable SHA-256:
  `3815342419503cbc5905aba8cce7d3fb3985a8f474a928d2149f3fcd0bb86620`.
- Candidate executable SHA-256:
  `2de13fb9ed535afb3ad70b100d27b414f821f0009c1945e1a04369968a478d42`.

The discovery report was anchored before confirmation with SHA-256
`86294ff35f280b43dbfadeabe1dd6e7bf2cac589a1627f9a3fdf0a597022accf`. Confirmation reused only
the frozen executables admitted by that report. The runner revalidated the source report after
sampling and recorded `verified`; there were no contract errors.

## Confirmation result

The confirmation report was generated at `2026-07-29T06:02:50+08:00` with eight balanced A/A
calibration pairs per executable, up to 32 balanced A/B pairs, a simultaneous 95% Bonferroni
confidence family, and the preregistered `>10%` and `>50 us` improvement thresholds. Repeated
process audits during the accepted run found no additional Cargo, rustc, nextest, Clippy, xtask,
or CLI verification process.

| Public lane | Result | Required pairs | Detail |
|---|---|---:|---|
| `flowchart_large` | Inconclusive calibration | 2,622 | Primary lane exceeded the 32-pair cap on the candidate A/A absolute margin. |
| `flowchart_medium` | Inconclusive calibration | 734 | Candidate A/A failed relative, absolute, order, and pair-cap checks. |
| `flowchart_ports_heavy` | Inconclusive calibration | 8 | Base A/A failed the absolute identity and order margins. |
| `class_medium` | Confirmed non-regression and non-improvement | 32 | Candidate was 1.86% slower; simultaneous bounds were +0.53% to +4.19%, or +4.47 us to +36.51 us. |

The suite outcome was `inconclusive` with exit code 3: zero confirmed improvements, zero confirmed
regressions, three calibration-inconclusive Flowchart lanes, and one confirmed non-improving
control. The confirmation JSON SHA-256 is
`5e80972cbd93435e5e7913405c5e69f42f189df1ca5f6b31d605481c3aa34e76`; its Markdown projection
SHA-256 is `9b163b7bdac73f0293e5f752d6c8b44288c8564e7727985401248f7c18763d42`.

## Decision

Admission rules 1 and 2 failed: both executables did not calibrate stably for the primary lane,
and no `flowchart_large` A/B result could establish the required improvement. Collecting the two
native-memory matrices cannot change that conjunctive decision, so the memory gate was deliberately
short-circuited rather than spending ten additional fresh-process matrices on an already rejected
candidate.

The indexed hierarchy, boundary, incident-edge, and extraction implementations and their private
work probes were removed. `crates/merman-render/src/flowchart/layout.rs` is byte-identical to the
adjacent base for this candidate. The Dugong batch-retirement change from `c3130d4dc` remains only
as a documented asymptotic complexity repair; it carries no ordinary-latency or memory claim.

Verification after removal:

```text
CARGO_BUILD_JOBS=1 cargo +1.95.0 test --locked -p merman-render --lib flowchart
67 passed; 0 failed; 794 filtered out
```

The candidate-bound low/high-cluster memory lanes, contracts, generators, and their dedicated
tests were retired with the rejected implementation. The generic Flowchart public scaling/stress
coverage and `flowchart-end-to-end-memory` lane remain useful for future adapter designs. Any new
production candidate requires a new adjacent base, preregistration, discovery anchor, and
confirmation receipt; this result must not be reinterpreted as evidence that the original adapter
is optimal.
