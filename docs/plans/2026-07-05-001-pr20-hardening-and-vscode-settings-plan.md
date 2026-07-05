---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
execution: code
created: 2026-07-05
branch: feat/editor-core-language-intelligence
---

# PR20 Hardening and VS Code Settings Plan

## Goal Capsule

Close the remaining PR20 review findings with a fearless refactor pass across the Rust core, Android
bindings, Web/WASM packaging, and VS Code extension. The branch should leave no known high-confidence
P1/P2 review gaps, should expose a mature VS Code settings surface for LSP-style workflows, and should
land with focused tests plus packaging checks that prove the affected surfaces are real.

## Context

The current branch already added editor-core language intelligence, VS Code integration, WASM/Web
surfaces, and Android wrappers. Independent read-only reviews found that the implementation is close,
but several release gates can still produce false positives: Android Gradle can miss Kotlin classes,
resource-limit analysis can return a valid empty result, web package scripts can escape `pkg/`, and the
VS Code extension is not yet smoke-tested in an actual extension host.

External settings references used for the VS Code design:

- VS Code contributes extension settings through `contributes.configuration`, which backs the Settings
  UI and JSON settings tooling.
- VS Code UX guidance treats settings as the native configuration surface for booleans, dropdowns,
  lists, and key/value pairs.
- rust-analyzer groups its extension settings into LSP-style categories such as Server, Trace,
  Diagnostics, and generated analysis settings.

## Requirements

- R1: Android Gradle must compile and package the Kotlin wrapper classes and run instrumentation smoke
  tests against the real JNI surface.
- R2: `MermanReusableEngine` must use the same native load and ABI validation gate as `MermanEngine`.
- R3: Source byte limits must never produce a valid empty analysis when the limit is exceeded.
- R4: Source-limit diagnostics must use the same CRLF visible-column semantics as normal `SourceMap`
  diagnostics without copying huge source text.
- R5: Markdown/MDX unclosed Mermaid fences must still be addressable at EOF for completion, hover,
  selection, rename, and related editor operations.
- R6: LSP full-document `didChange` must avoid copying the old document text under the document-store
  lock.
- R7: Web package build and cleanup scripts must reject path traversal and only operate inside
  `platforms/web/pkg`.
- R8: WASM `editorRename` must return the same structured binding error payload style as the rest of
  the Web API.
- R9: Web prepack checks must include every public exported runtime and type file.
- R10: VS Code preview webview messages must be bounded or coalesced while the webview is not ready.
- R11: VS Code extension settings must be organized as a native settings surface, including server,
  trace, language intelligence, diagnostics, analysis, preview/export, and developer controls where
  applicable.
- R12: VS Code CI must include a minimal extension-host smoke that proves activation and command
  registration inside VS Code, not only Node-level helper tests.
- R13: Public `docs/knowledge` files must not publish transient agent session/progress metadata.

## Key Technical Decisions

- KTD1: Treat `max_source_bytes` as a hard resource guard. Rule configuration may affect presentation
  later, but exceeding the configured guard must halt analysis with a diagnostic rather than silently
  returning a valid empty payload.
- KTD2: Keep Android native initialization centralized behind the existing `MermanEngine` JNI symbols
  to avoid widening JNI symbol names or duplicating ABI logic.
- KTD3: Use VS Code native `contributes.configuration` categories instead of a custom settings webview.
  This matches VS Code conventions and LSP extension practice, gives users Settings UI search/filtering
  for free, and keeps configuration in `settings.json`.
- KTD4: Web script path validation should resolve paths structurally and require the target to be
  `pkg` or a descendant of `pkg`; do not rely on string prefix checks.
- KTD5: Pending preview messages should preserve latest observable UI state, not stale render history.
  The ready replay should be small, deterministic, and robust when `postMessage` rejects.

## Implementation Units

### U1 Android Gradle Kotlin Packaging

Files:

- `platforms/android/build.gradle.kts`
- `scripts/verify-platform-bindings.py`
- `.github/workflows/ci.yml`

