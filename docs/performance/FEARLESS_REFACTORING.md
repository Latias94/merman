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

Do P0.1 (precompile detector regexes) first.
It is the largest “free” win and should immediately reduce the ms-level fixed cost on tiny diagrams.
