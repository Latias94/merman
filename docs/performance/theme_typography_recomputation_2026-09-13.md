---
type: Performance Decision
title: Rebuild resolved typography only when a property wins
timestamp: 2026-09-13
git_commit: 995640a99a96fd1f082f40f624036478d645c8d9
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
tags: theme,allocations,typography,wasm,verification
---

# Decision

Keep the typography winner guard as a structural allocation-work repair. A warmed public SVG
operation with 64 independent Flowchart nodes, 128 paint-only ordinal rules, and 32 font families
allocates 5,811,037 bytes instead of 62,612,317 bytes, a 90.719% reduction. The corresponding peak
heap growth is unchanged. The complete Web render artifact grows slightly; this is not a WASM
size improvement, a latency result, or a release-admission claim.

The base is `57b412693`; the implementation is `995640a99a96fd1f082f40f624036478d645c8d9`.
Native A/B used the same isolated checkout, shared target, macOS ARM64, Rust 1.95.0,
Cargo Release profile and facade default `complete-svg` capabilities. The candidate differed only
in `diagram_theme/resolved.rs`. The ignored probe used the existing native-memory allocator;
no benchmark framework or ordinary-CI job was added.

# Ownership and work bound

Previously, every typography apply or merge cloned the receiver's entire base style and replayed
its resolved patch, including calls made by paint-only rules. The property helpers already owned
the decision whether a specified value superseded its current winner. They now return that decision,
and the caller rebuilds computed typography only when at least one property wins.

All twelve helpers execute; boolean accumulation does not short-circuit. Equal-index writes still
supersede, Clear still restores the receiver's base value, and the base's explicit-property presence
remains intact. No cache key, retained field, public signature, rule origin, validation boundary,
resource charge, or supported mechanism changes.

For N resolved nodes, R matching paint-only ordinal rules per node, and F base-font bytes, the
font-copy work changes from O(N(R+1)F) to O(NF). F is bounded by the protocol's 32 entries and
256 bytes per family. This is a parameterized copy-work bound, not a claim that the whole operation
changes asymptotic class under fixed protocol limits. Candidate checks and charging remain O(NC)
for C candidates; provenance still performs O(NR) tree insertions, each O(log(R+1)) in the worst
case. Actual winning typography changes still rebuild the computed style.

# Allocation observations

The probe renders through the public `Renderer`, using a fixed diagram ID and a warmed renderer.
It measures 18 vectors twice each: one or 32 font families, one/16/128 rules, and empty, paint-only,
or font-size-changing rules. Rules use a period-one selector. The measurement excludes SHA-256
and log generation. All 36 base/candidate SVG hashes and lengths match. Repeated observations
within each vector agree, and allocation count, allocated bytes and peak heap growth never increase.

| 32 fonts, 64 nodes, 128 rules | Base allocated bytes | Candidate allocated bytes | Change | Base / candidate peak growth |
| --- | ---: | ---: | ---: | ---: |
| No rules | 5,353,923 | 5,353,923 | 0 | 828,532 / 828,532 |
| Paint-only ordinal rules | 62,612,317 | 5,811,037 | -56,801,280 (-90.719%) | 840,740 / 840,740 |
| Rules also changing font size | 62,649,181 | 62,208,861 | -440,320 (-0.703%) | 840,740 / 840,740 |

The primary vector's allocation count falls from 315,897 to 43,449. This clears the registered
50% allocated-byte reduction threshold; control allocations and peak growth do not increase.
Peak growth means additional live heap above the measurement starting point, not process RSS.
These BestEffort operations exercise resolver work even when an ordinal request remains residual
at the writer. Identical SVGs do not establish Portable admission or new ordinal typography support.

# Web artifact control and cost

The official `web-render` recipe, package assembler and render-package smoke pass. The artifact
retains `layout-cytoscape`, `layout-elk`, `math`, and `svg`. The unchanged TypeScript distribution
and other-profile build inputs were reused only to run the existing assembler; only the candidate
render profile was rebuilt and tested. This is not a fresh five-profile artifact matrix.

| Metric | Prior final render package | Candidate final render package | Change |
| --- | ---: | ---: | ---: |
| Raw | 13,994,997 | 13,995,236 | +239 |
| Stripped | 13,994,732 | 13,994,971 | +239 |
| Gzip | 5,283,021 | 5,283,131 | +110 |
| Brotli | 3,895,487 | 3,896,289 | +802 |

Accept this small artifact cost explicitly for the allocation repair. The largest relative growth
is about 0.021%; no zero-growth size admission threshold was registered for this structural lane.
**All four render size gates still fail**, and no budget or build flag was relaxed. The reference
artifact comes from `eda8c3846`; only documentation differs between that source and `57b412693`.
Candidate WASM SHA-256: `cd168e845601240f2979f568f0e03a0853ceb22e88d45f2415e29d17b5b66660`.

The first size invocation used a relative package path, which the prebuilt xtask resolved against
its compilation checkout. That invocation remeasured the old artifact and is discarded as candidate
evidence. The corrected invocation uses the absolute candidate package root and reports its exact
artifact path. Initial probe import/private-API compilation errors were corrected before collecting
native measurements; they did not require exposing an internal API.

# Validation and reproduction

Local checks passed: 324 theme unit tests, the complete render-library run (2,517 passed, two
skipped), 29 theme integration tests, render-package production smoke, formatting, and diff checks.
The twelve individual-property cases cover Value and equal-index Clear through both apply and
merge; combined cases cover losing/unspecified inputs and receiver-base ownership. Independent
review found no correctness or evidence findings. Clippy completed with 155 warnings outside the
changed file; this is not a warning-free workspace claim.

The independent checkout `/tmp/merman-typography-995640a99` at the full implementation SHA above
passed the combined library/integration command: **2,546 passed, two skipped**. Its final Git
status is empty, and the source blob matches the committed implementation. This confirms the
selected checks from a clean checkout; it does not upgrade the broader deferred gates.

```text
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=<shared-target> cargo run --locked --release -p merman --features svg --example theme_typography_allocation_probe
CARGO_BUILD_JOBS=2 cargo nextest run --locked -p merman-render --lib --test theme_resolution_svg_test --test diagram_theme_test --test root_canvas_theme_test --test theme_materializer_test --test theme_materialization_operation_test
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=<shared-target> npm run build:wasm:render --prefix platforms/web
node platforms/web/scripts/build-surface-packages.mjs --assemble
node platforms/web/scripts/smoke.mjs --package-id render
<shared-target>/release/xtask wasm-size-matrix --artifact-profile web-render --web-package-root /tmp/merman-typography-57b412693/platforms/web/packages --budget-file <repo>/docs/release/WASM_SIZE_BUDGETS.json
```

The example is an ignored experiment artifact, not a checked-in Cargo target: copy
`target/bench/experiments/theme-typography-recompute-57b412693/allocation-probe.rs` into a temporary
checkout's `crates/merman/examples/theme_typography_allocation_probe.rs` to reproduce it. The ledger
also retains source patches, allocator/probe/binary digests, raw allocation records, the comparison,
and candidate WASM. Logs are `/tmp/theme-typography-{base,candidate}-allocation.jsonl`,
`/tmp/theme-typography-{render-lib,integration,clean-tests,wasm-smoke,clippy}.log`, and the authoritative
`/tmp/theme-typography-wasm-size-candidate.log`. Full workspace, full browser, native raster, and
other artifact-profile runs were not repeated here. C7a/C7b remain open.
