---
type: Performance Decision
title: Reuse the family evidence requirement index
timestamp: 2026-09-13
git_commit: d92dee3ade9e83fdb519e8d1e908ca2da27d041a
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
tags: theme,complexity,evidence,wasm,verification
---

# Family theme evidence requirement indexing

Date: 2026-09-13. Base: `973e7c893de47ef61977e4480b854ef4361b46a1`.
Candidate: `d92dee3ade9e83fdb519e8d1e908ca2da27d041a`. Host: macOS ARM64, Rust 1.95.0. The experiment ledger is
`target/bench/experiments/theme-evidence-index-973e7c893/experiment.yaml`.

## Decision and mechanism

Accepted as a structural work-bound repair after scoped and complete private owner tests,
actual Typst package checks, SVG structure comparison and artifact non-regression controls.
No measured latency or peak-memory improvement is claimed.

`ResolvedDiagramTheme` previously collected native route keys into a vector with `contains`,
then `FamilyThemeEvidence` independently built its required-key BTreeSet from that vector.
Multiple family plans construct these ledgers. The candidate yields keys in route order and lets
the ledger's existing membership index decide whether to append each key. It preserves the
ordered requirement list, excludes LegacyCompatibility before insertion, and keeps a shared
rule key whenever any sibling facet belongs to native evidence. Accounting and report merging
are unchanged. The applicability check now stops on the first native route.

Let T be all mechanism routes, R the native-owned routes, U their unique keys, L the maximum
key comparison/copy cost, and P the number of ledger constructions. The old bound is
`O(P * (T + (R*U + U*log(U+1))*L))`; the new bound is
`O(P * (T + R*log(U+1)*L))`. Both retain `O(U*L)` space per ledger. The candidate adds no
index, cache, persistent state or resource-policy exception. The applicability check is at worst
`O(T+L)`, including legacy-only inputs; it does not become constant time.

The public Rust theme compiler accepts the test's 512 Node rules with two facets each. They yield
1,024 routes and 512 ordered rule keys. The baseline consecutive-facet vector searches perform
262,144 key equality checks by direct source/corpus derivation; this is not an instrumented
counter or a timing result. Effect IDs retain their complete string and target identity.
The current operation meter did not charge this enumeration; the repair removes its repeated
work without changing charged limits or cancellation observations. It does not claim to linearize
upstream compilation or downstream rendering.

## Correctness and artifact control

The clean baseline and patched candidate were built sequentially in
`/tmp/merman-evidence-index-973e7c893` with shared caches, two Cargo jobs and two Binaryen jobs.
The base checkout was clean before and after the baseline. Workspace sources were refreshed
before its build. Candidate code was applied only after all baseline stages terminated.
The same feature/profile recipe and checkout path were used for comparison.

- Baseline family/diagram-theme Release tests: 547 passed.
- Candidate tests: 550 passed, including three new requirement-list boundary tests.
- Both final Typst WASM files passed all 15 revision-86 support vectors, two materializations,
  three diagnostic/resource-error vectors and the ten-preset catalog.
- Both real Typst packages passed 22 compilations and nine expected failures under CI-pinned
  Typst 0.15.0. Package profile is `publish`, plugin ABI 4, version `0.8.0-alpha.6` and Typst
  package version `0.3.0`.
- Complete private Release owner suite: 3,273 passed, two existing tests skipped, both on the
  isolated candidate and again at the committed clean source. This includes
  renderer, facade, bindings, acceptance, all 706 native route-profile witnesses, support discovery,
  authoring and Block/Class retirement tests. SVG structure comparison and formatting passed on
  both candidates. Production-feature Clippy passed before the implementation commit; pre-existing
  warnings remain, with no reported warning in the new code.

The new tests cover duplicate native facets, a legacy-only rule, mixed native/legacy sibling
facets, first-occurrence order, empty input, the 512-rule case, non-lexical effect-binding order,
and shared effect IDs on distinct targets. The three existing family assertions now read the
final evidence ledger instead of an intermediate deduplicated vector. Independent Standards and
Spec reviews found no remaining issue after adding the legacy-route scan term to the bound.

Size was a non-regression control for the structural repair. The preregistered cap allowed each
metric at most 0.1% and 4,096 bytes growth; all four decreased. These small deltas do not close
any of the existing four Typst release budgets.

