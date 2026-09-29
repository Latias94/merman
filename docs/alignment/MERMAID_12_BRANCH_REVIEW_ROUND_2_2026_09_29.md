# Mermaid 12 branch review, round 2 — 2026-09-29

## Scope

This follow-up reviewed `2d70832e25497aae282de9da78d1d6db12f2b475...405acc0141a9c663193b7c7bbf1f6ddc82de8b31`
with seven reviewers covering new-family parsing, established-family parsing, layout/configuration,
SVG rendering, public consumers, repository standards, and the originating specification.
The specification remains the September 20 Mermaid 12 alignment plan and ADRs 0089–0091.
The previous review is `MERMAID_12_BRANCH_REVIEW_2026_09_29.md` in this directory.

Native counterexamples used a newly built CLI from that head with
`all-diagrams,svg,layout-elk,layout-cytoscape,analysis`. Expectations came from the pinned Mermaid
12 source, generated parsers/databases, and the selected standard Mermaid browser bundle.
Reviewers did not use old WASM artifacts to prove current native behavior. Cargo execution was
centralized and serial; local transcripts are under `target/branch-review-mermaid12-round2/`.

## Standards axis

Confirmed P1 gate failure: the XYChart upstream SVG changed while its root-viewport residual still
bound the preceding hash. The real browser owner rejected it as both a blocking paint comparison
and an unused exact receipt. This violates the CI contract that changed or unused receipts block.
The DOM comparison's success does not replace the browser paint gate.

Revalidation compared the same current local SVG against both old and new upstream SVGs using the
same browser. Both upstream versions had top structural overflow of 1,147 pixels and 225,959
painted pixels; the local version remained at 1,150 pixels and 227,700 painted pixels. Upstream
geometryTop remained -1146.5625, local geometryTop remained -1149.9375. The upstream rectangle
moved horizontally by one pixel; no new overflow edge, depth, or indeterminate state appeared.
The input still contains the same out-of-domain Y-axis extrapolation (`y-axis 45.5 --> 33; bar [1]`).
Only the receipt's upstream SHA-256 changed. The local hash and closed residual reason did not.
The focused browser owner passed after this update.

## Specification axis

Confirmed P2: graph-family layout selection treated the registered CoSE loader like an unknown
name and silently ran Dagre. Selected Mermaid 12 actually executes CoSE and reports that a root
node is required. KTD4 explicitly prohibits using execution failure as fallback permission.
Seven graph families reproduced the discrepancy: Agentflow, Flowchart, Class, State, ER,
Requirement, and Usecase. Selection now retains CoSE, capability admission reports Cytoscape,
and the actual adapter entry preserves its incompatible-model error. Builds without the loader
retain the unregistered-loader fallback. Mindmap's rootful CoSE path is unchanged.

## Confirmed behavioral repairs

| Area | Counterexample | Repair |
| --- | --- | --- |
| Agentflow root direction | A top-level `direction LR` overrode a `TB` header | Apply direction records only to their flow container |
| Agentflow physical-line rules | `direction LR; A --> B` created an extra edge; `accTitle: hello; world` created a node | Preserve whole-line token consumption while ordinary declarations keep semicolon separators |
| Agentflow quoted labels | A literal Windows path `C:\new` became a newline | Preserve literal backslashes in node, edge, and container labels |
| Agentflow metadata | A double-quoted multiline metadata label failed or folded differently | Apply the pinned shapeDataStr newline rule before YAML decoding |
| State composite declarations | `state "direction LR" as Inner` inside a composite lost Inner | Respect struct-mode statement priority and preserve editor selection spans |
| Agentflow measurements | Family minNodeWidth/wrappingWidth lost to Flowchart defaults/overrides | Project family node settings with upstream precedence and falsy wrapping fallback |
| Usecase JSON | 9007199254740993 retained extra precision relative to JavaScript | Normalize finite numbers to binary64 and use existing ryu-js for displayed scalar text |
| Numeric projection | Rounded positive i64 boundary saturated to i64::MAX | Keep the positive upper bound exclusive before integer conversion |
| Usecase math | Formula labels rendered literally while capability planning claimed readiness without a backend | Route admission, measurement, and sanitized output through the shared math backend |
| Flowchart fork/join | RL used a horizontal bar although Mermaid 12 uses the LR vertical orientation | Apply the LR/RL condition to geometry and verify both graph backends |
| Flowchart image labels | Labels measured as wrapped lines were emitted as nowrap HTML and escaped the capture boundary | Use the upstream fixed-width table/break-spaces wrapper when wrapping is required |
| WASM capability test | Expected 33 diagram families although the actual contract has 34 | Update the stale total; the failure was reproduced before the change |
| Lean Windows CLI | SVG/analysis feature build emitted unused import/variable warnings | Match conditional compilation to the actual feature/platform use |

