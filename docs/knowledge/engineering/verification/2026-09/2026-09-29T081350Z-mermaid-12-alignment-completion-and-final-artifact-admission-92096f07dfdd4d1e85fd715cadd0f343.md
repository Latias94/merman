---
type: "Verification Evidence"
title: "Mermaid 12 alignment completion and final artifact admission"
description: "Close the U1-U15 alignment goal with six unchanged size budgets, freshly rebuilt consumers, source-backed DOM gates and bounded runtime controls."
timestamp: 2026-09-29T08:13:50Z
record_id: "92096f07dfdd4d1e85fd715cadd0f343"
status: "verified"
source_session: "01a0eb5f-6fe3-7a11-b09a-e34b891d4a65"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "787fdc49e"
---

# Verification

The U1–U15 Mermaid 12 alignment implementation is complete on
`refactor/mermaid-12-alignment`. This checkpoint closes the remaining artifact and
consumer verification after the source-backed DOM repair in `bcc68519f` and its
review/verification record in `787fdc49e`. It is implementation acceptance for the
unreleased `0.8.0-alpha.7`, not a release, publication, or merge.

# Result

All 37 families pass the admitted structure/parity/parity-root gate. All five Web
WASM packages were rebuilt from the final source, assembled, and checked through
production consumers. The Typst artifact was rebuilt and optimized with the required
Binaryen 131. All six artifact profiles pass every existing size limit. No budget,
capability, upstream fixture, comparator tolerance, or residual receipt was changed
in this final admission.

Earlier records reporting open SVG and slim-WASM budget failures are superseded by
this checkpoint and the September 29 DOM verification. The full requirements remain
in scope; acceptance was not redefined around a subset of the plan.

# Evidence

## Final artifact measurements

Windows 11, Rust 1.95.0, wasm-pack 0.15.0, wasm-tools 1.252.0, `wasm-size` profile,
`wasm32-unknown-unknown`, serial Cargo. Web measurements use assembled package bytes,
then `wasm-tools strip --all` before gzip/Brotli. Typst uses Binaryen 131 `-Oz` before
stripping and compression. Values are bytes.

| Profile | Raw | Stripped / optimized | Gzip | Brotli | Existing budget |
| --- | ---: | ---: | ---: | ---: | --- |
| web-analysis | 3,546,891 | 3,546,668 | 1,414,931 | 1,090,886 | Pass |
| web-ascii | 5,107,030 | 5,106,807 | 1,929,033 | 1,469,860 | Pass |
| web-editor | 3,660,418 | 3,660,195 | 1,459,789 | 1,120,979 | Pass |
| web-full | 12,989,718 | 12,989,495 | 4,876,039 | 3,636,635 | Pass |
| web-render | 11,028,622 | 11,028,399 | 4,208,585 | 3,126,243 | Pass |
| typst-wasm | 13,562,767 | 8,582,480 | 3,330,414 | 2,478,338 | Pass |

The tightest remaining margin is Web ASCII Brotli: 5,140 bytes below its 1,475,000-byte
limit. This is a passing measurement, not grounds to increase that limit.

A same-host historical pre-upgrade baseline (`54d257aa8`) remains in
`target/bench/experiments/mermaid12-slim-size-budget/`. Independent source/closure
review found the same 101 external dependencies and versions in analysis/editor,
with no renderer or ELK dependency entering either slim profile. Their source growth
is explained by Agentflow/Usecase parsing, diagnostics, source mappings and shared
configuration/theme behavior. The production theme-audit snapshot removal is already
retained in `6dc4c9f6f`. No new optimization was necessary for admission.

The maintainer's September 25 instruction permits explained reasonable size growth;
its original message was recovered and verified. The current measurements fit the
unchanged budgets, so that permission did not need to be exercised.

Binaryen 131 is installed locally at
`target/tools/binaryen-131/binaryen-version_131/bin/wasm-opt.exe`. The official release
archive SHA-256 was verified before extraction:
`2f4edac1703a2f695254d6ff52ede03481e67db1f094915763d863158c17d9bc`.
Its reported version is `wasm-opt version 131 (version_131)`.

## Production consumers and runtime control

- Five Web packages: source-input manifests, package assembly/provenance, capability
  and output contracts, 37-family runtime smoke, and DOM-safety smoke passed.
- Web Node tests: 126 passed, two deliberately skipped; no failures.
- Playground: fresh production build and distribution verification passed. All eight
  desktop smoke cases passed, including default ELK, automatic versus explicit theme,
  and Agentflow/Usecase through the production WASM. Windows rejected the default
  preview port with EACCES; the existing `PLAYWRIGHT_PORT` override resolved it without
  changing code or reusing another process's server.
- Third-party contract and all 382 release legal projections passed again.
- Final Core/Render focused tests: 75 passed. Clippy for the affected libraries/tests
  and xtask passed with `-D warnings`; formatting and whitespace checks passed.

The existing browser corpus runner exercised ordinary Flowchart and both new families
using six measured repetitions per engine/mode, two warmups, seed `0x5eed1234`, and a
fresh Chromium process per fixture. Every cold and warm run succeeded, with no resource
error. Merman median time to budgeted SVG, in milliseconds:

