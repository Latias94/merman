---
type: Verification Evidence
title: C7a browser and native binding verification
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: b7a52635355021a27ad5fa00f53f39647de2c04a
tags: theme,c7a,browser,bindings,verification
---

# Scope and result

At source `b7a526353`, the prescribed Pages browser matrix passed: 110 desktop Chromium,
49 Firefox/WebKit smoke, and 12 Chromium mobile tests. The local native binding owner also
passed Android transport clippy, Flutter generation/analysis/contracts and rebuilt macOS
ARM64 Native Assets consumers. This record does not declare C7a contract freeze or device
and operating-system coverage that was not executed.

The browser run used the clean checkout `/tmp/merman-c7a-fde874d51`, advanced to the source
above. Its production Playground build was generated from the same bytes before the two
final commits; those commits contain only the already-built toolbar change and tests.
Node 24.21.0/npm 12.0.2 and Playwright 1.62.1 match the owner toolchain. Cargo ran in one
stream, with at most two jobs and two Rayon threads. The native verifier ran in the primary
worktree at the same commit; tracked files remained clean, with only the previously present
untracked August engineering-note directories. It is not a second independent native checkout.

# Repaired failures

- Firefox's Packet byte-label whole-element rectangle extended three pixels left of the
  character-cell start. Chromium and WebKit reported the same character-cell position.
  `baf659657` measures SVG text with `getExtentOfChar()` and maps the cells through
  `getScreenCTM()`. It keeps the real pixel-contribution check, positive-area/visibility
  rejection and containment tolerance. Added transformed-surface and displaced-child-tspan
  cases pass in all three browsers; the complete text-surface suite passed 69 tests.
  The renderer, font choice and rendered SVG were unchanged.
- `cd5ceca00` restores the existing localized SVG pipeline descriptions as accessible
  descriptions on the actual settings menu choices. Both desktop and narrow-layout
  guidance flows now pass, including the persistent fallback warning and return action.
- `b7a526353` checks that presentation toggles preserve authored Sequence title markup and
  the root viewBox, while retaining the overflow/clipping assertions. Exact browser font
  bounding floats are not an authored-geometry invariant across viewport scaling.
- The former 100-million-unit positive fixture exceeded the renderer's 16777216 coordinate
  ceiling. The test now requires an explicit rejection at that size and separately proves
  preview, zoom, fit, bounds overlay, SVG download and bounded PNG export for an admitted
  10-million-unit SVG. The production resource ceiling is unchanged.
- `e496dc4c5` corrects the migration guide's claim that unreleased theme changes shipped in
  alpha.6. It now names first-release theme v1, candidate UniFFI API 7, Typst ABI 3 and CLI
  contract 6 against their published baselines, and retains earlier migration advice as such.

# Executed owners

| Owner | Result and scope |
| --- | --- |
| Public acceptance boundary | At clean `747f359b2`, package listings exclude private acceptance modules; an external Rust consumer resolves production APIs with all public producer features and rejects each private/retired import. No Rust source changed between that run and this record. |
| Playground preparation | At `747f359b2`, `npm ci`, dependency-boundary verification, existing Web WASM input freshness, opaque-realm preparation and verification passed. Prepared unit suites passed 412 tests; lint, browser typecheck and the production build passed. |
| Changed Playground checks | Browser typecheck, lint and production rebuild passed after the toolbar change; 18 targeted presentation/guidance browser tests passed. |
| Final non-Chromium matrix | At clean `b7a526353`, `npm run test:browser:smoke:non-chromium:built --prefix playground`: 49/49, including Firefox, WebKit desktop and WebKit mobile. |
| Final Chromium desktop | Same source, `npm run test:browser:chromium:desktop:built --prefix playground`: 110/110. |
| Final Chromium mobile | Same source, `npm run test:browser:mobile:built --prefix playground`: 12/12. |
| Native binding owner | At `b7a526353`, `python3 scripts/verify-platform-bindings.py`: both Android ARM64 transport recipes passed clippy with `-D warnings`; the macOS Flutter native library rebuilt; ffigen produced no ABI diff; Flutter analysis, Dart formatting and ABI contracts passed. |
| Flutter Native Assets consumer | The same owner exercised rendering plus two theme materializations, 22 support queries, five errors and all three budgeted authoring operations through both one-shot and reusable consumers. |

The existing [artifact record](2026-09-15-c7a-post-merge-artifacts.md) separately owns the
11102-test workspace run, 13 optimized acceptance integrations, CLI qualification/replay,
installed Python and Node consumers, Web packages and Typst. Those artifacts retain their
actual source identities; this browser run does not relabel them or add qualification cells.

Logs:

- `/tmp/merman-c7a-747f359b2-public-boundary.log`
- `/tmp/merman-c7a-747f359b2-playground-{install,dependencies,prepare,tests,lint,build}.log`
- `/tmp/merman-c7a-text-geometry-{typecheck,lint,browser}.log`
- `/tmp/merman-c7a-preview-{typecheck,lint,build,browser}.log`
- `/tmp/merman-c7a-b7a526353-browser-{non-chromium,chromium,mobile}.log`
- `/tmp/merman-c7a-b7a526353-platform-bindings.log`

# Remaining delivery scope

Android clippy is not AAR packaging or device execution. The local machine has the pinned
NDK and JDK 17, plus an API 36 ARM64 emulator image, but this record did not boot it or run
instrumentation. Final Android package/device checks, remaining Apple host/simulator and
Linux/Windows artifact owners, archive/profile publication binding and formal C7a candidate,
rollout and contract decisions remain open. Browser smoke proves the application paths above;
it does not create Browser SVG portability qualification. Keep the existing host-dependent
native preset cells and empty cells for unqualified artifact profiles.

The subsequent [Android artifact/device record](2026-09-15-c7a-android-artifact-verification.md)
closes the local AAR/Maven/API 36 ARM64 tranche at its own source revisions. It does not
change the scope or source identity of the browser/platform observations above.
