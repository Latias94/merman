# ASCII Contract Hardening Performance Spotcheck

Date: 2026-08-25

## Scope

This spotcheck covers the representative unrestricted ASCII corpus entry used by the existing
Criterion harness and the bounded fallback contract tests. It is a guard against output drift and
late fallback admission regressions; it is not a cross-machine latency claim.

## Command

```text
cargo bench -p merman --no-default-features --features ascii --bench ascii_pipeline -- \
  --exact ascii_end_to_end/flowchart_medium --measurement-time 0.2 --warm-up-time 0.1 --sample-size 10
```

## Receipt

- Benchmark: `ascii_end_to_end/flowchart_medium`
- Output kind: `plain_ascii`
- Output bytes: `12558`
- Output SHA-256: `74e221ad525bdf28398dc086c8de6bba0b8f7fe94076cd70b411656751873a23`
- Measured time on the local host: `[10.589 ms, 10.667 ms, 10.738 ms]`
- Benchmark change versus the prior receipt: `[-3.6368%, -2.7565%, -1.9391%]` (all improved)
- Postflight identity: passed

The fallback allocation path is covered by `output_report` and `operation_cancellation`: a wide
primary is discarded before semantic fallback construction, candidate dimensions are checked in a
detached scope, and one complete candidate admission is committed to the render-wide ledger. The
exact/N-minus-one resource and cancellation assertions are the authoritative fallback guard; no
over-wide or partial candidate is accepted.
