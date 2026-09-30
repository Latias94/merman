---
type: "Work Progress"
title: "Mermaid 12 reference projection admission"
description: "Target runtime projections and remaining corpus admission work."
timestamp: 2026-09-21T01:39:36Z
record_id: "0e9509533c9245849d1b61bc38b8a674"
producer_id: "codex-mermaid12"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "edeb4d374"
---

# Status

The implementation goal remains active. Target release is `v0.8.0-alpha.7`, including
unreleased main changes and the Mermaid 12 alignment. Package versions remain alpha.6.
No push, PR, merge, or publication has occurred. The working bundle selects Mermaid 12,
but accepted SVG manifests and residual contracts still need genuine target refresh;
this is not a completed reference admission.

# Changes and evidence

- Reference package graph: Mermaid 12.0.0, parser 2.0.0, CLI 11.17.0, Tidy Tree 1.0.1,
  ZenUML plugin 1.0.1, ZenUML core 3.50.1, DOMPurify 3.4.15, Puppeteer 25.6.0.
  Tidy and ZenUML plugin source tags resolve to `a86a2bf4d8fd2a9045f564b5b37c4c70cde18ca6`,
  distinct from Mermaid's `98a0945418c76238f15df2afaddbba4272656c3b`.
- Official npm signature output is retained under the ignored admission directory and bound
  by the selection receipt. Source registry digests use Git's LF representation, with only
  CRLF normalized when checking a working checkout.
- The existing runtime generators produced new config values/shape, theme snapshots/oracle,
  and DOMPurify allowlists. The latter adds `pointer-events` and `vector-effect`.
- Removed temporary Agentflow/Swimlane configuration bridges. Formal target defaults now
  provide family appearance and global ELK. Mindmap's authored-layout-aware Cose default remains.
- Theme calculation now follows target container stroke ordering, base node-border gradient
  behavior, extended light/dark backgrounds, and Redux categorical palette dependencies.
  Initial theme/appearance nextest: 42 passed. Independent review found two falsy-value
  dependency-order defects; both were fixed and the expanded theme suite passes 40/40.
  The full core run initially had 1626 passed and 9 failed; the eight default/catalog failures
  are now resolved by source-backed assertions and Agentflow/Usecase characterization rows.
  The affected detection/metadata/catalog suite passes 142/142. Semantic golden refresh remains.
- Playground now loads built-in ELK from Mermaid core; only Tidy and ZenUML remain external.
  Typecheck, 7 requirement/registrar tests, 59 realm tests, and the generated npm license check pass.
  Its isolated engine build was 10,689,242 bytes before the sanitizer patch and met the existing budget.
- `edeb4d374` fixes the reference renderer: do not register legacy ELK over the built-in
  shape registry, and restrict deferred Sequence actor-math XML merging to Sequence SVGs.
  Target math generation passes for both Flowchart and Sequence. Mermaid 12 still starts
  asynchronous actor math; a version-11-only workaround would lose actor labels.
- Binding regression under target runtime defaults: 198/198 passed across bindings-core,
  C FFI, UniFFI, and WASM. This is native bridge testing; the actual rebuilt browser WASM
  and Playground integration remain final admission work.
- `93138643a` moves both upstream-render audit paths onto the shared standard runtime,
  preserving timeouts and error-SVG rejection. Eight targeted audit/source-receipt/renderer
  tests pass. The unused CLI Puppeteer-config helper and its implementation-only test were removed.
- Reference nextest: 21/22 passed. The remaining standing-verification failure is expected
  until target SVG manifests replace the old accepted reference source identities.

# Required continuation

1. Continue the existing live serial SVG generation in `target/mermaid12-upstream-svgs-all-3`;
   inspect its live process before considering any restart. It uses the selected installed
   runtime and writes genuine target SVGs plus provenance. Do not rerender completed work
   merely because a tool observation times out.
2. Refresh semantic/layout goldens with source-backed deltas. Protect the unrelated
   `src/tests/flowchart.rs` formatting edit.
3. Rebuild xtask after the active executable finishes, then generate the Playground example
   catalog from its target manifest. A normal rebuild was attempted and Windows rejected
   replacement of the live `target/debug/xtask.exe` (access denied); wait for the live renderer
   rather than moving/deleting its binary. Do not accept a catalog embedding old defaults.
4. Review target SVG changes and promote bytes and manifests together. Reassess the 5 root
   contracts, 96 browser-text residual fixtures, 30 label entries plus 7 label identity
   contracts, and the XYChart viewport receipt against target outputs. Never bulk-relabel
   hashes or relax the comparator to accept an algorithm/theme/structure mismatch.
5. Complete materialized/trusted-base reference checks and affected bindings, built WASM,
   Playground browser, feature/license/size integration. Update release projections and
   changelog to alpha.7 only at the final coherent admission boundary.
