# Fearless Refactoring Plan (Performance, With Correctness First)

This document is a prioritized, “fearless refactoring” backlog for improving `merman` performance
while preserving correctness.

## Goals

- Improve performance without regressing Mermaid parity guarantees.
- Keep changes incremental and measurable.
- Make it obvious what is safe vs. risky, and how to validate each step.

## Non-goals

- No “rewrite everything” plans.
- No `unsafe` (the repo forbids unsafe code).
- No performance claims without measurements.

## Correctness Guardrails (must stay green)

- Formatting: `cargo fmt --all`
- Lints: `cargo clippy --workspace --all-targets -- -W clippy::all`
- Tests: `cargo nextest run`
- DOM parity gate (upstream SVG baselines):
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
- Golden layers:
  - Semantic: `fixtures/**/*.golden.json`
  - Layout: `fixtures/**/*.layout.golden.json`

## Measurement Guardrails

- Use Criterion for local benchmarking: `docs/performance/BENCHMARKING.md`.
- Compare revisions on the same machine; avoid cross-machine comparisons.
- Prefer stage breakdown (parse vs layout vs SVG emission), then end-to-end.
- For SVG emission hotspots, use internal breakdown timings when available (see “Micro-timing” below).

## Current Gap (as of 2026-02-12)

From a local comparison run on a single machine (see `docs/performance/COMPARISON.md`,
generated via `tools/bench/compare_mermaid_renderers.py`):

- End-to-end geometric mean (8 fixtures): ~`6.8–6.9x` slower than `mermaid-rs-renderer` (mmdr).
- Medium fixtures (4): ~`3.1x` slower than mmdr.
- Tiny fixtures (4): ~`15.2x` slower than mmdr.

Stage spot-checks (same machine, Criterion, mid estimates; generate via
`python tools/bench/stage_spotcheck.py --fixtures flowchart_tiny,flowchart_medium,state_tiny,state_medium,class_tiny,class_medium,sequence_tiny,sequence_medium --out target/bench/stage_spotcheck.md`):

- `parse`: ~`10x` geometric mean slower than mmdr.
- `layout`: ~`2.9x` geometric mean slower than mmdr.
- `render`: ~`12–13x` geometric mean slower than mmdr.
- `end_to_end`: ~`4.8x` geometric mean slower than mmdr.

Interpretation:

- `render/*` is now the primary global outlier (especially `state_*` and `flowchart_*`).
- `parse/*` still has outliers for some tiny fixtures, but is no longer the dominant medium-fixture cost.
- `layout/*` is not the dominant stage overall, but it is the largest absolute cost for `flowchart_medium`
  and therefore still a worthwhile target after the SVG emitter hotspots are under control.

## Micro-timing (render sub-breakdowns)

When optimizing SVG emission, stage timing alone is often too coarse. `merman-render` supports an
internal breakdown that can be enabled via:

- `MERMAN_RENDER_TIMING=1`

Example (single fixture run):

- `cargo run -p merman-cli --release -- render --text-measurer vendored crates/merman/benches/fixtures/flowchart_medium.mmd > $null`

Typical interpretation (varies by machine/fixture):

- `flowchart-v2`: `render_svg` is dominated by per-node SVG emission (`nodes`).
- `stateDiagram`: `render_svg` is dominated by leaf node emission (`leaf_nodes`) rather than edges.

## Milestones (revised)

These milestones are designed to steadily reduce the merman/mmdr ratio while preserving parity
guardrails. Each milestone should be done as a small series of PRs/commits, with a comparison
report refreshed at the end.

### M0: Make hotspot evidence cheap (1–2 days)

Deliverables:

- A documented “stage spot-check” command set (flowchart_medium + class_medium) that runs:
  `parse/*`, `layout/*`, `render/*`, `end_to_end/*` for merman and mmdr.
- A repeatable “perf triage” workflow (Criterion filter + stable params) so we can answer:
  “which stage got faster/slower?” quickly.

Exit criteria:

- We can attribute each top regression to a specific stage within minutes.

