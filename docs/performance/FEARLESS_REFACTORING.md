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

## Current Gap (as of 2026-02-12)

From `target/bench/COMPARISON.latest.md` (generated locally via
`tools/bench/compare_mermaid_renderers.py`):

- End-to-end geometric mean (8 fixtures): ~`9.17x` slower than `mermaid-rs-renderer` (mmdr).
- Medium fixtures (4): ~`4.79x` slower than mmdr.
- Tiny fixtures (4): ~`17.55x` slower than mmdr.

Stage spot-checks (same machine, Criterion, mid estimates):

- `flowchart_medium`:
  - merman: `parse ~7.4ms`, `layout ~31ms`, `render ~10.8ms`, `end_to_end ~49ms`
  - mmdr: `parse ~0.42ms`, `layout ~6.77ms`, `render_svg ~0.24ms`, `end_to_end ~9.7ms`
- `class_medium`:
  - mmdr: `parse ~0.11ms`, `layout ~4.19ms`, `render_svg ~0.20ms`, `end_to_end ~4.11ms`
  - merman remains dominated by parse + SVG emission overhead (see comparison report).

Interpretation:

- For `tiny/*`, fixed overhead dominates (allocation churn, detection/preprocess, JSON/string
  building, SVG emission scaffolding).
- For `flowchart_*`, we pay heavily in all three stages, but SVG emission is especially expensive
  relative to mmdr.
- For `class_*`, parse is the primary outlier (layout is not the bottleneck).

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

### M1: Kill tiny fixed costs (P0/P1, low risk) (2–5 days)

Focus:

- Remove per-call setup costs that dominate `tiny/*`.

Candidates:

- Precompile *all* remaining per-call regexes across diagrams and preprocess/detect.
- Reduce `detect_type` preprocessing allocations (prefer `Cow<'_, str>` / single-buffer builds).
- Avoid repeated `String` clones in hot “scan the whole graph” loops.

Exit criteria:

- Tiny geometric mean ratio improves materially (goal: cut tiny gmean by 2–3x from baseline).

### M2: Make SVG emission cheap (high leverage, medium risk) (4–10 days)

Focus:

- Preserve exact output/DOM parity but reduce allocation + formatting overhead.

Candidates:

- Replace ad-hoc `format!` / intermediate `String`s with a dedicated writer (`fmt::Write`) and
  preallocation.
- Centralize attribute escaping/formatting into a small, reusable “SVG writer” utility.
- Minimize JSON roundtrips in render paths (avoid `serde_json::Value` construction during render).

Exit criteria:

- `render/*` times for flowchart/state/class drop substantially (goal: 2–5x for medium fixtures),
  with DOM parity gate still green.

### M3: Fix class parse as a first-class perf target (high leverage, medium–high risk) (1–3 weeks)

Focus:

- Reduce allocations and data shuffling in the `class` parser and semantic model building.

Candidates:

- Reduce `String` churn (prefer borrowing slices during tokenization where possible).
- Avoid building large `serde_json::Value` trees as the primary internal representation.
- Introduce a typed internal IR for class diagrams, and only convert to JSON at the boundary for
  fixtures/parity (diagram-scoped, incremental).

Exit criteria:

- `parse/class_*` improves by an order of magnitude relative to the current baseline, and
  end-to-end ratio for `class_*` moves closer to the `flowchart_*` ratio band.

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

- Cached hot regexes in class/gantt parsers (`perf(core): cache hot regexes in class/gantt`).
- Reduced dagreish edge-proxy overhead in dugong (`perf(dugong): cut dagreish edge-proxy overhead`).

## Prioritized Backlog

Legend:

- Impact: expected speed improvement for common diagrams (tiny/small).
- Effort: rough engineering effort.
- Risk: likelihood of correctness/parity regressions.

### P0 (High impact, low risk)

1) Precompile detector regexes (no per-call `Regex::new`)
   - Why: current detection compiles regexes on every parse; this dominates tiny diagrams.
   - Change:
     - Replace per-call `Regex::new(...).unwrap().is_match(txt)` with static `OnceLock<Regex>` (or
       equivalent) initialized once per process.
     - Keep detector order and patterns identical to upstream.
   - Impact: very high for `parse_only` on tiny inputs.
   - Effort: low.
   - Risk: low (behavior remains the same).
   - Validation: all guardrails + re-run `cargo bench -p merman --features render --bench pipeline`.

2) Precompile preprocess regexes (no per-call `Regex::new` in preprocessing)
   - Why: preprocessing runs on every parse and historically compiled multiple regexes per call
     (line ending normalization, HTML tag rewrites, entity encoding, frontmatter parsing).
     This is pure fixed overhead on tiny diagrams.
   - Change:
     - Replace per-call `Regex::new(...)` with `OnceLock<Regex>` initialized once per process.
     - Keep preprocessing behavior identical to upstream Mermaid.
   - Impact: very high for `parse_only` on tiny inputs.
   - Effort: low.
   - Risk: low (behavior remains the same).
   - Validation: all guardrails + re-run `cargo bench -p merman --features render --bench pipeline`.

3) Reduce allocations in `detect_type` preprocessing
   - Why: detection currently builds multiple intermediate `String`s (`replace` + directive removal).
   - Change:
     - Avoid unconditional `.to_string()` where possible.
     - Consider a single-pass “clean view” builder into one buffer, or a `Cow<'_, str>` strategy
       that only allocates when needed.
   - Impact: high for tiny/small diagrams.
   - Effort: low–medium.
   - Risk: low–medium (directive/comment stripping must remain identical).
   - Validation: add focused unit tests for directive/frontmatter/comment stripping + all guardrails.

4) Add “known diagram type” parse entrypoints (skip detection)
   - Why: many integrations already know the diagram type (e.g. Markdown fence info string).
   - Change:
     - Provide `Engine::parse_diagram_as_sync(diagram_type, text, opts)` (and async wrapper).
     - Keep existing `parse_diagram*` behavior unchanged.
   - Impact: high for UI/Markdown use cases; no effect for “auto detect” paths.
   - Effort: low.
   - Risk: low (additive API).
   - Validation: add tests for identical outputs vs. auto-detect when type matches.
   - Notes (measured):
     - After detector + preprocess regexes were precompiled (P0.1–P0.2), skipping detection has a
       small effect for the current tiny fixtures (mostly removing a small fixed overhead).
     - Command:
       - `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 50 --warm-up-time 1 --measurement-time 3 --discard-baseline --exact parse_only_sync/<name>`
       - `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 50 --warm-up-time 1 --measurement-time 3 --discard-baseline --exact parse_only_known_type_sync/<name>`
     - Observed medians (2026-02-10, local machine; exact runs):
       - flowchart: `~413 µs` (auto) vs `~407 µs` (known type)
       - sequence: `~56 µs` (auto) vs `~55 µs` (known type)
       - state: `~395 µs` (auto) vs `~416 µs` (known type)
       - class: `~1.347 ms` (auto) vs `~1.423 ms` (known type)

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

Do M0 (stage spot-check + triage workflow), then start M1 (tiny fixed costs).
