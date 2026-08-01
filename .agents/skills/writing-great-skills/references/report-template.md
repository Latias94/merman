# Release Report Template

Keep the final report compact enough for a release note, but retain a linked or attached machine-readable evidence file for the full ledger.

## Verdict

State the base and target, publication boundary, and one sentence describing the user outcome. Say plainly whether the refactor primarily reduces workflow-specific cost, expands capability coverage, improves reliability, or some combination.

## At-a-glance

| Surface | alpha/base | current/target | User consequence | Evidence |
| --- | ---: | ---: | --- | --- |
| SVG admission records / logical groups | ... | ... | ... | admission inventory |
| Rust lint/analysis | ... | ... | ... | command or fact path |
| Rust complete SVG | ... | ... | ... | command or fact path |
| CLI default | ... | ... | ... | binary/profile |
| Web analysis | ... | ... | ... | WASM profile |
| Web render | ... | ... | ... | WASM profile |
| Web full | ... | ... | ... | capability contract |
| Node WASM/N-API | ... | ... | ... | harness report |

Use one unit per row and mark incomparable contracts. Put percentage formulas in the evidence file, not in prose alone.

## What users can do now

Group outcomes by user value: diagram coverage and parity, workflow-specific package selection, safer host/runtime boundaries, editor/analysis integration, output formats, and operational reliability. Mention concrete families such as Swimlane, Railroad dialects, Wardley, and Cynefin only when the admission evidence supports the claim.

## Feature and dependency choices

| Workflow | Crate/package | Exact features | Dependencies intentionally avoided | Selection rationale |
| --- | --- | --- | --- | --- |
| ... | ... | ... | ... | ... |

Separate direct dependencies, resolved closure counts, and artifact bytes. Explain heavy leaves such as PDF, math, network icons, and parallel Markdown rather than calling them "unnecessary."

## Benchmarks

Report parse, layout, render, cold start, warm p50/p95, RSS, and parity as separate columns. Put Merman, Mermaid.js, and `mermaid-rs-renderer` on the same corpus row only when the contracts match. Keep Node N-API versus Node WASM parse-only and end-to-end SVG rows separate.

## Scenario guide

Include the relevant rows from [scenario-matrix.md](scenario-matrix.md), with exact migration actions and an explicit note for private or inconclusive products.

## Limits and risks

List missing targets, stale locks, unmeasured historical artifacts, browser text-metric residuals, incomplete external reference runs, and any semantic or SVG parity mismatch. A limitation is part of the result, not an appendix to hide.

## Changelog extraction

Finish with three to seven net outcomes suitable for `CHANGELOG.md`. Do not copy benchmark methodology or internal commit names into those bullets. Link the detailed report/evidence artifact and call out breaking migrations once.
