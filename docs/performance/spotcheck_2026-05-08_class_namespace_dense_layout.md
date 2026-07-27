# Class Namespace Dense Layout Spot-check

This report captures the first local baseline after the class layout namespace cleanup. The change
replaces the per-facade full class scan with a precomputed namespace parent/child lookup and adds a
namespace-heavy class fixture to the pipeline benchmark.

These numbers are local spot-checks, not release benchmark guarantees.

## Parameters

- Date: 2026-05-08
- Command:
  `cargo bench -p merman --features render --bench pipeline -- class_namespace_dense --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`
- Fixture: `class_namespace_dense`
- Fixture source: `fixtures/class/stress_class_dense_namespaces_generics_001.mmd`
- Text measurer: pipeline bench default
- Note: no same-machine pre-change sample was captured. Treat this as the baseline for future class
  namespace layout cleanup.

## Criterion Results

Mid estimates:

| fixture | stage | time |
| --- | --- | ---: |
| `class_namespace_dense` | `parse` | 79.688 us |
| `class_namespace_dense` | `parse_known_type` | 94.308 us |
| `class_namespace_dense` | `parse_typed` | 71.064 us |
| `class_namespace_dense` | `parse_typed_only` | 72.930 us |
| `class_namespace_dense` | `layout` | 1.5362 ms |
| `class_namespace_dense` | `render` | 292.15 us |
| `class_namespace_dense` | `end_to_end` | 1.9541 ms |

## Parity Check

Command:

`cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter stress_class_dense_namespaces_generics_001`

Result: passed.

## Observations

- The pipeline bench now has a class fixture that exercises namespace clusters, generic labels, and
  cross-namespace relations.
- Future class namespace/layout changes should use this fixture before relying on broader
  package-wide benchmark noise.
