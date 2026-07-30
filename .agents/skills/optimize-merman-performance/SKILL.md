---
name: optimize-merman-performance
description: Diagnose and improve Merman latency, throughput, memory use, input-amplification complexity, dependency closure, or artifact size with evidence matched to the claim and semantics-preserving implementation. Use when investigating a performance regression, profiling a Rust render stage, removing superlinear or attacker-controlled work, optimizing parser/layout/SVG/text paths, comparing Node N-API with Node-WASM, tuning browser WASM or CLI/native artifacts, or deciding whether a proposed fast path, cache, or refactor is a real improvement.
---

# Optimize Merman Performance

## Objective

Deliver a measured workload improvement or a proven resource-bound improvement without changing
Mermaid semantics, output policy, security, resource limits, supported capabilities, or error
behavior. Match evidence to the claim: timing requires same-input A/B; complexity requires explicit
input variables, old/new bounds, and a reachable causal path. Never present one as the other.

## Operating Contract

- Preserve correctness, Mermaid parity, and deterministic policy ahead of speed.
- Compare equivalent operations and capability sets on the same host. Keep native Rust, Node
  N-API, Node-WASM, browser-WASM, and external renderers in separate lanes.
- Select the admission class before editing production code. Register its primary metric or work
  bound, controls, thresholds, corpus, and correctness gates; do not apply one universal latency
  threshold to every performance claim.
- Run Cargo work serially. Use an isolated worktree when the active checkout is dirty or when two
  revisions must be built. Never reset, stash, or overwrite unrelated work.
- Reuse the checked-in benchmark tools. Read [benchmark-surfaces.md](references/benchmark-surfaces.md)
  before selecting or inventing a harness.
- Never add benchmark work to regular CI by default. Use the dedicated Performance workflow only
  when explicitly requested and justified by a stable, bounded gate.

## 1. Define the Question

Classify the work as one primary lane:

- latency or throughput for parse, layout, render, end-to-end, cold start, or concurrency;
- peak or retained memory;
- worst-case or expected complexity and input-amplifiable resource work;
- native, WASM, compressed, installed, or packaged artifact size;
- resolved dependency closure or compile-time cost.

State the exact user workflow, surface, operation, feature or artifact profile, representative
fixtures, and expected benefit. Inspect `docs/performance/PERF_PLAN.md`,
`docs/performance/RUNBOOK.md`, and `docs/performance/BENCHMARKING.md` before creating new work.

Preregister one admission model; do not optimize a percentage or a universal absolute number:

- For ordinary end-to-end latency, use the active plan's joint relative and absolute gate.
- For a complete public operation below the plan's low-latency cutoff, use the noise-adaptive gate
  from `BENCHMARKING.md`. Private stages remain attribution evidence, not admission surfaces.
- For a real frame, throughput, memory, package-size, or high-volume integration objective, freeze
  that workload and budget before confirmation.
- For structural complexity or resource amplification, name every input variable, prove the
  reachable old and new time/space bounds, and use an exact work counter or scale curve when code
  inspection alone is insufficient.

A structural repair may be accepted without an ordinary-fixture speedup when it removes a
higher-order user-controlled term or enforces a resource bound and passes its semantic, security,
resource-accounting, memory, and representative non-regression gates. Report it as a complexity or
resource-bound repair; claim a measured speedup only after the separate latency gate passes.

## 2. Register the Experiment

Create an ignored ledger under `target/bench/experiments/<slug>/experiment.yaml`. Record:

- question, owner, base and target revisions, and dirty-worktree state;
- host, OS, CPU, toolchains, target, profile, lockfile digest, and relevant package versions;
- exact command, fixture or corpus digest, options, capabilities, warmups, samples, repetitions,
  concurrency, and run order;
- admission model, primary metric or work bound, direction, thresholds, and control metrics;
- for structural work, the named input variables, reachable pre-change and post-change time/space
  bounds, expected-versus-worst-case qualifier, and the proof, counter, or scale-curve method;
- semantic, layout, SVG/DOM, security, resource-policy, and error-contract gates;
- each hypothesis, diff or commit, raw samples, outcome (`kept`, `rejected`, or `inconclusive`),
  and reason.

Keep exploratory ledgers and raw output under `target/bench`. Check in only a dated,
decision-relevant checkpoint. Preserve rejected results in the ledger even when their code is
abandoned.

## 3. Establish and Attribute the Baseline

Run correctness first, then collect the registered baseline. Use the `long` preset for latency
exploration. For a timing decision, follow `BENCHMARKING.md`: calibrate both executables with
balanced A/A, derive a fixed power-sized AB/BA confirmation count that is never below eight, and
alternate order. Normalize every result to the same target/base direction.

Split a regression across `parse`, `parse_cold_engine`, `layout`, `render`, and `end_to_end` before
profiling. Add a nearby unaffected fixture or family as a control. Record input/output size and
success/parity status so a smaller result or failed render cannot appear faster.

Use a profiler only after a stage is isolated. Prefer the single-stage `profile_render` example to
profiling the broad Criterion harness. Treat profiler percentages as attribution evidence, not the
acceptance measurement.

If the baseline is noisy enough to overlap the proposed effect, improve the harness, batch the
operation, lengthen the run, or classify the result as inconclusive before changing code.

