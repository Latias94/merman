---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
execution: code
created: 2026-07-05
branch: feat/editor-core-language-intelligence
origin: subagent-review-2026-07-05
---

# PR20 Subagent Followups Plan

## Goal

Close the second full read-only subagent review for PR20 by fixing the remaining P1/P2 defects in VS Code activation, CI release gates, editor-core completion semantics, Android wrapper verification, and web WASM packaging boundaries.

## Priority Order

1. Keep VS Code activation resilient when language-server startup fails.
2. Make Android instrumentation CI deterministic.
3. Declare and test VS Code Workspace Trust behavior.
4. Reject invalid VS Code analysis settings before they reach the LSP.
5. Fix preview diagnostic ranges for unclosed EOF Mermaid fences.
6. Fix shape completion brace insertion around existing braces and Markdown fence boundaries.
7. Expand Android AAR verification to every public wrapper class.
8. Harden web `pkg` script boundaries against symlink and junction escapes.
9. Add focused regression tests and run the affected verification matrix.
10. Run a final read-only review, then commit and push the PR branch.

## Requirements

- R1: VS Code extension activation must complete even if the language server binary is missing, fails to construct, or fails to start. Non-LSP commands and preview/export commands must remain registered.
- R2: Restart/configuration-change paths must not show a success notification after startup failure.
- R3: Android instrumentation CI must install or resolve a deterministic Gradle executable before invoking `verify-platform-bindings.py --gradle-path`.
- R4: VS Code manifest must declare limited Workspace Trust support, while runtime trust checks continue to block unsafe custom executable paths, server arguments, and Cargo-run development settings.
- R5: Extension-host smoke must execute at least one harmless contributed command path, not only observe activation side effects.
- R6: VS Code analysis configuration must expose integer/range validation in `package.json` and must sanitize invalid user values before sending settings to the LSP.
- R7: Markdown/MDX preview-source extraction must include the final content line of an unclosed EOF Mermaid fence in `diagnosticRange`.
- R8: Shape completion must not duplicate an existing `}` and must decide whether to append a closing brace using the current diagram or fence body boundary, not the host document tail.
- R9: Android AAR verification must require every public Kotlin wrapper class exposed by the runtime callback API, including text-measure request/result classes.
- R10: Web package scripts must reject existing symlink/junction path components under `platforms/web/pkg` before build, clean, pack, or test operations write/delete through them.
- R11: All fixes must have targeted regression tests or an explicit local-environment limitation in the final handoff.

## Key Technical Decisions

- KTD1: Treat LSP startup as an optional subsystem during activation. Activation registers all commands first, then attempts LSP reconciliation through a failure-isolating path.
- KTD2: Keep user-triggered restart strict enough to report failures, but do not claim success unless the restart action actually completed.
- KTD3: Validate VS Code settings at both schema and runtime-normalization layers. The extension should not rely on the Rust LSP JSON parser rejecting invalid settings.
- KTD4: Track whether a Markdown fence has a real closing fence. EOF and closing-fence ranges have different end-line semantics.
- KTD5: Shape completion should prefer parser expected spans, and brace insertion must be bounded by the active source body rather than the full host document.
- KTD6: Android release verification should test the artifact surface that downstream apps compile against, not only the primary facade classes.
- KTD7: Web script path validation should combine lexical containment with filesystem-aware checks of existing path components, while preserving first-build behavior when `pkg` does not exist yet.

## Implementation Units

### U1: VS Code Activation Failure Isolation

- Files:
  - `tools/vscode-extension/src/extension.ts`
  - `tools/vscode-extension/src/language-client-start.ts`
  - `tools/vscode-extension/src/test/language-client-start.test.ts`
  - `tools/vscode-extension/src/extension-host-smoke.ts`
