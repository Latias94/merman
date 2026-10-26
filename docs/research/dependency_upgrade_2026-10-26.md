# Dependency Upgrade Validation - 2026-10-26

Scope: the Astro rendering-contract fix branches, based on development and release/0.9.x.
The development branch merged origin/main at 8eba29510 in 3e3f289d9, preserving both the
rendering fixes and upstream semantic/presentation capability boundaries. A follow-up test
migration is fe8f652aa. No release, tag, push, or Mermaid baseline change is included.

## Selection

- Rust: granit-parser/serde-saphyr 2.0.0, jsonschema 0.60.1, and compatible current
  versions selected from crates.io. Development also selects serde_json_canonicalizer 0.4.1.
  All three active Cargo lockfiles per branch are refreshed. The unused development-branch
  workspace `lopdf` declaration is removed rather than presenting an unused version bump as
  an upgrade. Historical issue reproduction packages and reference runtimes are not active
  upgrade targets.
- Explicit YAML comment support preserves source-rewrite safety. Regression coverage fixes
  quoted line folding, literal chomp behavior, and non-ASCII scalar whitespace semantics.
- LALRPOP generator/runtime advance together to 0.23.2; checked-in parser code is regenerated
  through xtask, not hand-edited.
- NAPI runtime/derive/build/CLI are admitted as 3.15.0/3.6.13/2.6.0/3.10.9.
- VS Code languageclient advances to 11.0.0; React/React DOM to 19.3.1, Playwright to
  1.64.1, and the pinned npm toolchain to 12.2.1. Existing workflow and package assertions
  advance with their selected tools.
- Gradle 9.8.2 includes the official distribution checksum. Modern Python builds use
  setuptools >=84.2.1; Python 3.9 source builds retain >=82.0.1,<83.
- Application/npm tool updates retain product versions and each branch's own Options schema.

## Deliberate Compatibility Boundaries

- Mermaid/reference graph: development 12.2.0 and maintenance 12.1.0 remain independently frozen.
- resvg/usvg 0.47 remains shared with the latest krilla-svg 0.8.1; isolated 0.49 upgrades would
  split the public font database and PDF adapter types.
- TypeScript 6.0.3 remains for application compiler-API and typescript-eslint <6.1 peer support;
  VS Code independently uses TypeScript 7.0.2.
- js-yaml 4.3.2 remains because 5.5.0 rejects an accepted multiline flow mapping.
- ZenUML core 3.x remains within the selected plugin's ^3.47.9 contract, not latest 4.11.0.
- Legacy svgtypes 0.11 remains public roughr compatibility, not an accidental duplicate.

## Validation

Development checks completed:

- Core/render/bindings/WASM/export/theme unit suite: 6,285 pass, one skipped.
  Additional YAML, config-schema, canonical JSON, native-font and public contract suite:
  2,946 pass, six skipped. The suites overlap; these are not a distinct-test total.
- NAPI: 23 Rust tests and actual development-addon loading, runtime catalog, sync/async
  operations, lifecycle/resource errors, scoped-icon construction and engine isolation.
- Clippy with warnings denied for the changed Rust libraries; Cargo deny advisories,
  bans, licenses and sources; generated sources; representative artifact closures;
  49 feature-matrix build cases and isolated consumer checks.
- Full 37-family SVG structure gate using the release xtask build and reviewed
  browser-text-layout receipts.
- Rust license report generation/check, third-party source/lock provenance and release
  projections; license support unit tests. RaTeX 0.1.15 source identity is
  `7d9f9a39218914bead8ffdf39b79028a01d12c22`; embedded font/license bytes are unchanged.
- Actual TypeScript 6.0.3 Web build; 119 Node script tests and 169 Web script tests
  pass (two browser tests initially skipped without an explicit browser environment). The
  Canvas browser contract subsequently passes in Chromium on both lines; the updated-WASM
  renderer browser lane remains deferred.

Maintenance checks completed independently:

- Core/render/bindings/WASM/export unit suite: 4,128 pass, no skips. All four config-schema
  acceptance-corpus contracts pass with jsonschema 0.60.1.
- NAPI: 18 Rust tests and the same actual addon/runtime/lifecycle/scoped-icon probes pass.
- Clippy with warnings denied, Cargo deny, generated sources, representative artifact
  closures, and the full 37-family release-mode SVG structure gate pass.
- Rust license reports, source/lock provenance and release projections pass; legal support
  unit suites pass on both lines (40 development, 42 maintenance).
- TypeScript 6.0.3 Web build, 103 Node script tests and 137 Web script tests pass; the latter
  initially skip two browser cases. The Canvas case then passes separately in Chromium.

Both lines' npm owner checks completed: VS Code check/build and 255 tests; npm legal
reports; six non-reference npm dependency graphs have zero reported vulnerabilities.
The frozen reference graph retains two low entries on development and five on maintenance.
The existing MSAL tooling override remains a compatibility risk documented in the policy.

Platform checks completed on both lines: Gradle 9.8.2 starts under JDK 17; 19 Android
build-helper tests and six Python wheel-license tests pass. Python 3.9 and 3.12 isolated
wheel builds and Python 3.9 sdist-to-wheel rebuilds select the intended build backend.
These packaging probes use a test dynamic library, not a new Merman runtime receipt.

Both exact `web-full` Cargo recipes compile for `wasm32-unknown-unknown`: development
remains math-free and maintenance retains its math capability. This is a cross-target
compile check, not a browser runtime or package-group receipt. Formatting and diff checks
pass on both lines.

Both lines' workflow-security unit suites pass with the updated npm pins (57 development,
38 maintenance). The DOMPurify
3.4.17 npm distribution used for generated-default verification is byte-identical to
`dist/purify.cjs.js` at locked source commit `a77a3e78b59f72fa4d1c83f22e121ad9cc7d3575`.
Local logs are not release receipts.

## Deferred Environment Lanes

- Android JVM tests did not finish within the bounded local compile window. Gradle startup
  and build-helper contracts pass; CI still owns the JVM/instrumentation and native NDK lanes.
- Playground application/browser typechecks and its prepared build initially fail because
  the five local Web package artifacts and opaque-realm generated runtime inputs are absent,
  not because of a diagnosed dependency API error. The existing Web build pipeline and Pages
  workflow must assemble that group before the application, browser and bundle checks.
- The Web renderer browser test, complete browser artifact rebuild/packaging, VS Code
  extension-host smoke and non-host native targets are not replaced by host unit tests.
  Existing pre-upgrade WASM bytes are not claimed as evidence for the updated Rust graph.
- Native-addon execution and platform wheel probes use development/test artifacts; no
  production size, performance, release preflight, publication or cross-platform qualification
  is claimed by this dependency update.