| Metric | Base bytes | Candidate bytes | Delta | Change |
| --- | ---: | ---: | ---: | ---: |
| Raw linked WASM | 18,681,418 | 18,680,425 | -993 | -0.0053% |
| Optimized/stripped WASM | 11,564,084 | 11,563,769 | -315 | -0.0027% |
| gzip | 4,434,079 | 4,433,871 | -208 | -0.0047% |
| Brotli | 3,276,329 | 3,275,835 | -494 | -0.0151% |

Final packaged payload SHA-256:

- Base: `7f967690eda6741f5923b4068e8dc4503512165aa2fe421e0a29f38efca33921`.
- Candidate: `0c06a209c4a946306d71c25a50c2f241f39515ba068bc1f443afc6e89a2dd010`.

The canonical production optimizer produced the package and was rerun independently by the size
matrix over the same raw build. Compression is measured on the optimized/stripped bytes. This
records one frozen base/candidate build per variant; it is not a latency, RSS or peak-memory claim.
The earlier rejected five-sort and `-Oz` experiments remain rejected and were not incorporated.

## Reproduction and limits

```console
CARGO_BUILD_JOBS=2 cargo nextest run --locked --release -p merman-render --features layout-elk,math --lib -E 'test(family::) | test(diagram_theme::)'
CARGO_BUILD_JOBS=2 BINARYEN_CORES=2 cargo run --locked --release -p xtask -- typst-package-smoke --profile publish --out /absolute/package/path --keep-artifacts --typst /absolute/path/to/typst-0.15.0
CARGO_BUILD_JOBS=2 BINARYEN_CORES=2 cargo run --locked --release -p xtask -- wasm-size-matrix --surface typst --budget-file /absolute/path/to/docs/release/WASM_SIZE_BUDGETS.json
```

Use an xtask compiled from the selected checkout. Logs are
`/tmp/theme-evidence-index-{baseline,candidate}-{tests,package,size}.log` and
`/tmp/theme-evidence-index-candidate-{owner,structure,clippy,fmt}.log`. The ignored ledger retains
commands, patch, checksums, the bound derivation, artifact manifests and package copies.

## Clean-source confirmation

A new detached checkout at `d92dee3ade9e83fdb519e8d1e908ca2da27d041a` was clean before and
after the serial owner suite, SVG structure comparison, real Typst package smoke, `typst-wasm`
runtime dependency-closure check (115 packages), and formatting. Compiler logs identify
`/private/tmp/merman-evidence-d92dee3ad`; 1,550 workspace Rust sources and Cargo manifests had
their timestamps refreshed before rebuilding against the shared cache. The owner suite passed
3,273/3,273 with two existing skipped tests. The final optimized WASM passed the same 20 shared
vectors and preset catalog; the real CLI passed 22 compilations and nine expected failures.

The clean packaged WASM has 11,563,736 bytes and SHA-256
`36bfef2b047ff30dd2cd1941f6fcfa1d032dd14fda53c65a968a22ac645cff90`.
Its source-input fingerprint is `aa83d6a79f44834dace5dcecd03f2f4d56dd4837d211106422e3df9d3c1fca1f`
and its tool fingerprint is `88df78cfc6abcfb1f4c895daf872c437f467b1a0caeddab6fde6bdba6a96c8dc`.
The input and tool records exactly match the patched candidate's manifest; the payload differs
by 33 bytes. The cause has not been isolated, so this record claims verified behavior from clean
source, not byte-identical artifacts across checkout paths. The A/B table above remains tied to
its one shared comparison path; clean compressed sizes were not remeasured.

Clean logs use `/tmp/theme-evidence-index-clean-{owner,structure,package,closure,fmt}.log`.
The same explicit owner command used for both full regressions is:

```console
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-render -p merman-theme-acceptance -p merman-bindings-core -p merman --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg --lib --test class_svg_test --test theme_support_discovery_test --test theme_authoring --test route_cutover_runtime --test legacy_projection_retirement --test block_title_legacy_projection --test class_edge_label_background_legacy_projection --no-fail-fast
```

This is scoped renderer/transport evidence. Full workspace, complete browser, all-host builds and
the `xtask verify --strict` release umbrella were not run for this slice.

This slice does not qualify public preset cells, change support revision 86 or retire bridge
routes. C7a artifact size, host/profile qualification and formal rollout/freeze remain open;
C7b retains Class Text's four and Block's 32 legacy routes.