### M1: Fix parser fixed costs (highest leverage, medium risk) (1–3 weeks)

Focus:

- Make `parse/*` competitive for `class_*` and `state_*` without relaxing strictness.

Candidates:

- Reduce preprocess/detect allocation churn (prefer `Cow<'_, str>` / single-buffer builds where safe).
- Stop using large `serde_json::Value` trees as the primary internal representation for hot parsers.
- Introduce typed IR for `class` and `state` parsers (diagram-scoped, incremental), and only convert
  to JSON at the fixture/parity boundary.
- Enable a conservative “fast parser” by default where it can safely decline and fall back to the
  strict parser (keep an escape hatch like `MERMAN_CLASS_PARSER=slow` for bisect/debug).

Exit criteria:

- `parse/class_medium` and `parse/state_medium` improve by an order of magnitude vs baseline.

### M2: Make SVG emission cheap (highest leverage, medium risk) (4–10 days)

Focus:

- Preserve exact output/DOM parity but reduce allocation + formatting overhead.

Candidates:

- Eliminate per-number `String` allocations in path/points emission (write directly into the output buffer).
- Replace hot `format!` / intermediate `String`s with a dedicated writer (`fmt::Write`) and preallocation.
- Centralize attribute escaping/formatting into a small, reusable “SVG writer” utility.
- Minimize JSON roundtrips in render paths (avoid `serde_json::Value` construction during render).

Exit criteria:

- `render/*` times for flowchart/state/class drop substantially (goal: 2–5x for medium fixtures),
  with DOM parity gate still green.

### M3: Reduce flowchart layout overhead (high impact, medium risk) (4–10 days)

Focus:

- Cut `layout/flowchart_medium` absolute time (it dominates flowchart end-to-end).

Candidates:

- Reuse buffers between passes (crossing minimization / ordering / routing).
- Reduce `HashMap` churn in inner loops (prefer `IndexMap`/stable indexing where possible).
- Use profiling to identify the top per-pass offenders before attempting large refactors.

Exit criteria:

- `layout/flowchart_medium` ratio improves materially (goal: 2x+), without correctness regressions.

### M4: Make dugong’s dagreish pipeline index-based (highest potential, highest risk) (2–6 weeks)

Focus:

- Keep the external API keyed by `String` IDs for compatibility/parity, but run heavy layout
  algorithms on compact indices to eliminate hash map lookups and string cloning in inner loops.

Strategy:

- Add a `GraphView`/`GraphIndex` layer:
  - map `node_id: String -> NodeIx(u32)` once per layout run
  - store adjacency as `Vec<Vec<NodeIx>>`/CSR and store labels in parallel arrays
  - translate results back to the external graph at the end
- Start with the most expensive subpipeline (ordering / crossing minimization), then expand.

Exit criteria:

- `layout/flowchart_medium` ratio improves materially (goal: 2x+), without correctness regressions.

## Completed (recent)

- Skip expensive HTML-sanitizer passes for strict-mode plain text (`sanitize::remove_script` fast-path).
- Cached hot regexes in class/gantt parsers (`perf(core): cache hot regexes in class/gantt`).
- Reduced dagreish edge-proxy overhead in dugong (`perf(dugong): cut dagreish edge-proxy overhead`).
- Made SVG number/path formatting allocation-free (`fmt_display`, `fmt_path_into`, curve/path emit refactors).
- Reduced allocations in flowchart/state SVG emission (escape display wrappers, fewer intermediate `String`s).
- Added an internal render breakdown switch (`MERMAN_RENDER_TIMING=1`) to cheaply localize SVG hotspots.
- Reduced flowchart node render allocations by borrowing node inputs and avoiding style string cloning.
- Cached RoughJS path generation within state leaf-node rendering (per-render cache; avoids repeated `roughr` work).
- Optimized state `parity-root` bbox scan to skip `<style>/<defs>` and reuse transform parse buffers.

## Prioritized Backlog

Legend:

- Impact: expected speed improvement for common diagrams (tiny/small).
- Effort: rough engineering effort.
- Risk: likelihood of correctness/parity regressions.

