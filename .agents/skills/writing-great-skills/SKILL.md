---
name: writing-great-skills
description: Build evidence-backed, user-facing Merman release reports by comparing a configurable base release tag and target revision. Use for alpha/beta/stable release analysis, changelog-ready summaries, dependency and Cargo feature comparisons, native/WASM/Web/Node artifact sizes, Rust/Mermaid.js/mermaid-rs-renderer benchmarks, diagram-family support matrices, migration guidance, and scenario-based crate/package recommendations.
---

# Release Evidence Reports

## Objective

Produce a concise report that answers what changed for users, what each workflow costs, which capability/package to choose, and what remains unproven. Treat the report as a release delta, not a commit transcript. Keep source facts, measurements, interpretation, and recommendations visibly separate.

## Default Contract

- Compare `TARGET=HEAD` with the nearest reachable release tag matching `v*` before `TARGET`.
- Accept explicit `--base` and `--target` overrides whenever the caller names a range.
- Verify both refs resolve and `BASE` is an ancestor of `TARGET`; stop with a diagnostic when they do not.
- Identify whether an intermediate prerelease was published. Do not fold audience-visible behavior across a publication boundary without saying so.
- Pin the measurement environment: commit, tag, host, OS, CPU, Rust toolchain, Node version, package manager, optimization profile, target triple, lockfile digest, corpus digest, warmups, iterations, and concurrency.
- Record `measured`, `not-applicable`, `unavailable`, `blocked`, and `inconclusive` distinctly. Never substitute a budget, estimate, or old receipt for a measurement.
- Treat `crates/xtask/src/cmd/admission.rs` as the authority for primary SVG admission record
  counts. Keep record IDs separate from logical family groups; the human status table may
  summarize an older or narrower view.

Use the bundled collector for a reproducible source ledger:

```console
python3 .agents/skills/writing-great-skills/scripts/collect_release_facts.py \
  --base v0.8.0-alpha.3 \
  --target HEAD \
  --manifest merman \
  --manifest merman-cli \
  --manifest merman-wasm \
  --artifact web-full=platforms/web/packages/full/artifacts/wasm/merman_wasm_bg.wasm \
  --artifact web-render=platforms/web/packages/render/artifacts/wasm/merman_wasm_bg.wasm \
  --output /tmp/merman-release-facts.json
```

The collector is intentionally read-mostly. It gathers refs, diff inventory, manifest feature/dependency facts, SVG admission records, tool versions, and explicitly supplied artifact sizes. Run Cargo and JavaScript benchmarks separately using [benchmark-matrix.md](references/benchmark-matrix.md), then merge their raw JSON into the evidence ledger before writing prose.

## Workflow

### 1. Establish the release delta

Run the collector and inspect its `comparison`, `changed_paths`, and `first_parent_commits` fields. Read `CHANGELOG.md`, the nearest published section, `docs/release/PACKAGE_SURFACES.md`, `docs/FEATURES.md`, and the relevant ADR before interpreting feature changes. Use `git diff BASE..TARGET` as the final-state source; use history only to explain intent and publication exposure.

### 2. Build the capability and packaging ledger

Compare these authorities instead of inferring behavior from names:

- Language and family support: `crates/merman-core/src/family.rs`, `docs/alignment/STATUS.md`, and `docs/alignment/ADMISSION_INVENTORY.md`.
- Public Cargo vocabulary: `capabilities/feature-surface-v1.json`, generated capability projections, and each package `Cargo.toml`.
- Exact release closure: `capabilities/artifact-profiles-v1.json`, `docs/release/PACKAGE_SURFACES.md`, and `docs/release/WASM_SIZE_BUDGETS.json`.
- Browser package behavior: `platforms/web/web-surface-descriptor.json`, package manifests, and package contract tests.
- Node status: `platforms/node/candidate-builds.json`, `platforms/node/README.md`, and `docs/performance/NODE_TRANSPORT_ADMISSION.md`. Do not call a private candidate a shipped product.
- ASCII support: `docs/rendering/ASCII_SUPPORT_MATRIX.md` and runtime
  `ascii_capabilities`; never infer ASCII support from SVG admission.
- Distribution channels: `docs/release/SURFACES.json` and
  `docs/release/PACKAGE_SURFACES.md`. Also record the generated README installation mode reported
  by `python3 scripts/release-version.py`. An artifact profile or README registry projection is a
  build/release contract, not proof that a registry channel is live. Use the release-status probe
  or registry evidence before describing an untagged candidate version as published.
- Typst: `packages/typst/merman/README.md`, `typst.toml`, and the `typst-wasm` artifact profile.
  Record the independent package-to-Merman version mapping and do not infer browser or math
  capabilities.

For each surface, record `BASE state`, `TARGET state`, `audience`, `capability IDs`, `direct Cargo features`, `resolved dependency closure`, `artifact bytes`, and `migration action`. Distinguish:

