# Performance Plan

This is the single current performance backlog for Merman. It records what to optimize and why.
Use [RUNBOOK.md](RUNBOOK.md) for the execution loop and [BENCHMARKING.md](BENCHMARKING.md) for
measurement semantics and tool details. Dated reports are evidence for a particular revision and
host; they are not rolling sources of truth.

## Current evidence

The release-range baseline and latest committed cross-runner checkpoint were measured on
2026-07-27:

- [Alpha.3 to Alpha.4 Refactoring Report](../release/ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md)
  compares `v0.8.0-alpha.3` with `d2698d0a3`.
- [Three-runner checkpoint](renderer_comparison_2026-07-27.md) compares that historical candidate
  with Mermaid.js 11.16.0 and `mermaid-rs-renderer` at `7ff1196`.
- [Post-optimization mmdr checkpoint](renderer_comparison_2026-07-27_901afd393_vs_mmdr.md)
  compares Merman `901afd393` with the same mmdr revision using the long preset and
  byte-identical input gating.

| Comparison lane | Shared rows | Median ratio | Geometric mean | Faster / slower |
| --- | ---: | ---: | ---: | ---: |
| `d2698d0a3` / alpha.3, complete SVG product | 34 | 1.10x | 1.09x | 13 / 21 |
| `d2698d0a3` / alpha.3, minimal same-capability SVG | 32 | 1.12x | 1.07x | 10 / 22 |
| `901afd393` / `mermaid-rs-renderer`, identical inputs | 30 | 0.663x | 0.293x | 18 / 12 |
| `d2698d0a3` native / warm Mermaid.js browser | 34 | 0.0237x | not reported | 34 / 0 |

The cross-runner rows are not quality-adjusted rankings. Merman and `mermaid-rs-renderer` have
different Mermaid coverage and output goals, and native Rust versus warm Chromium is not a
browser-WASM comparison. The mmdr lane excludes Treemap and XYChart because their same-named
fixtures differ; `flowchart_large` and Info have no shared measurement.

## Triage policy

An ordinary regression enters the active queue when both conditions hold:

- the same-host A/B regression exceeds 10%; and
- the absolute median increase exceeds 50 microseconds.

A workload can bypass that threshold when it crosses an interactive frame budget, materially
changes throughput or memory use, or affects a documented high-volume integration. A large ratio
on a two-microsecond operation is evidence to retain, not automatically a priority.

Before implementation:

1. Match capabilities between base and target revisions.
2. Attribute parse, layout, render, and end-to-end stages.
3. Repeat at least three same-host base/head runs for a decision-grade result, alternating which
   checkout runs first.
4. Record model size, SVG bytes/elements, and the relevant semantic, DOM, or raster parity result.
5. Profile only after the slow stage is known.

## Priorities

| Priority | Fixture | Current latency | Current / alpha.3 | Current / mmdr | User impact |
| --- | --- | ---: | ---: | ---: | --- |
| P0 | `requirement_medium` | 274 us | Not rerun at `901afd393`; historical candidate was 2.12-2.25x | 3.85x (+203 us) | Parse is faster than mmdr; the measured gaps are layout (2.83x) and SVG emission (10.14x). |
| P1 | `mindmap_medium` | 162 us | No confirmed current regression; the ordinary-label fix reduced its pre-fix latency by 75% | 2.16x (+87 us) | The layout ratio compares real COSE-Bilkent with mmdr's radial fallback, so only same-algorithm work is actionable. |
| P1 | `flowchart_large` | 23.67 ms | about 1.00x at the historical checkpoint | unavailable | This is not a refactor regression, but it is the only standard fixture beyond a 16.7 ms frame. |
| P1 | `class_medium` | 874 us | Historical candidate was 1.10-1.15x (+89-126 us) | 0.37x | A common family worth rechecking against alpha.3; Merman remains about 2.7x faster than mmdr. |

Do not prioritize the current Info, Packet, Radar, or Sankey ratios without new absolute-cost
evidence. Their measured alpha.3 increases are about 2-5 microseconds. Architecture,
`flowchart_small`, Gantt, and `flowchart_ports_heavy` are within the current noise band.

Kanban is no longer a cross-runner P0: it now measures 56.50 us versus mmdr's 28.43 us, a 28.07 us
absolute gap below the active threshold. Its alpha.3 lane must be rerun before declaring the
historical release regression closed. XYChart is not a valid cross-runner priority until both
runners use the same fixture.

## Completed work

### Mindmap ordinary-label rendering

Commit `b2129d9ec` resolved the confirmed ordinary-label regression without a family-local
classifier. Against pre-fix revision `71cb231c8`, three same-host runs measured:

