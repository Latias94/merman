---
type: Current State
status: active
---

# Current State

- Active Mermaid parity focus: complete the Mermaid 12.0.0 alignment plan on
  `refactor/mermaid-12-alignment`, targeting unreleased `v0.8.0-alpha.7`. The selected
  reference/runtime bundle and admitted corpus use 12.0.0 (37 families, 3,716 SVGs).
  The full alignment goal remains active; artifact-size and final consumer admission
  are not complete.
- Current parity evidence (September 29): all 37 families pass the admitted
  structure/parity/parity-root comparison with `--diagnostic-browser-text-layout`.
  Existing exact residual receipts include Flowchart 145, State 22, and Sequence 84.
  Old `*_report.md` diagnostic failures without this policy are not the current gate;
  use the fresh `*_report_parity_root.md` reports. No tolerances, baselines, or receipts
  were relaxed. Equal-browser-measurement replay also passes all 45 provider graphs.
- Agentflow/Usecase integration (`bcc68519f`): both families are included in the primary matrix.
  Agentflow DOM ordinals are retained at parse time, including connector replacement
  and delayed grouped metadata. Expanded hand-drawn flow containers retain upstream
  paint semantics. Usecase label/glyph ordering and DOM wrappers converge without
  copying emitted glyph buffers. Independent specification and standards reviews
  are closed after fixes; 75 focused tests pass, including both layout backends.
- Earlier integration evidence: September 24 pre-test feature, dependency, generated,
  alignment, and legal gates passed. Workspace nextest had 8,634 passes and one stale
  residual-count assertion; the assertion was fixed and its three catalog tests pass.
  Subsequent layout/provider/paint fixes and snapshot updates are committed. These
  historical runs do not replace final checks on any newly rebuilt distributions.
- Consumer focus: five production Web WASM packages previously built; Web Node tests,
  Playground prepared/lint/build/eight browser smoke cases, and 255 VS Code tests
  passed. Those artifacts predate the latest paint/kernel fixes. Final full/render
  rebuilds and consumer verification remain.
- Artifact-size focus: production theme audit snapshots were removed in `6dc4c9f6f`
  after equivalent-output tests and isolated measurement. The last assembled slim
  measurements still exceeded analysis gzip/Brotli by 37,226/37,900 bytes and editor
  by 7,351/19,502 bytes. Remeasure before treating these historical deltas as current.
  Preserve capabilities and budgets. Typst requires local Binaryen 131 on PATH.
- Stable focus: editor-language integration hardening spans SVG safety, platform binding lifecycle
  contracts, editor snapshot memory use, and release-gate coverage.
- Stable decisions: SVG text returned to browser-like surfaces must be validated before DOM
  insertion, copy, export, or preview replay; platform wrappers must document document-analysis
  facts and reusable-engine callback lifecycle; editor snapshots should share document text rather
  than copy every Markdown fence body.

# Citations

- [PR20 post-review refactor plan](../../plans/2026-07-04-005-refactor-pr20-post-review-refactor-plan.md)
- [LSP capability contract](../../lsp/CAPABILITIES.md)
- [Android JNI binding contract](../../bindings/ANDROID_JNI.md)
- [Flutter/Dart FFI binding contract](../../bindings/FLUTTER_DART_FFI.md)
- [Release package surfaces](../../release/PACKAGE_SURFACES.md)

- [Mermaid 12 reference admission checkpoint](progress/2026-09/2026-09-21T013936Z-mermaid-12-reference-projection-admission-0e9509533c9245849d1b61bc38b8a674.md)

- [Mermaid 12 Playground and binding checkpoint](progress/2026-09/2026-09-21T071341Z-mermaid-12-playground-and-binding-verification.md)

- [Mermaid 12 DOM and golden refresh checkpoint](progress/2026-09/2026-09-21T081300Z-mermaid-12-dom-and-golden-refresh.md)

- [Mermaid 12 family convergence checkpoint](progress/2026-09/2026-09-21T092000Z-mermaid-12-family-convergence.md)

- [Mermaid 12 integration closeout checkpoint](progress/2026-09/2026-09-24T035200Z-mermaid-12-integration-closeout.md)

- [September 29 admitted DOM gate and recovery evidence](verification/2026-09/2026-09-29T073648Z-mermaid-12-admitted-dom-matrix-and-agentflow-registration-recovery-8953d0eb1a314fa49862bf4a59c013ec.md)