| Fixture | Cold | Warm |
| --- | ---: | ---: |
| basic-flowchart | 99.450 | 4.500 |
| usecase-basic | 78.400 | 3.700 |
| agentflow-basic | 82.600 | 4.150 |

These timings are a bounded final-version runtime control, not a same-implementation
speedup claim. Cold and warm boundaries differ; they must not be combined. Earlier
kernel/adapter cancellation, exact work-limit interruption and seed controls remain
the evidence for those contracts; successful browser samples do not replace them.

## Requirement closure

| Requirements | Units | Acceptance evidence |
| --- | --- | --- |
| R1 | U1, U15 | Selected Mermaid 12.0.0 behavior graph, exact reference/provenance checks, authentic corpus and generated alignment gates recorded in the integration history; current admitted 37-family matrix. |
| R2 | U2, U9 | Configuration/appearance precedence tests; final browser theme and built-in ELK cases. |
| R3 | U3, U14 | Product feature/default contracts and 117 feature combinations; independent current slim dependency closure and five distinct final package capability contracts. |
| R4, R5 | U4–U8 | Source-backed kernel/adapter, distinct algorithm/strategy, compound routing, ports and seed tests in implementation checkpoints; final equal-browser-measurement 45-graph replay. |
| R6 | U11, U12 | Native family parser/model/diagnostic tests, source registration regressions, both layout backends, primary DOM-matrix admission and final production WASM/browser cases. |
| R7 | U9, U10 | Reviewed family source repairs and layout snapshots; final complete structure/parity/parity-root gate. |
| R8 | U13 | Editor, Tree-sitter, binding/FFI and reusable-engine checks in the editor/binding checkpoints; current-source recovery tests and final Web/Playground artifacts. |
| R9 | U14 | Explicit ELK source/license boundary, generated artifact closures, license reports and final 382 legal projections. |
| R10 | U3–U8, U13 | Deterministic operation/cancellation/work-budget contracts, later targeted resource regressions and successful final bounded runtime controls. |
| R11 | U9–U12, U15 | Exact browser residual receipts, source-backed family admission and final artifact/consumer measurements. |
| R12 | All | Shared family/layout ownership and source-backed replacement of obsolete paths; independent specification/standards review closed after concrete fixes. |

The prior broad workspace, feature, editor/binding, doctest and generated checks are
retained evidence for their unchanged surfaces. Their identified failures were fixed
and rerun in their owning suites. The final pass reran affected checks and rebuilt
changed artifacts; it did not mechanically repeat every unaffected historical matrix.
Independent final plan review identified only the Typst measurement and small runtime
record as outstanding; both are now completed above.

## Reproduction and local receipts

All final artifact logs, input fingerprints, size results and runtime source hashes
are indexed by `target/bench/experiments/mermaid12-final-artifact-admission/experiment.yaml`.
It records all six passing budgets, unchanged budget-file SHA-256, and runtime results.
Key commands, with Cargo build concurrency set to one:

```text
node platforms/web/scripts/build-wasm.mjs --all-packages
npm --prefix platforms/web run build:ts
npm --prefix platforms/web run build:packages
npm --prefix platforms/web test
npm --prefix platforms/web run smoke
cargo run --locked -p xtask -- wasm-size-matrix --surface web --web-package-root platforms/web/packages --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run --locked -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
npm --prefix playground run build
npm --prefix playground/tests run test:desktop -- playground.smoke.spec.ts
node --experimental-strip-types playground/tests/run-benchmark-corpus.mjs --fixtures basic-flowchart,agentflow-basic,usecase-basic --iterations 6 --warmups 2 --port <available-port> --timeout-seconds 360 --out <new-target-bench-output.json>
```

Typst requires the installed Binaryen 131 directory on PATH for its command. Browser
smoke may require `PLAYWRIGHT_PORT` to name an available local port.

# Follow-up

No required implementation work remains for this alignment plan. Publication and merge
remain separate maintainer actions. Preserve historical records and the pre-existing
untracked `fixtures/upstream-svgs/.xtask-upstream-svg-staging/` directory. Browser text,
HTML/font measurement and RoughJS remain explicitly bounded residuals under the existing
source/receipt policy, not a claim of universal pixel identity.

# Citations

- [Alignment plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Final DOM and review evidence](2026-09-29T073648Z-mermaid-12-admitted-dom-matrix-and-agentflow-registration-recovery-8953d0eb1a314fa49862bf4a59c013ec.md)
- [Integration history and owner reruns](../../progress/2026-09/2026-09-24T035200Z-mermaid-12-integration-closeout.md)
- [Editor/dispatch checkpoint](../../progress/2026-09/2026-09-20T211650Z-mermaid-12-editor-syntax-and-layout-dispatch-checkpoint.md)
- [Binding and Playground checkpoint](../../progress/2026-09/2026-09-21T071341Z-mermaid-12-playground-and-binding-verification.md)
- [Size budgets](../../../../release/WASM_SIZE_BUDGETS.json)
