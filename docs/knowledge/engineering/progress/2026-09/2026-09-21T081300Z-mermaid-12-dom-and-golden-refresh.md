---
type: Progress
status: active
date: 2026-09-21
---

# Mermaid 12 DOM and golden refresh checkpoint

The full U1–U15 goal remains active on `refactor/mermaid-12-alignment`; final release target
is v0.8.0-alpha.7. Package versions remain alpha.6 until final admission. Preserve unrelated
working-tree changes. No publication or merge is authorized.

## Completed evidence

- Root CSS font emission now shares one writer across the standalone family renderers.
  Explicit empty/whitespace root font omits the root rule, independently of family theme fonts.
  Fourteen focused CSS tests passed; independent source review confirmed the empty sentinel.
- Full-feature renderer library run: 1342/1347 passed. Four math-capability assertions and one
  ELK explicit minNodeWidth precision case were corrected; all five targeted reruns passed.
- Refreshed semantic goldens through xtask: 277 State defaults/model changes and one Flowchart
  diagram-type change. Restored only the generator's JSON-equivalent serialization churn after
  comparing parsed JSON with Git objects. Core semantic snapshot tests passed 2/2.
- Refreshed 2073 existing layout goldens. One documentation Flowchart exceeded interactive ELK
  work budget; its corrected kernel now passes the actual fixture without increasing limits.
  The missing layout golden still needs regeneration through the updated xtask.
- Class/State DOM follows built-in ELK label-wrapper presence, plural edge group classes and Neo
  margin markers. Class dividers follow Neo/useGradient. State end-state paints use source theme
  fields instead of obsolete color exceptions. Explicit Dagre empty wrappers remain supported.
- Flowchart colored marker definitions share the base marker geometry and cover all three kinds,
  both endpoints, Neo margin and animated exceptions. Process rectangles defer Neo rounding to CSS.
  Ordinary container color slots follow source containment preorder; Agentflow keeps its own offset.
- Targeted renderer run: 113/116 initially passed. Fixed two stale State assertions and a real
  Agentflow collapsed/cyclic containment projection bug; all three reruns plus the budget fixture
  passed (4/4). Agentflow core and position tests passed 40/40.
- Five ELK budget/order kernel tests passed. SortByInputModel charges actual chain/comparison work;
  flat Barycenter charges executed attempts/counts/sweeps. Identity port reorder avoids unnecessary
  full-graph reference writes. Independent review found a remaining quadratic final saved-port
  lookup. Replaced it with linear ID buckets preserving duplicate/mixed fallback behavior;
  independent review passed, and four focused tests including the 1024-parallel-edge boundary and
  real documentation fixture passed (`target/mermaid12-elk-port-restore-tests.log`).
- Third-party authority now records Mermaid 12.0.0 and DOMPurify 3.4.15 at the selected commits.
  License bytes were checked against exact Git objects (checkout CRLF is not source-byte evidence).
  Existing legal generators refreshed notices; third-party verifier and 382 legal projections pass.

## Genuine upstream SVG regeneration

The prior all-3 staging set has 37 complete families / 3716 SVGs (including eight explicit
source-negative Packet/Radar/Treemap error diagrams). Their transport no longer loses cyclic
Langium errors: the generator serializes a plain Error, and only the exact eight known negative
fixture identities may admit an actual rendered error SVG.

Review then found two reference-runner bugs: an inner CSS background rule suppressed root
background injection, and pre-parsing before render advanced Class's global node counter twice.
Both are fixed in generate.rs. A genuine fresh run is active at
`target/mermaid12-upstream-svgs-all-4`, log `target/mermaid12-upstream-svgs-all-4.log`, session 33113,
owned by promotion_readiness. Agentflow/Usecase/ER completed; Flowchart is still running at this
checkpoint. Keep the live process; do not rebuild target/debug/xtask.exe while Windows executes it.
The exact browser is Edge 153.0.4234.48, font probe df16e5d02be72139eddf67ff954b0b112aeabe403e927a42717d8b9efec3d800.
Do not label old bytes as corrected output or promote incomplete family manifests.

## Admission and immediate continuation

