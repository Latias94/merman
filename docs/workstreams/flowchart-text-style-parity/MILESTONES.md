# Flowchart Text Style Parity Milestones

## M0 - Baseline

- Existing flowchart SVG/layout tests pass.
- Known text-style gaps are documented.

## M1 - First Core Slice

- Mermaid label-style classification is complete for flowchart rendering.
- Relative `font-size` values affect flowchart label measurement.
- Focused tests pass with `cargo nextest`.

## M2 - Measurement Completeness

- Whole-label `font-style` has fixture-backed measurement behavior.
- Spacing and line-height behavior is either implemented or explicitly rejected with evidence.

## M3 - Cleanup

- Duplicate label-style key lists are consolidated where safe.
- Strict parity gates show no unintended regressions.