Work:

- Apply the Kotlin Android Gradle plugin using the same Kotlin line already used by CI tooling where
  practical.
- Configure JVM 17 for Kotlin compilation.
- Wire `examples` as an Android test Kotlin source root, not only a Java source root.
- Add a verifier that checks the release AAR contains expected Kotlin wrapper classes and that
  instrumentation output includes `MermanInstrumentedSmokeTest`.
- Keep existing standalone `kotlinc` wrapper verification if it still adds value.

Tests:

- Python unit coverage for any new verifier helper.
- Android Gradle smoke where available; if local Android SDK/emulator is unavailable, CI command and
  verifier coverage must still be added.

### U2 Android Native Runtime Gate

Files:

- `platforms/android/src/main/kotlin/io/merman/MermanEngine.kt`
- `platforms/android/src/main/kotlin/io/merman/MermanReusableEngine.kt`
- `platforms/android/examples/MermanSmoke.kt`
- `platforms/android/README.md`

Work:

- Expose an internal `MermanEngine.ensureNativeReady()` entry that triggers object initialization and
  shared ABI checks.
- Remove duplicate `System.loadLibrary` from `MermanReusableEngine` and call the shared gate before
  `nativeNew`.
- Make smoke coverage construct `MermanReusableEngine` before any static render path.

Tests:

- Kotlin compile smoke.
- Android instrumentation smoke when Android tooling is available.

### U3 Analysis Resource Guards and CRLF Spans

Files:

- `crates/merman-analysis/src/analyzer.rs`
- `crates/merman-analysis/src/source_map.rs`
- `crates/merman-analysis/src/rules.rs`
- `crates/merman-analysis/tests/**`

Work:

- Prevent disabled or filtered diagnostics from turning a source-limit hit into `valid: true`.
- Make `whole_text_span_without_source_copy` mirror `SourceMap::span(0, text.len())` for CRLF and
  terminal CR cases while keeping the no-copy property.
- Add regression tests for disabled `merman.resource.source_bytes_exceeded`, CRLF source-limit spans,
  and normal under-limit analysis.

Tests:

- `cargo nextest run -p merman-analysis --cargo-quiet`

### U4 Editor Snapshot and LSP Change Handling

Files:

- `crates/merman-editor-core/src/snapshot.rs`
- `crates/merman-editor-core/tests/**`
- `crates/merman-lsp/src/server.rs`
- `crates/merman-lsp/tests/**`

Work:

- Extract a small fence inclusion helper and allow EOF to hit unclosed Markdown/MDX fences.
- Update `didChange` full sync to take the last change text directly and avoid the old-text clone under
  lock.
- Add tests for EOF fence lookup with and without trailing newline, and for full-sync empty-change/no
  old-copy behavior where local test hooks make that practical.

Tests:

- `cargo nextest run -p merman-editor-core --cargo-quiet`
- `cargo nextest run -p merman-lsp --cargo-quiet`

### U5 Web Package Script Boundaries

Files:

- `platforms/web/scripts/arg-parse.mjs`
- `platforms/web/scripts/build-wasm.mjs`
- `platforms/web/scripts/clean-pkg.mjs`
- `platforms/web/scripts/*.test.mjs`

Work:

- Add a shared resolver for package-relative output directories.
- Reject absolute paths, drive/UNC paths, empty paths, `..` traversal, and resolved paths outside
  `platforms/web/pkg`.
- Reuse the resolver in both build and cleanup scripts.

Tests:

- `node --test platforms/web/scripts/*.test.mjs`

### U6 WASM and Web Rename Error Contract

Files:

- `crates/merman-wasm/src/lib.rs`
- `platforms/web/src/index.ts`
- `platforms/web/scripts/smoke.mjs`

Work:

- Map `RenameError` variants into structured binding errors with stable code names and messages.
- Preserve JS consumer ability to use `isBindingErrorPayload(error)`.
- Add smoke coverage for invalid rename and no-symbol rename paths.

Tests:

- Focused `cargo nextest`/`cargo test` for `merman-wasm` under editor-language features.
- `npm run smoke --prefix platforms/web`

### U7 VS Code Preview Queue

Files:

- `tools/vscode-extension/src/preview-webview-client.ts`
- `tools/vscode-extension/src/preview-messages.ts`
- `tools/vscode-extension/src/test/preview-webview-client.test.ts`

Work:

- Replace the unbounded pending message array with a bounded/coalescing state.
- Preserve required source-list/source-selection UI messages and latest render lifecycle/result.
- Keep replay failure behavior deterministic: unposted pending state remains available for retry.

Tests:

- Existing preview webview tests.
- New test proving many pre-ready render messages replay as a bounded latest state.

### U8 VS Code Settings Surface and Extension-Host Smoke

Files:

- `tools/vscode-extension/package.json`
- `tools/vscode-extension/src/config.ts`
- `tools/vscode-extension/src/extension.ts`
- `tools/vscode-extension/src/test/**`
- `tools/vscode-extension/src/extensionHostSmoke.ts`
- `tools/vscode-extension/scripts/**`
- `.github/workflows/vscode-extension.yml`
- `.github/workflows/release-preflight.yml`

Work:

- Convert the flat Merman configuration contribution into categorized native VS Code Settings groups:
  Server, Language Intelligence, Diagnostics, Analysis, Preview and Export, and Development.
- Add useful schema metadata: `scope`, `order`, enum descriptions, numeric minimums, and markdown
  descriptions that explain operational impact without marketing copy.
- Add preview/export settings only where current code can actually honor them; avoid decorative dead
  configuration.
- Add tests that package configuration schema and config parser stay in sync.
- Add a minimal extension-host smoke script using VS Code test infrastructure: activate extension in a
  fixture workspace, assert core commands exist, and execute a harmless command path with language
  intelligence disabled if needed.
- Wire the smoke into VS Code workflow and release preflight.

Tests:

- `npm test --prefix tools/vscode-extension`
- `npm run test:extension-host --prefix tools/vscode-extension`

### U9 Public Knowledge Hygiene

Files:

- `docs/knowledge/engineering/**`

Work:

- Remove `source_session` and transient progress type metadata.
- Retain durable engineering facts, verification evidence, and decision context.

Tests:

- `rg -n '(source_session|type: "Progress"|type: Work Progress)' docs/knowledge` should return no
  matches.

### U10 Final Verification and Review

Work:

- Run formatting and whitespace checks.
- Run focused tests for each touched package.
- Run a final read-only subagent review on the changed diff.
- Commit in logical conventional commits. Push the PR branch after a green-enough local verification
  pass, noting any environment-bound checks that could only be wired for CI.

Verification commands:

- `cargo fmt --all --check`
- `git diff --check`
- `cargo nextest run -p merman-analysis --cargo-quiet`
- `cargo nextest run -p merman-editor-core --cargo-quiet`
- `cargo nextest run -p merman-lsp --cargo-quiet`
- `npm test --prefix tools/vscode-extension`
- `npm run test:extension-host --prefix tools/vscode-extension`
- `node --test platforms/web/scripts/*.test.mjs`
- `npm run build --prefix platforms/web`
- `npm run smoke --prefix platforms/web`
- `npm run prepack --prefix platforms/web`
- Android Gradle and emulator checks where the local Android SDK/emulator is available.

## Execution Order

1. Land low-risk correctness fixes first: U3, U4, U5, U9.
2. Land platform binding hardening: U1, U2, U6.
3. Land VS Code queue/settings/smoke: U7, U8.
4. Run verification, request read-only subagent review, fix findings, then commit and push.

## Done Definition

- Every requirement R1-R13 has implementation or a documented environment-bound verifier.
- Focused tests pass locally unless blocked by missing Android/emulator infrastructure.
- No remaining high-confidence P1/P2 finding from the read-only reviews is unaddressed.
- The PR branch contains reviewable conventional commits and is pushed.