- Removed Flowchart comparison's historical explicit-ELK allowlist gate, which silently skipped
  ordinary Mermaid 12 default-ELK fixtures. Removed the obsolete force-ELK CLI option. Historical
  collection/provenance membership remains intact; strict comparator and parser-only policy remain.
  Import entry points now admit explicit ELK and Neo under their existing family constraints.
  Ten focused comparison/import policy tests passed across the initial run (9/10) and a
  one-test rerun after correcting the new test's temporary-corpus filter. Semantic-label checking
  was not disabled; the first attempt correctly rejected treating a one-fixture temporary directory
  as a complete family corpus.
- Flowchart container CSS now preserves separate source rect/path rules and sits after base CSS,
  before family rules; Agentflow retains its separate palette. All five focused palette/marker DOM tests now pass
  (`target/mermaid12-flowchart-dom-final-tests.log`).
- Final WASM/Playground production rebuild is still required after Rust renderer changes.
  The earlier successful build/browser evidence is recorded in the preceding checkpoint.
- Complete all-4 SVG family review/promotion, layout golden regeneration, strict comparison and
  residual admission, remaining feature/size/integration checks, independent review and focused
  commits. Do not declare the complete goal achieved from these scoped passes.

## Logs

- `target/mermaid12-render-lib-tests.log`
- `target/mermaid12-update-snapshots-current.log`
- `target/mermaid12-semantic-delta-review.json`
- `target/mermaid12-update-layout-snapshots.log`
- `target/mermaid12-layout-snapshot-tests.log`
- `target/mermaid12-dom-budget-tests.log`
- `target/mermaid12-dom-budget-rerun.log`
- `target/mermaid12-container-core-tests.log`
- `target/mermaid12-elk-budget-kernel-tests.log`

## Resource incident and continuation

The combined xtask/renderer policy test command selected every renderer integration binary, and
Cargo's default parallel links exhausted memory. No policy test result was produced by that attempt.
It also caused one Edge launch failure in the still-live all-4 generation. Root's build has exited;
subsequent Cargo commands must use `CARGO_BUILD_JOBS=1` and explicit `--bin`/`--test`/`--lib` targets.
The bounded retry completed, log `target/mermaid12-admission-policy-retry.log`, selecting only
xtask's binary tests; its final corrected test passed in `target/mermaid12-admission-policy-final.log`. Do not overlap Cargo jobs. The generator owner has recovered and continues
session 33113; preserve complete families and inspect final failure status before retrying only the
failed family/fixtures through the existing generator. Do not manufacture a complete manifest.

Rust dependency license check also passed: 13 generated reports / 3626891 bytes, log
`target/mermaid12-rust-license-check.log`. No license report regeneration was required.

Representative artifact dependency closure verification passed all 34 profiles; log
`target/mermaid12-artifact-closures.log`. This checks dependency closure, not cross-target compilation.

## 08:40 UTC integration continuation

- Verified previous turn made concrete implementation and test progress; goal remains active.
- Full renderer library integration exercised 1350 tests: 1348 passed, two old assertions still
  expected `edges edgePath`. Updated them to the source-backed plural ELK class; targeted rerun
  passed 2/2 (`target/mermaid12-render-lib-final.log`). All 1350 have passing evidence across
  the full run and focused correction; no repeat full run was needed.
- Capability descriptor digest drift traced to earlier commit f02509691: the ELK/Cytoscape
  absence-contract text changed without regeneration. Independent canonical serialization proved
  old e3dfc678... vs current f36a4518... digests. Regenerated seven projections with the existing
  generator, then updated artifact-profiles capability_authority reference to the actual digest.
  No hashing/validation algorithm was changed. Rustdoc generated receipt remains stale and must
  be rebuilt through the CLI before completion. Authority input changes may require final legal
  reports/source-digest projection refresh; earlier reports predate this authority correction.
- `verify-feature-matrix` now passes all eleven curated builds plus 34 profile/feature structure
  validation (`target/mermaid12-feature-matrix.log`). Cargo build jobs were limited to one.
- all-4 Flowchart transaction failed due to the earlier Edge OOM and correctly rolled back the
  whole family; no Flowchart baseline was promoted. The original session33113 continues State
  and subsequent families. A directed retry (session7810) was rejected by the existing global
  toolchain lock and is terminal; do not retry concurrently. Promotion owner will retry only
  Flowchart after the original generator finishes. Three completed families remain valid.
