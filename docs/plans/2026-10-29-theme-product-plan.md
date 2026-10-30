# Theme product readiness

Date: 2026-10-29. Status: implemented and locally verified; remote owner checks pending.

## Goal and evidence

Make theme selection predictable for users and developers before a public
`v0.10.0-alpha.1`. The product audit at `4178dca33` reproduced a default Playground
font overriding recipe typography, CLI silence about partial theme application,
mixed editing semantics, limited import recovery, and stale release-contract
expectations. Keep the working compiler, prepared renderer, and Rust-owned catalog.

This plan supersedes the immediate-setting behavior in the earlier theme-discovery
plan. It does not publish a release, change maintenance `0.9.x`, resume general
performance work, or freeze experimental font-resource contracts.

## Decisions

- New workspaces inherit theme typography. Only an explicit font choice injects
  font configuration. Historical legacy/s2/s3 links retain their old defaults;
  new inherited selections are encoded explicitly.
- Panel-owned base theme, font, recommended configuration, and Merman recipe use
  one preview/Apply/Cancel model. Source and independent config edits stay live.
  Draft configuration is a sparse patch merged with the latest authored config;
  Apply never restores an old whole-workspace snapshot over concurrent edits.
- Retain the last validated candidate independently of a failed import attempt.
  Recovery invalidates late asynchronous imports and validations.
- Show applicable family guidance and actionable outcomes first. Preserve exact
  execution evidence in optional details, distinguish catalog load failure from
  loading, and disclose source/config overrides without a second Mermaid parser.
- Reuse existing CLI execution evidence for concise diagnostics and explicit
  reporting/strict policy. Do not change image stdout or issue new support claims.
- Keep pure recipe exchange distinct from full workspace reproduction. Document
  one precedence policy and the experimental resource/output discovery surface.
- Fix stale shared fixtures and synthetic catalog inputs at their owners rather
  than weakening assertions or introducing new release checks.

## Ownership and validation

- Main agent: font/input semantics, sparse draft/store lifecycle, shared appearance
  projection, developer documentation, integration, browser reproduction, commits.
- UI worker: theme panel/cards/evidence, translations and browser scenarios.
- CLI worker: CLI evidence, policy/report interface and focused regressions.
- Contract worker: Python synthetic catalog inputs, shared support golden, Rust
  preset projection tests, focused Python tests.

Workers have disjoint write scopes in the shared feature worktree. Cargo runs are
sequential, use nextest and the existing shared target with two build jobs. Owner
checks are focused CLI/theme Rust tests, catalog Python tests, Web shared-vector
smoke, Playground runtime/store/share/config tests, type/lint/build checks and
Chromium theme interactions. Existing CI owns the broad platform matrix. There
are no renderer, comparator, dependency or feature-profile changes planned.

## Acceptance

- A fresh Ghibli Pie uses its recipe font stack; explicit fonts still override it,
  and old share links render with their historical font selection.
- Every panel appearance change previews without changing the share snapshot;
  Apply commits coherently, Cancel discards drafts and preserves source/config
  edits, and recommended config preserves unrelated latest keys.
- Invalid live JSON is never replaced or masked by stale config. Failed imports
  can recover the last valid selection, and stale callbacks cannot overwrite it.
- Pie does not offer Flowchart mutations. Known source/config ownership has a
  visible explanation and editing path. Target evidence remains accessible.
- CLI partial theme application reports useful diagnostics; strict rendering and
  machine reports consume the same result without rendering twice.
- Current required CI failures are resolved through their causal contracts.

## Completion

The implementation preserves the existing renderer and its theme ownership. New workspaces
inherit recipe/source fonts; legacy/s2/s3 default interpretation remains independent. Panel
appearance choices commit coherently through sparse patches, and invalid live configuration
cannot be replaced by a stale snapshot. Import recovery rejects late work and retains valid
custom recipes. Application errors and import errors have separate recovery paths.

Current-family recommendations, explicit declaration hints, catalog retry, alpha labels and
optional target details keep the ordinary path short. Recovery and recommendation actions
retain keyboard focus. The gallery contains before/after screenshots from the real Playground.

CLI uses existing render evidence for human diagnostics, ordered JSON reports and strict target
policy. Reports cannot alias inputs, stdio or image/document outputs; new case-only filename
collisions are rejected before input acquisition. Native feedback stays outside `mmdc` and
rustdoc. Completion and manual assets were regenerated with their existing owner.

Verification:

- 838 Python hygiene contracts passed, including archive, install and qualification consumers.
- 59 renderer catalog/support contracts passed, including the shared support golden.
- 385 CLI tests passed with one intentional skip, covering all graphical formats, partial
  application, strict rejection, report-path protection, batch ordering, output integrity,
  compatibility silence and transaction behavior. No-feature and SVG-only builds also passed.
- CLI asset generation and currentness checks passed under both existing temporal environments.
- All-target/all-feature CLI Clippy passed with `-D warnings`.
- Full Web package smoke passed using the verified unchanged `c43e3f7b8611` WASM inputs.
- 509 prepared Playground tests passed, including 178 runtime/store and 32 config/appearance
  tests; TypeScript, changed-surface ESLint and the production artifact graph passed.
- Chromium custom-theme, design-scope and initialization scenarios passed. The final changed
  ergonomics suite passed all 13 cases; 320px, mid-width and shortened mobile checks passed.
- Firefox and WebKit startup/render/Compare/focus smoke passed.
- A real Ghibli preview used `"Open Sans", "Noto Sans SC", sans-serif`; explicit Georgia and
  return-to-inherit were observed. The CLI audit recipe now emits `unsupported-geometry` with
  the matching JSON report while retaining the supported stroke.
- Independent correctness and adversarial reviews found five edge cases; all were repaired,
  covered by targeted tests and re-reviewed without remaining findings.
- `cargo fmt --all -- --check`, `git diff --check` and indexed path-length checks passed.

The first remote run passed hygiene, Windows/macOS core, Node WASM/N-API, Python, platform
bindings and the Web package owner. Typst progressed past its repaired shared support test,
then exposed another old ten-preset assertion in its package-level API fixture. That fixture
now verifies the complete ID projection including Ghibli. Reusing the exact CI-uploaded plugin
with Typst 0.15.1, all 20 package tests passed locally, including nine expected failures. The
follow-up changes only this fixture and documentation; its current-head owner checks remain
the final remote evidence.

The unchanged renderer corpus was not rebuilt again for host/UI/CLI-only implementation work.
CI remains the owner of the full SDK/OS matrix and Linux SVG containment. This work does not
publish the alpha release or create qualification cells.
