---
type: "Verification Evidence"
title: "Mermaid 12.2 coverage verification"
description: "Source-backed fixture follow-up, independent review fixes and issue 197-200 triage."
timestamp: 2026-10-13T14:56:40Z
record_id: "116f37a2c25a483eaa6abae7553094d1"
resource: "docs/alignment/FIXTURE_COVERAGE_12_2.md"
producer_id: "codex-mermaid-12-2-root"
run_id: "mermaid-12-2-20261013"
related_issue: "https://github.com/Latias94/merman/pull/196"
git_branch: "feat/mermaid-12-2-alignment"
---

# Verification

Coverage follow-up on `feat/mermaid-12-2-alignment`, based on `a605b5d72c44d905945eba5684223af3d7c598ec`, for PR #196. Selected Mermaid source remains `1fb7869866dd619168d840291c9c54b4f129758b`; trusted transition base remains `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`.

# Result

Six additional primary inputs cover ABNF/EBNF/PEG grammar combinations, nested Agentflow collapse/self-loops, a compound Usecase ELK guard rejection, and distinct nested/explicit/synthetic Swimlane lanes with strict security. Each has a semantic golden, layout golden and actual selected-runtime SVG. The primary matrix contains 3,761 inputs, 3,758 layout goldens and 3,737 SVGs; 24 explicit exclusions are unchanged.

The inputs and independent review exposed production omissions. Agentflow retains unmerged Dagre segments and empty-label DOM order; parallel loops overwrite metadata at their first Graphlib slots. Usecase retains text metrics separately from padded SVG edge boxes used by layout and guards, preserves ordinary/custom shape label positioning and signed bbox origins, and selects static/animated Neo markers according to metadata and compiled class animation. Swimlane synthetic colors follow declaration slots, and unchanged pinned routes retain layout label anchors.

# Evidence

- Workspace nextest with all features: 9,716 passed, eighteen existing feature/diagnostic skips (`target/mermaid-12.2/coverage-workspace-nextest.log`).
- After the last production review fixes: 1,813 affected render library, Flowchart/Agentflow/Usecase/Swimlane/Railroad/ELK and layout-snapshot tests pass (`coverage-final-render-nextest.log`). The final Clippy cleanup of a test signature also passes its focused actual-reference regression.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` pass.
- Release-mode `compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root` passes all 37 families from frozen production source (`coverage-final-full-dom.log`). Comparator normalization, tolerances and residual catalogs are unchanged.
- Materialized and trusted-base Mermaid reference verification and generated projections pass again. All 37 CI-plan tests pass.
- All five Web/WASM packages are rebuilt from final production source; Web tests, package integrity, the 37-family consumer smoke and DOM-safety smoke pass (`coverage-final-web.log`). Two Windows symlink tests retain their existing platform skips.
- Actual instrumented Usecase ELK capture is byte-identical to its reference SVG and confirms padded provider label dimensions; its raw capture remains under `target/mermaid-12.2/usecase-boundary-guard-probe/`.
- The complete 3,737-input Chromium root audit verifies no geometry/structural violations or overflow pixels for `swimlane/upstream_7_car_sales_constr` and `swimlane/upstream_docs_basic`. Windows still has the existing filtered-title platform fingerprint mismatch, with one blocking diagnostic and one unused exact receipt; it is not reported as a green full painted-viewport gate (`coverage-final-root-viewport-diagnostic.json`). Linux CI owns the exact replay on the pushed revision.
- Two generated family-local comparison SVGs were archived from `target/compare` into `target/mermaid-12.2/family-local-compare` before the final primary audit. Their committed private source/reference evidence remains intact.

# Follow-up

Issues #197–#199 remain presentation-theme defects: C4's text/relationship defaults, ZenUML's fixed stylesheet/fragment paint, and Wardley/Gantt/XYChart plus the shared Mindmap label-background fallback. Issue #200 is reproduced with the current CLI: the shared export fallback drops full transform order and rotation when hoisting foreignObject text; PNG/JPEG/PDF use that same path. These product issues remain open and are separate from the release alignment fix. No issue comments or closure actions were published.

Canonical PR monitoring retains invocation `ea58ea61fd6b41f1abee35b438961179`; the root owns final commit/push and the recovery watcher owns subsequent checks. Merge remains a maintainer action.

# Citations

- [Fixture coverage and issue triage](../../../../alignment/FIXTURE_COVERAGE_12_2.md)
- [ADR-0093](../../../../adr/0093-mermaid-12-2-upstream-transition.md)
- [PR #196](https://github.com/Latias94/merman/pull/196)