Cross-review caught two implementation issues before handoff: a missing internal math-label-plan
field and disagreement between raw-label capability detection and escaped plain-label math input.
Their resolution is included in validation below.

## Validation

- Core/editor: 1,706 tests passed. A final focused Agentflow run passed all 14 tests after
  freezing its physical-line scan cache.
- Renderer library plus Agentflow, Usecase, and Flowchart SVG integration: 1,554 tests passed.
- Release SVG comparison passed all three DOM modes (`structure`, `parity`, `parity-root`):
  37 diagram types, 3,717 rendered fixtures, and 19 existing harness skips. No DOM receipt changed.
- The full browser root owner inspected 3,717 baseline-rendered SVGs. After the final render,
  per-file hashes showed that only image-label fixture 136 changed. The real browser owner
  revalidated that fixture, title fixture 029, and the XYChart receipt owner; both local and
  upstream hashes matched the earlier audit for the other 3,714 reused results.
- Final browser classifications: 1,321 contained, 46 browser-owned diagnostics, 2,348 upstream
  inherited, one exact residual, and one blocking fixture (029). Image-label fixture 136 is
  contained with no violations or indeterminate result. XYChart uses its exact receipt once;
  no receipt is unused. The root-viewport gate is still failing, as explained below.
- Lean renderer without math/ELK/Cytoscape: four targeted capability/backend tests passed.
- WASM host library: all 39 tests passed after reproducing the stale-count failure.
- Clippy passed with warnings denied for all targets/features of core, renderer, CLI, WASM,
  and xtask; the SVG/analysis-only Windows CLI feature recipe also passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- Local evidence includes `full-svg.log`, `root-viewport-delta-inputs.json`,
  `root-viewport-delta.json`, and `root-viewport-combined-summary.json` under the transcript directory.

## Commit disposition

The independently validated changes were committed locally:

- `22fa99134`: parser token priority, physical-line handling, and finite-number semantics.
- `9ced25e2a`: CLI feature-specific conditional compilation.
- `3745af95e`: WASM registered-family test expectation.

Renderer repairs and the revalidated XYChart receipt remain in the working tree because the
browser root-viewport gate still has the unresolved failure below. Validation above describes
that complete working-tree state, not just the committed subset. Pre-existing local changes and
staging scratch files were left untouched. No changes were pushed.

## Unresolved browser gate: deterministic title measurement

`flowchart/stress_flowchart_title_padding_subgraph_029` remains blocking in the Windows browser
paint owner. This is not being converted into an accepted receipt. The source-backed title bbox
union already exists and receives the same CSS family, size, and weight as the browser:
`"Recursive Variable", arial, sans-serif`, 18px, normal. CDP reported the browser's actual fallback
as Arial/ArialMT. Its title bbox width is 391.21875px; the font-agnostic deterministic provider
estimates 370.62px, so the 376px subgraph width controls the local viewport. The resulting viewport
exposes the subgraph's left 1px stroke and right 5px shadow. These are structural pixels; the
oracle did not misclassify title paint.

A browser-only in-memory counterfactual changed just the viewport's horizontal title bbox to the
measured one, preserving every node, edge, filter, and Y coordinate. Both extra side overflows
vanished, leaving the same bottom overflow as upstream. This isolates the cause; it does not
constitute a production fix. Fixed padding, fixture-specific width overrides, character-coefficient
tuning, or relaxing the paint gate would violate the parity strategy. A robust follow-up can use
the existing host `TitleBBoxX` measurement channel with the actual display environment. The
headless deterministic comparison remains explicitly different from that environment.

Ubuntu CI was not run here. Its font fallback and rasterization may differ, so the exact Windows
pixel result is not asserted for Linux. The gate failure and its effect remain visible rather than
being described as a green complete browser sweep.

## Remaining limits

- Usecase JSON numbers that parse to non-finite JavaScript values, such as `1e309`, still fail the
  finite JSON model's admission. No public numeric-provenance model or alternate JSON parser was
  introduced; finite-number parity is the scope of this repair.
- A pre-existing Sequence note/message font mismatch under special global font configuration was
  identified outside the branch-introduced findings. It is not claimed fixed by this round.
- Previously documented ELK explicit-null and browser-dependent text/font residuals remain.
- Reviews and focused adversarial cases are not a proof that every language/configuration
  combination matches Mermaid. Native, browser, and platform evidence are recorded separately.
