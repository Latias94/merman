# Runtime Hypothesis Closure Receipt

Date: 2026-07-29

Status: U3, U9, U11, and U12 are rejected at discovery. No candidate code remains.

## Scope and identities

- Typed-stage screen: adjacent pre-U5 revision `1d63259af3df9d3292fbfb38760baeaa9b0efb67`.
- Kanban public-path base: accepted U5 revision `19da80418dd0ad47a87ff7f7a99b5d3a7ccf76af`.
- Host: macOS arm64, Apple M4 Pro.
- Existing fixtures and Criterion `parse`, `compatibility_json_parse`, `layout`, `render`, and
  `end_to_end` groups were reused.

No permanent benchmark lane, runner, script, fixture, test function, dependency, or production API
was added. The only production candidate was the temporary U3 Kanban screen described below; it was
removed after measurement.

## Typed parser upper bounds

The screen produced these diagnostic median estimates:

| Family | Typed parse | Compatibility JSON | Layout | SVG emission | End to end |
|---|---:|---:|---:|---:|---:|
| Mindmap | 11.956 us | 136.052 us | 93.510 us | 59.284 us | 155.258 us |
| Requirement | 5.487 us | 108.984 us | 124.906 us | 48.724 us | 181.549 us |
| Kanban | 6.118 us | 111.934 us | 10.455 us | 36.083 us | 53.450 us before U5 |

Compatibility JSON is reported only to keep the lane meaning explicit; it is not the render-only
candidate target. Using the optimistic 10% component of the low-latency gate gives these public-path
lower bounds before any A/A noise multiplier:

| Unit | Public base | Minimum 10% saving | Entire typed parser | Discovery result |
|---|---:|---:|---:|---|
| U11 Mindmap | 155.258 us | 15.526 us | 11.956 us | Impossible even if the entire parser were free. |
| U12 Requirement | 181.549 us | 18.155 us | 5.487 us | Impossible even if the entire parser were free. |
| U3 Kanban after U5 | 40.710 us | 4.071 us | 6.118 us | Required an owner-local upper-bound probe. |

Editor fact and lexeme work is only a subset of each typed parser. U11 and U12 therefore cannot
clear the public low-latency gate on these fixtures, and no family mode or test matrix was built.

## U3 Kanban upper-bound probe

The temporary family-local candidate disabled `KanbanLexemeTrace` storage/finalization and explicit
`EditorSemanticFacts` construction on render and compatibility paths while leaving the combined
editor path enabled. It deliberately removed the dominant explicit bookkeeping before investing in
a complete purpose-typed parser design.

- Base executable SHA-256: `d1902b22e76cff6cc7c50280b34aa1bb51e41f19437ec1e17600cbbc11e4876e`.
- Candidate executable SHA-256: `197ec2dbe3d4680430d16126f52dd62c25d299f1027c44762972cbb4c9226b72`.
- Candidate artifact delta: +112 bytes.
- Existing lane: `end_to_end/kanban_medium`.
- Diagnostic schedule: four alternating BH/HB pairs, 20 samples, one-second warmup, one-second
  measurement, and 10,000 resamples per observation.

| Pair | Order | Base ns | Candidate ns | Delta ns |
|---:|:---:|---:|---:|---:|
| 1 | BH | 39,771.163 | 38,788.768 | -982.395 |
| 2 | HB | 40,211.117 | 39,387.731 | -823.386 |
| 3 | BH | 40,103.107 | 39,172.154 | -930.953 |
| 4 | HB | 40,528.934 | 39,266.081 | -1,262.853 |

The means were 40,153.580 ns and 39,153.683 ns: -999.897 ns and -2.49%. The optimistic minimum
gate was 4,015.358 ns and 10%, so the observed effect reached only about one quarter of either
threshold. Confirmation sampling could not change that effect-size mismatch. The candidate was
removed without adding tests or retaining a parser-purpose abstraction.

## U9 Requirement residual

The retained sampling profile is
`target/bench/u9-requirement/layout.sample.txt`, SHA-256
`ab970f5d5320a6f228c221d9dcdfe3c959edaa269487f398cde73448061df1ec`.
It contains 3,710 samples below `merman_render::family::prepare` for the existing medium fixture.

The top-level offset clusters attribute 3,585 samples (96.6%) to work that U9 explicitly excluded:

- 1,860 samples across the two Requirement box/label construction phases;
- 1,366 samples in the layout invocation, including 1,326 in Dugong;
- 359 samples across the two edge/label text-measurement phases.

The remaining 125 samples are dispersed across configuration lookup, graph assembly, formatting,
and destruction. No named Requirement-owned repeated lookup, allocation, or carried value has an
effect remotely close to the 18.155 us optimistic public threshold. Revisiting text measurement or
replacing Dugong would be a separate semantic/layout program, not a U9 residual fix. U9 therefore
closes without production code or new tests.

## Decision

- U3: rejected; explicit render-only editor bookkeeping saved about 1.0 us, below both low-latency
  thresholds.
- U11 and U12: rejected by strict whole-parser upper bounds.
- U9: rejected because the complete profile names only excluded dominant owners and sub-threshold
  dispersed residuals.

Retest only when the public workload changes materially, a production high-volume contract is
registered, or a profile names a new owner-local term. A smaller absolute threshold must not be
selected after observing these results.
