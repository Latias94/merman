---
type: "Work Progress"
title: "Mermaid 12 integration closeout and remaining source convergence"
description: "Verified integration surfaces, source-backed route repairs, and open size/parity gates."
timestamp: 2026-09-24T03:52:00Z
record_id: "mermaid12-integration-closeout-20260924"
producer_id: "codex-mermaid12-recovery"
run_id: "session-01a0c895-recovery"
---

# Current scope

Continue the Mermaid 12.0.0 refactor on `refactor/mermaid-12-alignment`. The genuine
12.0.0 reference corpus has been admitted in `9e74c56a7` (37 families, 3,716 SVGs).
This is corpus admission, not a claim that every local renderer passes parity. No push,
PR, publication, or release was performed.

# Verified consumer surfaces

- Five Web WASM packages built successfully with their declared capability recipes.
  The measured source predates the final classic-circle padding correction. Input identities:
  full `279ca08fe5f8`, analysis `5279c6978fb5`, render `76dcb039a5af`,
  editor `7740b674c3c5`, ASCII `03e7c91e8cb4`.
- Web contracts verified 35 WASM exports, 45 runtime bindings, and five package entries.
  Node scripts passed 126 tests, with two skips.
- Playground production build and eight desktop browser smoke cases passed using the
  above full WASM. Prepared tests and ESLint passed. The capability test now covers all
  33 diagram types and explicitly checks Agentflow/Usecase as unsupported by ASCII
  (`a69c1e7ca`). Fresh renderer changes still require a new full WASM build.
- VS Code extension build and 255 tests passed.

# Rust integration status

The preceding serial workspace nextest run executed 8,607 tests: 8,605 passed,
two failed, and seven were skipped. Both failures were diagnosed:

1. The nested Flowchart ELK layout golden retained `subGraph1` where the current parser
   names the node/cluster `main`; only those two IDs were updated.
2. An xtask operation-evidence test selected seven fixtures through the ambiguous `basic`
   substring. It now selects the exact basic-flowchart example. Full-corpus parity still
   checks the excluded-from-this-test browser-sensitive fixture.

A subsequent targeted layout snapshot run without `layout-elk` was a vacuous pass:
the main snapshot test returns early without that feature. Do not use it as golden
verification. The ongoing strict workspace run includes ELK through feature unification.

The September 24 `cargo run -p xtask -- verify --strict` run is recorded in
`target/mermaid12-verify-strict-final.log`. Before nextest it passed fmt, workspace
check/Clippy, 117 feature combinations, artifact recipes, materialized reference,
generated contracts, alignment, RustSec exceptions, dependency closures, 13 Rust
license reports, 382 legal projections, legal contents of 23 Cargo packages, and
Playground/VS Code npm licenses. Nextest stopped after 7,516 of 8,612 tests:
7,515 passed, one failed, seven skipped, and 1,096 were not run. The failure is the
classic-circle assertion in `flowchart_node_shape_dimensions_follow_mermaid_rules`,
which still expected label width plus padding rather than the source label diagonal.
That assertion is corrected in `8931f9988`. New small-node alignment unit tests
passed in this run. Some renderer edits occurred after initial library compilation;
current-source affected checks remain necessary. Doctests and downstream SVG/package
stages were not reached.

# Source-backed geometry findings

- Circle dimensions follow Mermaid 12 `circle.ts`: label diagonal plus 64 for Neo,
  label diagonal plus the complete configured padding for classic. The initial
  classic implementation doubled that padding; the working correction also covers
  an empty label with custom padding. Commit `a9f3cd939` contains the preceding
  circle repair, evidence-test selection, and nested-group IDs.
- State parallel-edge label receipts were reviewed against the same input/upstream
  hashes. The renderer's source-backed Neo barb offset changes local terminal y from
  204 to 198.5; only the three exact local route/mask signatures are updated.
  Label widths, anchors, provider routes, and upstream signatures remain unchanged.
  The catalog is embedded with `include_str!`, so an old xtask executable cannot
  validate the edited file.
- State `stress_state_quoted_multiline_names_015` has a reproduced browser floating
  boundary residual. Upstream x delta is `-2.842170943040401e-14` relative to the half
  width; local delta is positive. `outsideNode` therefore selects five versus four
  points. The upstream extra y is `183.38425601494978`. The replay extracts the actual
  pinned release functions; no renderer epsilon or comparator relaxation was added.
  Reproduction: `node target/mermaid12-state-015-replay.mjs`; output is the adjacent
  JSON. Source: ELK `geometry.ts` lines 98/224/276 and `render.ts` lines 2452/2475/2487.
- Flowchart small-node anchor alignment and shared endpoint-cutter migration are
  implemented in `8931f9988`, including the classic circle padding correction.
  Shape-specific intersections remain an adapter to the shared cutter. Existing
  cancellation and work-budget accounting cover the new alignment pass.
- Class now preserves authored internal member whitespace (`496aa53a6`). Sequence
  final drawn text inherits the theme family when the inline family is absent or
  rejected (`203c757c5`); body-level measurement retains its original context.
- `fb3d5707c` records the three exact State Neo marker receipts and the self-loop
  golden omitted by the earlier NORTH-port fix. No broad normalization was added.

# Current-source verification at 04:36 UTC

The affected renderer nextest run executed 1,654 tests: 1,650 passed and four failed.
Two failures were test-only CSS whitespace expectations, one assumed a symmetric
compact cutter that the upstream algorithm does not provide, and one was the golden
aggregate. The corrected contracts pass all ten focused unit tests. Exactly 129
layout goldens were regenerated after cause review: 23 Class whitespace, 105
Flowchart circle/small-node/previously fixed parser identity, and one prior State
self-loop repair. All three layout snapshot tests now pass with `layout-elk` enabled
and zero skips. Formatting and render/core all-target/all-feature Clippy pass.