### P0 (High impact, low risk)

1) Reduce preprocess/detect allocation churn (single-buffer strategy)
   - Why: preprocess currently performs multiple whole-string passes (`replace_all(...).to_string()`),
     which becomes fixed overhead for small diagrams and non-trivial overhead for medium ones.
   - Change:
     - Prefer `Cow<'_, str>` / “only allocate when needed” transforms.
     - When allocation is required, build into a single buffer per stage (avoid 2–4 full copies).
   - Impact: high for `parse/*` on tiny/small; medium for medium fixtures.
   - Effort: low–medium.
   - Risk: low–medium (must preserve upstream quirks).
   - Validation: focused unit tests for preprocess (entities/directives/frontmatter) + guardrails.

2) Keep “known diagram type” fast-path healthy (already exists)
   - Why: many integrations know the diagram type (Markdown fences).
   - Change:
     - Maintain and benchmark `parse_known_type/*` alongside `parse/*`.
     - If regressions appear, consider API layering changes so `parse_diagram_as_sync` avoids any
       detection-only setup work.
   - Impact: medium (integration-dependent).
   - Effort: low.
   - Risk: low.
   - Validation: `parse_known_type/*` benches + golden/parity.

### P1 (Medium impact, low–medium risk)

4) Text measurement caching (hot path during layout)
   - Why: layout calls `TextMeasurer::measure` frequently; repeated identical strings are common.
   - Change:
     - Add an internal cache keyed by `(text, style)` in `VendoredFontMetricsTextMeasurer`.
     - Ensure cache is deterministic and bounded (LRU or size cap) to avoid unbounded memory growth.
   - Impact: medium–high depending on diagram (flowchart, gantt, kanban).
   - Effort: medium.
   - Risk: low–medium (must not change measured values; must remain deterministic).
   - Validation: guardrails + add a benchmark fixture with many repeated labels.

5) Cut down JSON cloning/serialization in hot paths
   - Why: some stages may clone `serde_json::Value` trees.
   - Change:
     - Avoid `to_value`/`from_value` roundtrips where possible.
     - Use references and structured accessors instead of cloning objects.
   - Impact: medium.
   - Effort: medium.
   - Risk: medium (easy to accidentally change normalization behavior).
   - Validation: guardrails + snapshot comparisons.

### P2 (High impact, higher risk; do only with discipline)

6) Move semantic models to typed structs internally; keep JSON as an interchange layer
   - Why: `serde_json::Value` is convenient for parity snapshots but expensive at runtime.
   - Strategy (incremental, diagram-by-diagram):
     - Introduce typed models for one diagram (e.g. flowchart) behind internal modules.
     - Parse into typed, then export to JSON for snapshots and existing APIs.
   - Impact: potentially high.
   - Effort: high.
   - Risk: high (model drift vs upstream; snapshot stability).
   - Validation:
     - Diagram-scoped golden tests + DOM parity gate + additional roundtrip tests (typed -> JSON).

7) SVG emission rework to reduce allocations
   - Why: constructing many intermediate strings is expensive; `format!` can be costly.
   - Change:
     - Use a dedicated writer with preallocation and `fmt::Write`.
     - Keep output ordering stable to preserve parity.
   - Impact: medium–high depending on diagram size.
   - Effort: medium–high.
   - Risk: medium–high (SVG DOM parity is sensitive).
   - Validation:
     - DOM parity gate must stay green.
     - Add an “SVG size stress” benchmark for large graphs.

### P3 (Niche / optional)

8) Add a profiler workflow
   - Why: once P0/P1 are done, improvements require real profiling to find true hotspots.
   - Change:
     - Document `criterion --profile-time` usage.
     - Optionally add a `pprof`-based dev workflow (documented only; not required for all users).
   - Impact: indirect.
   - Effort: low.
   - Risk: low.

## Recommended Next Step

Keep M0 tooling green (comparison + stage spot-check), then prioritize M1 (parser fixed costs),
starting with `class_*` and `state_*` typed IR work while keeping fixture/golden boundaries intact.
