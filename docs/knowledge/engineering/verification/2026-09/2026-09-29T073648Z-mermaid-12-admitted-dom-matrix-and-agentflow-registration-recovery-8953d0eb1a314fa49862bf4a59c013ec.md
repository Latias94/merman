---
type: "Verification Evidence"
title: "Mermaid 12 admitted DOM matrix and Agentflow registration recovery"
description: "Distinguish the passing admitted DOM gate from obsolete diagnostic reports; preserve parser-owned Agentflow DOM ordinals and independent review fixes."
timestamp: 2026-09-29T07:36:48Z
record_id: "8953d0eb1a314fa49862bf4a59c013ec"
status: "verified"
source_session: "01a0eb5f-6fe3-7a11-b09a-e34b891d4a65"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "bcc68519fe6b1e9bf2acaef4bee07f80c2b46440"
---

# Verification

Recovered session `01a0eb11-13df-79a1-adcb-8c1d8930b58f` into the current thread. The
working tree began at `30af46b6c48ebcd28a57c5c9c8267eae4b0ed8fd` with pending Agentflow/Usecase renderer
and comparator-registration changes. The goal still covers the entire U1–U15 plan.

# Result

The admitted three-mode comparison passes all 37 families. Earlier failure reports
named `target/compare/*_report.md` came from a diagnostic run without the admitted
browser-text-layout policy. They do not establish release-gate blockers. The actual
invocation below writes `*_report_parity_root.md` and accepts existing exact residual
receipts. No receipt, comparator tolerance, upstream SVG, or size budget was changed.

Agentflow DOM ordinals now come from parser registration, including repeated and
standalone vertices, chained/grouped endpoints, delayed Jison metadata reductions,
style declarations, and replacement of ordinary vertices by connectors. Rendering
facts survive serialization while remaining outside the compatibility projection.
The previous uncommitted edge-list reconstruction was removed.

Independent specification review found one P2: expanded Agentflow hand-drawn
containers bypassed family-specific paint. It is fixed and independently rechecked.
They now retain transparent fill, 10px corners, the configured flow-container stroke,
and 0.75px borders. Standards review found two P3 maintainability issues; direct
actor emission and an explicit label-kind enum resolve both. Both reviewers reported
no remaining findings in this slice after rereading the fixes.

# Evidence

- Full admitted gate, exit 0:
  `cargo run -p xtask -j 1 -- compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout`.
  Log: `target/mermaid12-recovery-full-matrix.log`. This ran before the subsequent
  parser-counter/review fixes; affected families are rechecked separately below.
  Exact residual counts include Flowchart 145, State 22, and Sequence 84.
- Equal-browser-measurement ELK replay: `flowchart_elk_browser_measurements_test`
  passed its 45-graph provider-layout comparison. Do not hide f64/getBBox endpoint
  residuals by increasing near-duplicate-point tolerances.
- Five parser-counter cases independently replayed against the installed pinned
  Mermaid 12 package in local Edge. Results: `target/agentflow-dom-counter-replay.json`.
  Regression expectations are retained in the core test, including serialization.
- Final affected unit/integration run, exit 0: 75 tests passed using
  `cargo nextest run -p merman-core -p merman-render --features layout-elk --lib --test agentflow_svg_test --test usecase_svg_test -j 1 --no-fail-fast -E 'test(flowchart_browser_measured_terminals_preserve_upstream_geometry) or test(agentflow) or test(usecase)'`.
  Log: `target/mermaid12-family-review-fixes-tests.log`.
- After all review fixes, Agentflow, Usecase and Flowchart passed the same admitted
  three-mode invocation with explicit `--diagram` selection, exit 0. Log:
  `target/mermaid12-family-review-fixes-dom-matrix.log`.
- Clippy passed with `-D warnings` for Core/Render libraries and tests with `layout-elk`,
  and separately for the xtask binary, all using `-j 1`. Logs:
  `target/mermaid12-family-clippy.log` and `target/mermaid12-family-xtask-clippy.log`.
  `cargo fmt --all --check` and `git diff --check` also passed.
- Code is committed as `bcc68519fe6b1e9bf2acaef4bee07f80c2b46440` (`fix(svg): converge Agentflow and Usecase DOM with Mermaid 12`).
- A broad initial nextest invocation filtered names without selecting test binaries;
  it linked unrelated examples/tests and hit Windows linker LNK1102. It was stopped.
  Scoped single-job commands above completed; no target cleanup was necessary.

# Follow-up

The entire alignment goal is still active. Final distribution artifact rebuilding,
consumer verification, and size-budget admission remain. The last recorded slim
artifact measurements exceeded analysis gzip/Brotli by 37,226/37,900 bytes and editor
by 7,351/19,502 bytes; those are historical measurements, not fresh results from this
session. Keep capabilities and budgets unchanged while investigating. Typst's
release recipe requires Binaryen 131. Preserve the pre-existing untracked
`fixtures/upstream-svgs/.xtask-upstream-svg-staging/` directory.

# Citations

- [Alignment plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Agentflow parser and counter regression](../../../../../crates/merman-core/src/diagrams/agentflow.rs)
- [Agentflow SVG regressions](../../../../../crates/merman-render/tests/agentflow_svg_test.rs)
- [Diagram verification facts](../../../../../crates/xtask/src/cmd/compare/diagrams.rs)
