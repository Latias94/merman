# Performance Plan

This is the single current performance backlog for Merman. It records what to optimize and why.
Use [RUNBOOK.md](RUNBOOK.md) for the execution loop and [BENCHMARKING.md](BENCHMARKING.md) for
measurement semantics and tool details. Dated reports are evidence for a particular revision and
host; they are not rolling sources of truth.

## Current evidence

The current release-range baseline was measured on 2026-07-27:

- [Alpha.3 to Alpha.4 Refactoring Report](../release/ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md)
  compares `v0.8.0-alpha.3` with `d2698d0a3`.
- [Renderer comparison, 2026-07-27](renderer_comparison_2026-07-27.md) compares that candidate
  with Mermaid.js 11.16.0 and `mermaid-rs-renderer` at `7ff1196`.

| Comparison lane | Shared rows | Median ratio | Geometric mean | Faster / slower |
| --- | ---: | ---: | ---: | ---: |
| Current / alpha.3, complete SVG product | 34 | 1.10x | 1.09x | 13 / 21 |
| Current / alpha.3, minimal same-capability SVG | 32 | 1.12x | 1.07x | 10 / 22 |
| Current / `mermaid-rs-renderer` | 32 | 0.697x | not reported | 19 / 13 |
| Current native / warm Mermaid.js browser | 34 | 0.0237x | not reported | 34 / 0 |

The cross-runner rows are not quality-adjusted rankings. Merman and `mermaid-rs-renderer` have
different Mermaid coverage and output goals, and native Rust versus warm Chromium is not a
browser-WASM comparison.

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
| P0 | `requirement_medium` | 337 us | 2.12-2.25x (+178-184 us) | 4.58x | Both complete and minimal capability lanes reproduce the regression. |
| P0 | `kanban_medium` | 153 us | 3.88-4.08x (+114-118 us) | 5.22x | Both capability lanes reproduce a high family-local fixed cost. |
| P1 | `flowchart_large` | 24.10 ms | about 1.00x | unavailable | This is not a refactor regression, but it is the only standard fixture beyond a 16.7 ms frame. |
| P1 | `class_medium` | 950 us | 1.10-1.15x (+89-126 us) | 0.40x | A common family with a modest alpha.3 regression; Merman remains about 2.5x faster than mmdr. |

The alpha.3/current confidence intervals for Requirement and Kanban did not overlap in the release
measurements. These families also gained Mermaid 11.16 semantics, so optimization must preserve the
additional work rather than reverting it.

Do not prioritize the current Info, Packet, Radar, or Sankey ratios without new absolute-cost
evidence. Their measured alpha.3 increases are about 2-5 microseconds. Architecture,
`flowchart_small`, Gantt, and `flowchart_ports_heavy` are within the current noise band.

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

### P0.1: Establish stage-level evidence

- Run parse, layout, render, and end-to-end measurements for Requirement and Kanban with 30-50
  samples, a two-second warm-up, and a three-second measurement window.
- Use Instruments Time Profiler and Allocations only for the stage that remains slow.

Exit: each remaining P0 family has a repeated same-host A/B result and a profile-backed cause.

### P0.2: Remove redundant configuration ownership

Static code review found two low-risk hypotheses:

- Kanban constructs an owned `KanbanMarkdown` configuration for layout and again for SVG emission.
- Requirement clones the effective configuration in its SVG path.

Change these private helpers to borrow the operation configuration where lifetimes remain local.
Measure `layout`, `render`, and `end_to_end` independently; preserve Kanban/Requirement goldens,
sanitization behavior, and custom text-measurer behavior.

Exit: the targeted configuration clones are gone and the stage benchmark demonstrates the effect.

### P0.3: Prepare labels once per render operation

Kanban and Requirement currently repeat portions of Markdown conversion, HTML sanitization, and
label measurement between layout and SVG emission. Mindmap still repeats work for complex labels,
including paths where FontAwesome replacement can make measured and emitted content diverge.
Introduce a private prepared label/layout artifact only after profiling confirms the remaining
repeated work dominates.

- Reuse prepared XHTML and exact text metrics across layout and SVG.
- Keep caches operation-scoped and bounded.
- Keep shortcut decisions with the Markdown interpreter or sanitizer policy owner. Do not restore a
  family-local classifier.

Exit: no duplicated semantic work on the measured path, with family goldens and DOM parity green.

### P1.1: Avoid editor-only bookkeeping in render-only parsing

Several typed render parsers reuse semantic constructors that also populate editor facts and lexeme
journals, then discard them. Prefer a shared constructor with a semantic-only or no-op facts sink;
do not fork a second parser.

Exit: semantic models, errors, recovery, spans, and editor output remain identical, while
`parse`, `parse_known_type`, and end-to-end measurements show where the fixed cost moved.

### P1.2: Optimize large Flowchart scaling

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