Fresh three-mode SVG comparisons use the rebuilt xtask executable:

| Family | Rendered | Comparisons | Current result |
| --- | ---: | ---: | --- |
| Class | 250 | 750 | pass, including existing exact accepted receipts |
| Flowchart | 1,153 | 3,459 | 68 distinct DOM blockers; 34 stale local receipt fixtures |
| State | 285 | 855 | 18 distinct DOM blockers; three label receipts accepted |
| Sequence | 321 | 963 | 25 distinct DOM blockers, plus stale receipts |
| Timeline | 93 | 279 | one new browser wrapping residual; 26 existing receipts accepted |

These counts distinguish the report's Mismatches section from accepted Notes. Modes
and stale receipts may overlap; do not sum them as distinct fixtures. Class's existing
026 receipt matches again after authored whitespace is preserved.

Two genuine remaining gaps are being implemented: shared line hops need the pinned
Mermaid 12 rounded-corner exclusion, 2px clearance, and 60% useful-radius filtering;
Sequence drawing must accumulate each actual text line's height and preserve signed
margin/dy/JavaScript rounding behavior. State style-spec HTML height is under audit.

# Timeline exact residual admission

The new Timeline 012 receipt binds input SHA-256
`913f05bbfa2bfba5177aeb5b34b64e40720bcd3e03617d0fd9d19aa9f8ed6bb9`, upstream SVG
`75d2c562ccc20d50e430afb785f0854383b821bfd36cf8b33c0ca433ac842d1a`, and canonical
local signature `eb0aeb1b8bf6885d50ab26aee280864b3e7cd4df4acd91653b783727fec598c1`.
Both SVGs preserve all 34 text elements and word sequences. Seven labels wrap at
different words with the same Fira Sans 17px style and 150px width. Pinned Timeline
`svgDraw.js:456` uses `getComputedTextLength`; local `timeline.rs:145` uses its
headless measurement counterpart. Both use first-row 1em and subsequent-row 1.1em.
The resulting row heights propagate into the viewport. Outside text subtrees,
224 elements retain order, IDs, classes, and path command sequences. Additional
upstream Neo CSS is inactive because this fixture has no `data-look` attributes.
This receipt does not admit missing text, topology, or active-style differences.

The existing comparator captured the canonical signature, then a fresh full
Timeline comparison passed all 93 fixtures / 279 mode comparisons, including 81
exact residual comparisons. Only this one entry was added; the 26 historical
Timeline entries were unchanged. Detailed local evidence is in
`target/mermaid12-timeline-012-residual-audit.md` and the verification log is
`target/mermaid12-timeline-receipt-verified.log`.

# Artifact-size gate

All values below are bytes measured by the checked-in `wasm-size-matrix` command
against stripped production artifacts. Budgets remain unchanged.

| Profile | Raw | Stripped | gzip | Brotli | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| analysis | 3,552,714 | 3,552,491 | 1,415,366 | 1,089,915 | gzip/Brotli exceed 1,375,000/1,050,000 |
| ASCII | 5,114,034 | 5,113,811 | 1,930,013 | 1,470,327 | pass |
| editor | 3,666,234 | 3,666,011 | 1,460,379 | 1,119,715 | gzip/Brotli exceed 1,450,000/1,100,000 |
| full | 12,951,294 | 12,951,071 | 4,861,117 | 3,600,842 | pass |
| render | 10,990,069 | 10,989,846 | 4,189,184 | 3,108,904 | pass |

Typst compilation succeeded but measurement could not find `wasm-opt` on PATH.
Binaryen 131 is available under `target/tools/binaryen-131/binaryen-version_131/bin`;
retry with that directory prepended to the process-local PATH.

Read-only attribution excludes large generated-theme growth: theme JSON standalone
gzip decreased by 9,780 bytes from the 11.17.2 baseline, while defaults and shape
metadata together grew only 208 bytes. Production-only theme provenance snapshots
are a separate redundant-work candidate, not a proven cause of this upgrade's growth.
Its isolated experiment is registered under
`target/bench/experiments/mermaid12-theme-audit-elision/experiment.yaml` and the sibling
`merman-theme-audit` worktree. The candidate was admitted as `6dc4c9f6f` after 41 theme unit tests, 11 public
contract tests, and real analysis/editor WASM smoke checks. Candidate gzip/Brotli
sizes are analysis 1,413,257/1,088,501 and editor 1,458,058/1,119,272. Improvements
are respectively 2,109/1,414 and 2,321/443 bytes, with raw size also decreasing.
This is a bounded intermediate improvement, not artifact-size gate completion. No budget increase or capability removal is authorized.

# Open gates

Complete the confirmed line-hop and Sequence drawing repairs, then execute the
remaining Rust/doctest and SVG gates.
Rebuild xtask before checking embedded residual catalogs. Continue source-backed
Flowchart/State/Sequence convergence; do not accept old reports overwritten by focused
negative-evidence tests. Rebuild production WASM after the final renderer changes,
remeasure Web/Typst artifacts, and rerun relevant consumer/browser checks. Exact browser
residuals require reviewed evidence rather than blanket signature refresh.

# Citations

- [Mermaid 12 plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Default ELK boundary](../../../../adr/0089-mermaid-12-default-elk-products.md)
- [Performance measurement contract](../../../../performance/BENCHMARKING.md)
