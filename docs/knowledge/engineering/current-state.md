---
type: Current State
status: active
---

# Current State

- Mermaid 12 alignment is complete on `refactor/mermaid-12-alignment` for unreleased
  `v0.8.0-alpha.7`. All U1–U15/R1–R12 acceptance evidence is reconciled in the September
  29 completion record. Publication and merge remain separate maintainer actions.
- All 37 families pass admitted structure/parity/parity-root comparison. Existing
  exact browser residuals remain explicit; no comparator, receipt or baseline was
  loosened. Agentflow/Usecase DOM fixes and independent reviews are closed in
  `bcc68519f`; 75 affected tests, Clippy and formatting pass.
- All five Web WASM packages were rebuilt from the final source. Production package,
  37-family consumer and DOM-safety smoke pass. Playground build/dist and all eight
  browser smoke cases pass. Three representative default-ELK browser workloads pass
  cold/warm controls without resource errors. Prior editor/binding/feature evidence
  remains valid for unchanged surfaces.
- All six WASM artifact profiles, including Typst with verified Binaryen 131, pass
  every original size budget. Earlier slim-artifact overages are superseded. No budget
  adjustment or additional production optimization was needed. Web ASCII Brotli has
  5,140 bytes of remaining headroom; preserve the existing gate for future changes.
- The third-party license contract and all 382 legal projections passed again.
- Local Binaryen 131 is available under
  `target/tools/binaryen-131/binaryen-version_131/bin/wasm-opt.exe`. The original
  untracked upstream-SVG staging directory is preserved.
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

- [Mermaid 12 completion and final artifacts](verification/2026-09/2026-09-29T081350Z-mermaid-12-alignment-completion-and-final-artifact-admission-92096f07dfdd4d1e85fd715cadd0f343.md)