- Direct manifest dependencies from the transitive `cargo tree` closure.
- Cargo features from artifact profiles; feature unification means a feature name is not a size guarantee.
- Logical diagram families from aliases/variants and from `parse`, `layout`, `render`, or `editor` admission.
- A complete product growing because it gained semantics from a workflow-specific slim artifact getting smaller.
- Published registry packages from GitHub artifact-only builds, credential-blocked channels, and
  source-only crates.

### 3. Measure representative workflows

Follow [benchmark-matrix.md](references/benchmark-matrix.md). At minimum collect:

- Native Rust pipeline: Merman, `mermaid-rs-renderer`, and Mermaid.js on the same corpus, diagram mix, output contract, and warmup/iteration policy.
- Revision A/B: run both the release-default product path and a same-capability
  `--no-default-features` path. A default that gained layouts or math is a product change, not
  automatically an implementation regression.
- CLI: release/default and analysis-only binaries, with raw and stripped bytes and resolved dependency counts.
- Browser WASM: exact Web artifact profiles; record raw, stripped, gzip, and Brotli bytes. Compare only the same capability contract.
- Node: the repository harness for N-API versus Node-targeted WASM, plus a semantic-only `semantic-json` pass when parse latency is requested. Keep end-to-end SVG latency as a separate metric.
- Native SDK: report measured SKUs only. If the repository has intentionally retained one full SKU and lacks an all-target baseline, say so instead of inventing a slim-SKU win.

Prefer repository commands and checked-in corpora. Use isolated temporary worktrees for historical builds. Never change the user's worktree to generate an old lockfile or artifact. When a build fails under `--locked`, preserve the exact error and classify the result as `blocked`; do not silently regenerate the lockfile in the source checkout.

When a same-capability revision lane regresses materially, split parse, layout, SVG emit, and
end-to-end timing before writing the verdict. Confirm a suspected shared cost with one isolated
A/B that preserves the input and benchmark harness. Record a proven regression, its semantic
repair constraints, and its priority in `docs/performance/PERF_PLAN.md`; keep the runnable
validation gates in `docs/performance/RUNBOOK.md`. Do not hide it behind aggregate improvements or
apply a diagnostic shortcut as a production fix.

### 4. Write the user-facing report

Use [report-template.md](references/report-template.md). Lead with a verdict and a compact table. Explain benefits in user terms:

- "lint/CI now ships only parser and diagnostics code" is a user outcome.
- "feature unification moved from crate A to crate B" is implementation evidence supporting that outcome.
- "full Web grew" is an honest compatibility/coverage consequence, not a regression claim without a same-contract comparison.

Include a scenario table using [scenario-matrix.md](references/scenario-matrix.md), covering at least static blog generation, dynamic browser rendering, editor/LSP, lint/CI, Markdown rendering, terminal/ASCII, Node/SSR, and Typst. For every recommendation show the crate or package, exact feature/package selection, why it fits, and what it does not include.

End with migration notes and a "what is still unproven" section. Surface breaking names (`full` -> capability leaves, `render` -> `svg`, `ratex-math` -> `math`, ABI changes, Web subpaths -> standalone packages) only when the compared range affects the reader.

### 5. Validate and extract changelog material

Trace each table cell to a raw fact or command output. Recompute percentages from stored bytes, check that every benchmark uses the same corpus and toolchain, and label incomparable numbers. Keep one physical line per Markdown bullet or paragraph. Extract only net user outcomes into `CHANGELOG.md`; leave method detail in the report or a linked artifact.

Run the collector's smoke test and the skill validator:

```console
python3 scripts/release-version.py
python3 .agents/skills/writing-great-skills/scripts/collect_release_facts.py \
  --base v0.8.0-alpha.3 --target HEAD --output /tmp/facts.json
python3 /Users/frankorz/.codex/skills/.system/skill-creator/scripts/quick_validate.py \
  .agents/skills/writing-great-skills
```

## Failure Rules

- Ref resolution or ancestry failure: stop and request the intended range.
- Missing historical artifacts: report "not available from source history" and offer an isolated rebuild; do not use current artifacts as historical evidence.
- Mismatched capability contracts: do not calculate a percentage as if it were a like-for-like size win.
- Semantic/SVG mismatches between transports: report parity failure before discussing speed.
- Raw JSON key-order differences between Node transports: canonicalize parsed JSON objects
  recursively before declaring a semantic mismatch; keep arrays in order.
- A single-host Node result: call it host evidence, not product admission.
- A failed locked build: report the lockfile or dependency failure as a release risk.
- External implementation absent or unpinned: report the benchmark as unavailable, not zero.

## Resource Map

- [benchmark-matrix.md](references/benchmark-matrix.md): commands, fairness rules, and metric schema for native, JS, Web, and Node measurements.
- [scenario-matrix.md](references/scenario-matrix.md): required workflow coverage and range-specific migration fields.
- [report-template.md](references/report-template.md): changelog-friendly report structure with compact tables.
- `scripts/collect_release_facts.py`: deterministic release-range and source-fact collector.