For a structural complexity lane, baseline the reachable work rather than forcing a clock result:
define the input variables and resource limits, trace the repeated operation, and capture either an
exact test-only work counter or a preregistered multi-scale curve. Wall-clock measurements remain a
secondary workload control unless latency is also an explicit claim.

## 4. Form a Causal Hypothesis

Write one falsifiable sentence connecting an observed cost to a proposed change. Prefer removal of
duplicated work, better ownership, bounded preparation, allocation reduction, or an algorithmic
complexity improvement over parameter tuning.

Do not force a local patch when profiling shows that duplicated ownership or an incorrect stage
boundary is the cause. Allow a deeper refactor, but bound it by the registered metric, public
contracts, and an independently measurable exit condition.

Before implementing a fast path, cache, semantic shortcut, parser bypass, sanitizer bypass, or
family-specific special case, read
[semantic-shortcut-review.md](references/semantic-shortcut-review.md) and construct a counterexample
that would break a merely heuristic classifier.

Accept shortcuts only when the owning layer can prove equivalence, for example:

- an interpreter-defined exact sublanguage with identical output;
- a policy-owned identity condition derived from the complete effective policy;
- an operation-scoped prepared artifact reused by layout and rendering;
- a bounded cache with complete keys, explicit lifetime, and invalidation;
- a source-backed algorithm or data-structure improvement preserving public behavior.

Reject punctuation blacklists, family allow-lists, fixture-specific constants, input-size switches
that alter behavior, caller-owned sanitizer assumptions, or hidden removal of diagnostics,
resource checks, and cancellation.

## 5. Run Isolated Experiments

Use one isolated branch or worktree per independent hypothesis when experiments overlap. Make one
coherent causal change at a time. Do not combine cleanup, capability changes, and performance work
in one measurement.

During each iteration:

1. Run focused semantic and output-contract tests.
2. Run the identical registered quick measurement used for the baseline: A/B, resource probe, or
   work curve according to the admission model.
3. Inspect the primary metric, controls, output/parity, and errors.
4. Mark the experiment kept, rejected, or inconclusive in the ledger.
5. Abandon rejected branches without merging them; do not leave dead fast paths or benchmark-only
   production switches.

Use diagnostics only to localize cost and disable them in the measured production path. Do not
measure debug builds unless debug performance is the explicit user contract.

## 6. Prove the Winner

For an accepted candidate:

- rerun decision-grade A/B with alternating order for latency or throughput claims;
- for memory, artifact size, dependency closure, or compile cost, rerun the frozen controlled
  measurement and its absolute/growth bounds;
- for structural claims, rerun the registered counter/curve and independently review the bound
  derivation;
- run affected crate tests with `cargo nextest`, then expand coverage with blast radius;
- run `cargo fmt --all -- --check` and Clippy with the production feature set;
- run strict verification and SVG DOM parity for shared parse/layout/render behavior;
- test hostile, malformed, Unicode, entity, Markdown/HTML, math/icon, custom configuration,
  custom measurer, resource-limit, and cancellation cases when those paths are relevant;
- run a cross-family control for shared code and the matching stress bench when timing is claimed;
- for structural work, cover the adversarial shape, boundary limits, expected-versus-worst-case
  qualifier, added-space bound, and exact semantic/error/resource-accounting equivalence;
- confirm artifact size, dependency closure, RSS, or installed footprint did not regress when the
  optimization can move those costs.

Reject the candidate even when it is faster if it changes accepted output, errors, security policy,
resource behavior, or capability coverage. Fix correctness first, then rerun the benchmark; never
reuse pre-fix performance numbers.

## 7. Integrate and Hand Off

Commit only the winning implementation and its regression tests. Stage exact owned paths and leave
other worktree changes untouched. Include the experiment ledger path and benchmark command in the
handoff.

Report:

- for timing or size, baseline, candidate, absolute delta, relative delta, thresholds, and raw-run
  count; for structural work, named variables, reachable old/new bounds, evidence method, and added
  space;
- the isolated stage and causal explanation;
- correctness and parity gates;
- rejected hypotheses and why they failed;
- host scope, residual risk, and remaining optimization frontier.

Update `docs/performance/PERF_PLAN.md` only for durable priorities or unresolved regressions.
Update `docs/performance/RUNBOOK.md` only when the executable workflow changes. For a release-facing
comparison, pass the machine-readable evidence to `$writing-great-skills`; do not turn a single
optimization run into a release claim.

## Failure Rules

- Mismatched capability or output contracts: stop and define comparable lanes.
- Dirty or moving comparison refs: isolate them before measuring.
- Failed correctness, parity, sanitizer, resource, or error gate: reject the experiment.
- Material control regression: attribute it before accepting the primary win.
- For a timing claim, a single run, overlapping noise, missing raw samples, or unpinned external
  reference:
  report `inconclusive`.
- Historical locked build failure: record `blocked`; do not regenerate its lockfile in place.
- Missing observation support: add the smallest reusable benchmark, counter, or resource probe only
  when existing surfaces cannot test the primary contract. Keep it separate from production code,
  and remove candidate-only lanes after rejection instead of retaining one-fixture infrastructure.

## Resource Map

- [benchmark-surfaces.md](references/benchmark-surfaces.md): select existing Rust, Node, Web/WASM,
  size, dependency, and profiling tools.
- [semantic-shortcut-review.md](references/semantic-shortcut-review.md): prove or reject fast paths,
  caches, prepared artifacts, and semantic bypasses.