- Work:
  - Catch activation-time and configuration-change LSP reconciliation failures after status/error reporting.
  - Ensure `createLanguageClient` failures update status and surface a clear error.
  - Ensure restart does not show a success notification after failed stop/start.
  - Add or update unit/smoke coverage for failure isolation and a harmless command execution path.

### U2: VS Code Workspace Trust Contract

- Files:
  - `tools/vscode-extension/package.json`
  - `tools/vscode-extension/src/test/binaries.test.ts`
  - `tools/vscode-extension/scripts/run-extension-host-smoke.mjs`
- Work:
  - Add `capabilities.untrustedWorkspaces.supported = "limited"`.
  - Preserve existing runtime trust gates for unsafe custom execution settings.
  - Add manifest-level regression coverage.

### U3: VS Code Analysis Settings Validation

- Files:
  - `tools/vscode-extension/package.json`
  - `tools/vscode-extension/src/config.ts`
  - `tools/vscode-extension/src/test/language-intelligence.test.ts`
- Work:
  - Use integer schema types and minimum/maximum bounds for numeric LSP settings.
  - Normalize only safe integer values in extension config construction.
  - Add regression tests for fractional and out-of-range values.

### U4: Preview EOF Fence Diagnostics

- Files:
  - `tools/vscode-extension/src/preview-source.ts`
  - `tools/vscode-extension/src/test/preview-source.test.ts`
  - `tools/vscode-extension/src/test/preview-diagnostics.test.ts`
- Work:
  - Track real closing fences separately from EOF termination.
  - Include final content line for unclosed EOF Mermaid fences.
  - Test that diagnostics on the last content line are retained.

### U5: Editor-Core Shape Completion Braces

- Files:
  - `crates/merman-editor-core/src/context.rs`
  - `crates/merman-editor-core/tests/completion.rs`
- Work:
  - Avoid appending a closing brace when one already follows the cursor.
  - Limit missing-brace detection to the active diagram/fence body.
  - Add regression tests for inline shape values and Markdown fenced diagrams.

### U6: Android CI and AAR Verification

- Files:
  - `.github/workflows/ci.yml`
  - `scripts/verify-platform-bindings.py`
  - `scripts/test_verify_platform_bindings.py`
- Work:
  - Pin or install Gradle for Android instrumentation verification.
  - Require public text-measurer request/result classes in AAR verification.
  - Add unit tests for missing-class failures and positive reports.

### U7: Web WASM Package Path Hardening

- Files:
  - `platforms/web/scripts/arg-parse.mjs`
  - `platforms/web/scripts/arg-parse.test.mjs`
- Work:
  - Reject existing symlink/junction components in package output paths.
  - Keep non-existent `pkg` paths valid for first builds.
  - Add regression tests for symlink/junction escape attempts.

### U8: Verification, Review, Commit, Push

- Work:
  - Run focused Rust, VS Code, web, and Python tests.
  - Run formatting/check commands for modified areas.
  - Dispatch read-only subagents for final review across the fixed surfaces.
  - Commit with Conventional Commit messaging.
  - Push `feat/editor-core-language-intelligence` after local verification.

## Verification Plan

- `cargo fmt --all --check`
- `cargo nextest run -p merman-editor-core --cargo-quiet`
- `npm test --prefix tools/vscode-extension`
- `npm run test:extension-host --prefix tools/vscode-extension`
- `node --test platforms/web/scripts/*.test.mjs`
- `python -m unittest scripts/test_verify_release_crate_order.py scripts/test_workflow_path_filters.py scripts/test_verify_platform_bindings.py`
- `git diff --check`

## Risks

- Extension-host smoke may depend on the locally downloaded VS Code test binary; if unavailable, unit coverage must still prove command registration and failure isolation.
- Android emulator instrumentation remains CI-owned in this environment unless a local Android SDK and emulator image are available.
- Symlink/junction behavior differs by platform and privileges; tests should skip only the OS operation that cannot be created locally, while keeping path-guard unit coverage deterministic where possible.