| Metric | Pre-fix median | Fixed median | Change |
| --- | ---: | ---: | ---: |
| Mindmap end-to-end | 654,210 ns | 161,330 ns | -75.34% |
| Mindmap layout | 94,693 ns | 89,608 ns | -5.37% |
| Mindmap SVG | 542,590 ns | 62,383 ns | -88.50% |
| Kanban control | 63,881 ns | 56,272 ns | -7,609 ns |

The control delta remained below the registered 10,000 ns noise threshold. The retained
implementation defines an exact syntax-free ASCII projection in the Markdown interpreter and lets
the sanitizer skip DOM work only when its complete effective tag policy preserves the generated
paragraph. Unicode, entities, Markdown, HTML, icons, math, unknown security levels, and customized
sanitizer policies retain the full path. The faster punctuation-blacklist experiment was rejected
after `#quot;` exposed Mermaid placeholder leakage.

## Work queue

### P0.1: Profile Requirement layout and SVG emission

The long stage run measured parse at 0.71x mmdr, layout at 2.83x, and SVG emission at 10.14x.
Profile the render stage first, then layout, using the exact `requirement_medium` benchmark.
Separate the cost of label preparation, text measurement, sanitization, DOM emission, and
operation/report bookkeeping.

Exit: three same-host runs reproduce the stage result and a profile attributes the dominant cost.

### P0.2: Prepare Requirement labels once per render operation

Requirement currently repeats portions of Markdown conversion, HTML sanitization, and label
measurement between layout and SVG emission. Introduce a private prepared label/layout artifact
only after profiling confirms that repeated work dominates.

- Reuse prepared XHTML and exact text metrics across layout and SVG.
- Keep caches operation-scoped and bounded.
- Keep shortcut decisions with the Markdown interpreter or sanitizer policy owner. Do not restore a
  family-local classifier.

Exit: no duplicated semantic work on the measured path, with Requirement goldens, sanitizer tests,
custom text-measurer tests, and DOM parity green.

### P1.1: Add fair reusable and strict cross-runner lanes

The current public-path comparison is intentionally asymmetric: Merman's one-shot helper creates a
render environment per call, while mmdr reuses its theme/config; Merman uses strict render-model
parsing, while the mmdr benchmark uses its permissive parser. Keep the public-path lane, then add:

- a reusable environment/config lane for both implementations; and
- a strict parsing lane where both products perform their documented validation.

Exit: reports name the public, reusable, and strict lanes separately and never merge their ratios.

### P1.2: Compare Mindmap with the same layout algorithm

Run a tidy-tree lane only if both implementations expose equivalent configuration. Do not replace
Merman's default COSE-Bilkent layout with mmdr's radial fallback to improve a benchmark.

Exit: the report separates algorithm cost from parser/render cost and retains default-product
geometry.

### P1.3: Avoid editor-only bookkeeping in render-only parsing

Several typed render parsers reuse semantic constructors that also populate editor facts and lexeme
journals, then discard them. Prefer a shared constructor with a semantic-only or no-op facts sink;
do not fork a second parser.

Exit: semantic models, errors, recovery, spans, and editor output remain identical, while
`parse`, `parse_known_type`, and end-to-end measurements show where the fixed cost moved.

### P1.4: Measure reporting overhead on the string-only SVG path

At `901afd393`, each render session initializes and updates measurement-provenance counters, and
the string-only API constructs a completed report before discarding it. A/B a no-report terminal
path while preserving the report-returning API unchanged. This is a hypothesis, not a proven
cause; reject it if the absolute saving is below the active threshold.

Exit: report APIs retain identical evidence, string APIs retain identical SVG/error behavior, and
the stage benchmark demonstrates any saving.

### P1.5: Optimize large Flowchart scaling

Build a size/density curve rather than tuning against one 420-line fixture. Attribute ordering,
crossing minimization, routing, text measurement, and SVG emission separately. Preserve
source-backed layout semantics and reject magic-number changes made only to improve the benchmark.

Exit: the curve identifies the first superlinear or allocation-heavy stage and a representative
large preview fits the agreed interactive budget.

## Guardrails

- Correctness and Mermaid parity take precedence over a timing ratio.
- Keep native, Node N-API, Node-WASM, and browser-WASM measurements in separate lanes.
- Do not infer a shared cause from unrelated family ratios.
- Do not add family allow-lists, bypass deterministic runtime policy, or remove sanitization.
- Keep exploratory Markdown/JSON under `target/bench`; check in only dated, decision-relevant
  evidence.
