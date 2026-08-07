---
name: contribute-merman-pr
description: Prepare and validate a focused Merman pull request with change-scoped Rust, FFI, feature/dependency, generated-license, workflow, platform, and performance checks. Use when opening, updating, reviewing, or repairing a PR, deciding whether a CI gate is redundant, or handing off a branch for review.
---

# Contribute Merman PR

Use this skill to make a branch review-ready while keeping evidence proportional to the changed
surface. It owns local validation and CI triage; release publication, registry mutation, and tag
creation remain separate authorized work.

## Read the authority

Read these before changing a check or interpreting a failure:

- `AGENTS.md` and any nearer repository instructions;
- `docs/development/CI.md`;
- the relevant owner workflow under `.github/workflows/`;
- `docs/release/FFI_CONTRACT_READINESS.md` for FFI, artifact-profile, or dependency-boundary work.

Treat workflow files, artifact descriptors, and generators as authority. Treat checked-in reports
and projections as derived outputs. Completion criterion: the changed surface, its owner workflow,
and the expected evidence are named before edits begin.

## Establish scope and ownership

1. Capture `git status --short`, the current branch, `git diff --stat`, and `git diff --name-only`.
2. Inspect the diff, including generated files and workflow conditions; preserve unrelated user edits.
3. Classify paths into Rust behavior, FFI/platform bindings, Cargo/features, generated/legal
   projections, workflows/scripts, Web/Playground, or performance tooling.
4. Select the smallest owner test for each class, then add a wider gate only when a shared contract
   changed. Run Cargo commands sequentially with the repository's pinned toolchain and prefer
   `cargo nextest` for Rust tests.

Completion criterion: every changed path has an owner check or an explicit reason why no local
check is needed.

## Run the baseline gates

Run:

- `git diff --check` for every PR;
- `cargo fmt --all -- --check` when Rust, Cargo manifests, or Rust-generated sources changed;
- focused `cargo nextest run` for affected crates/tests, using `--locked` when the workflow does;
- `python3 -m py_compile` for changed Python scripts and the narrowest relevant Python unit tests.

Do not run every matrix combination locally by habit. The central CI workflow owns the broad default
workspace test; local work should prove the changed seam first and record any intentionally deferred
lane. Completion criterion: baseline checks pass and deferred checks have a stated owner and reason.

## Refresh generated legal material in dependency order

Run this chain whenever `Cargo.lock`, dependency declarations, artifact profiles, native binding
features, or license/report inputs change. A report-only refresh does not by itself require the
FFI/Cargo feature matrix below:

```text
python3 scripts/generate-rust-license-report.py --check
python3 scripts/sync-release-legal-materials.py --check
python3 scripts/verify-third-party-licenses.py
python3 -m unittest scripts.test_generate_rust_license_report scripts.test_sync_release_legal_materials
```

When the first command reports stale or missing reports, run its existing `--write` mode, inspect
the semantic diff, and rerun `--check`. Then refresh release projections with the second command's
`--write` mode and rerun both checks. Never hand-edit generated JSON or update a projection before
its source report. The report command invokes the pinned `cargo-about` generator, so an environment
that forbids Cargo must record this gate as deferred rather than claiming a Python-only pass. This
order catches both report drift and stale copies embedded in packages.

Completion criterion: source reports, release projections, and the third-party contract all pass;
the diff contains only generator-owned changes justified by the current lock/features.

## Route feature, dependency, and FFI changes

For feature or artifact-profile changes, run the representative PR closure and feature graph checks:

```text
python3 scripts/verify_artifact_dependency_closures.py --representative-targets
cargo run --locked -p xtask -- verify-feature-matrix
```

For FFI or native SDK changes, also run:

```text
cargo run --locked -p xtask -- verify-native-abi
cargo run --locked -p xtask -- verify-binding-contract
python3 scripts/verify-platform-bindings.py
```

For a changed `Cargo.lock` or external dependency, run the policy owner when `cargo-deny` is
available:

```text
cargo deny check advisories bans licenses sources
```

Security Audit owns this check in CI; record it as deferred when the local tool is unavailable
instead of replacing it with an ad hoc dependency parser. Derive feature strings from
`capabilities/artifact-profiles-v1.json` or the owner workflow. For a real public-consumer proof,
reuse the exact recipe in `.github/workflows/ci.yml` (C `c_consumer_smoke`, the Apple UniFFI smoke,
or the platform binding verifier) rather than inventing a second feature list. For an intentional
breaking API, update all in-repository consumers and tests to the new contract; do not restore a
compatibility alias merely to make an old test compile.

Completion criterion: the descriptor, Rust library, generated bindings, and at least one real public
consumer agree on the same feature/API contract.

## Validate workflow and performance changes

For `.github/workflows/**` or workflow-contract script changes, run the targeted contract suites:

```text
python3 -m unittest scripts.test_release_workflow_security scripts.test_workflow_path_filters scripts.test_ci_workflow_android_emulator
```

Run `actionlint` when it is installed; the targeted Python suites remain the repository-specific
semantic check when it is not. Validate only the workflow files touched by the change and keep the
workflow contract tests in the same commit as a changed gate condition.

For `tools/bench/**` changes, run the Python performance contracts:

```text
python3 tools/bench/test_perf_baseline_manifest.py
python3 tools/bench/test_native_memory_contracts.py
python3 tools/bench/test_native_memory_driver_contracts.py
python3 tools/bench/test_perf_contracts.py
```

Keep compiled Criterion discovery and native-memory probes on the performance-labelled, main,
scheduled, or manual lanes. When that lane is explicitly in scope, use the complete pipeline
capability set:

```text
python3 tools/bench/verify_pipeline_bench_list.py --features svg,layout-cytoscape,layout-elk,math
```

A high-cost check earns a standing PR slot only when it detects a unique merge-blocking failure.
Otherwise narrow it to unique tests or move it to the owner performance lane, and add or update the
workflow contract test that proves the condition. Keep fixture capability failures visible: repair
the feature recipe or the lane rather than silently skipping an unsupported fixture.

Completion criterion: each workflow step has a unique claim, a tested condition, and a cost-appropriate
owner lane.

## Triage CI failures

Use `gh pr checks` followed by `gh run view <run-id> --log-failed` to inspect the current head SHA.
Classify each failure as one of:

- source/API regression: update the current contract and its consumers;
- generated drift: regenerate the source artifact, then its projections;
- feature-recipe mismatch: fix the recipe or move the gate to the lane with the required capability;
- redundant/high-cost gate: preserve the unique assertion while narrowing or gating the step;
- runner/toolchain/network failure: document it as environment evidence and avoid weakening source gates.

Fix the first causal failure, rerun the smallest reproducer, then rerun the owner workflow. Preserve
unrelated working-tree edits and use precise file staging. Completion criterion: every red check is
green, skipped by an intentional documented condition, or shown to be external to the branch with
reproducible evidence.

## Hand off the PR

Before committing or opening a PR:

- inspect `git diff --check` and the staged file list;
- separate source, generated, and workflow changes into focused Conventional Commits;
- summarize changed public contracts, feature/dependency impact, checks run, and intentional skips;
- keep the PR body free of Compound Engineering badges;
- push or open the PR only when explicitly authorized.

Completion criterion: the branch is reviewable, generated outputs are fresh, evidence is reproducible,
and no publication or release action was performed implicitly.
