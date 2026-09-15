---
type: Verification Evidence
title: C7a transport catalog contract follow-up
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: a09453da0
tags: theme,c7a,transport,catalog,verification
---

# Scope and result

The current source revision passed the focused public catalog and qualification contract suites
without changing fixtures or generated outputs. This is a same-source contract observation; it is
not a claim that every packaged platform artifact has been rebuilt, and it does not freeze C7a.

# Executed checks

- `python3 -m unittest scripts.test_qualify_theme_presets scripts.test_theme_preset_catalog_contract` — 13/13 passed.
- `node --test platforms/node/tests/api-contract.test.mjs platforms/web/scripts/theme-catalog.test.mjs` — 48/48 passed.
- `dart run tool/abi3_contract_test.dart` from `platforms/flutter` — passed with `ABI 3 Dart contract tests passed`.
- The post-ARM64 focused archive/catalog and package-group suite — 91/91 passed; its command and
  log are recorded in the ARM64 archive verification record.

The qualification tests reject unknown profile, family, output, and admission identifiers before
projecting a qualified catalog. Node and Web retain open metadata where their transport contract
allows future identifiers while preserving known profile behavior. Flutter's ABI 3 consumer
contract also preserves future profile/admission catalog cells and rejects malformed or conflicting
conditions. These transport tests cover metadata handling, not an end-user portability decision.

These checks confirm the existing transport metadata and qualification-projection contracts. Packaged Web,
Node, Flutter, Typst, UniFFI and Native C ABI artifacts still require their final same-source owner
matrix and release-preflight receipts before C7a contract freeze. The selected next workspace release is `v0.8.0-alpha.7`; the candidate remains unfrozen
while the alpha.6 compatibility decision and final artifact matrix are open.


# Execution limits

Logs: `/tmp/merman-c7a-qualification-contracts.log`,
`/tmp/merman-c7a-transport-catalog.log`, and `/tmp/merman-c7a-flutter-abi3.log`.
Initial Flutter attempts used `dart test`, which is not this repository's runner and failed
before executing tests. The documented `dart run` command above succeeded; no test dependency
was added. No Rust test execution is attributed to these Python/JavaScript/Dart commands.

The local machine has Xcode 26.5 and Swift 6.3.2, with no additional Xcode installation or Swift
toolchain under the inspected standard application/toolchain directories. It cannot establish
the required Swift 5.9/Xcode 15.2 floor. A read-only GitHub run query for
`3fe3b1b970a1d5ae211a1f05510adb7f03efa4ba` returned no runs; there is no current-source hosted
preflight result to consume. Formal release preparation still needs the maintainer's version
selection and the subsequent immutable-source platform jobs.

# Theme acceptance rerun at `5980cc9ec`

At `5980cc9ec71b0a1c5181cde0f7816c7c8d935b24`, the four Cargo invocations ran sequentially with the workspace acceptance
configuration; tests within each invocation used nextest's default concurrency. Results were:

- C6 runtime, preset qualification, Block title retirement, Class edge-label-background retirement
  and Flowchart marker retirement: 10/10 passed, zero skips. Nextest marked one passing C6 test as
  `LEAK`; its cause was not investigated in this run. Passing assertions do not resolve that
  marker, and the marker alone does not establish a production memory leak.
- KTD23 legacy projection retirement: 4/4 passed, zero skips.
- KTD17 route-cutover runtime: 1/1 passed in 334.152 seconds. This is a slow existing authorization
  test, not a timeout or a failure.
- Renderer support discovery: 48/48 passed, zero skips, including unknown catalog identifiers
  remaining visible and resolving to `Unverified`.

Log: `/tmp/merman-c7a-current-retirement.log`. These results revalidate the merged Block/provider
retirement and support-discovery behavior at the named source. They do not replace the final
same-source platform matrix, selected release line, immutable preflight, or C7a contract freeze.

# C6 leak-marker recheck

The passing C6 test that received a nextest `LEAK` marker in the ten-target run was isolated twice.
The acceptance-configured `cargo test` invocation passed the test in 4.92 seconds with `png`; the
same acceptance-configured nextest invocation with the original `png,layout-cytoscape` feature set
passed in 5.00 seconds without a `LEAK` marker. The first direct `cargo test` attempt was not used
as evidence because it omitted the workspace acceptance cfg and consequently ran zero tests.
Logs: `/tmp/merman-c7a-c6-cargo-test-acceptance.log` and
`/tmp/merman-c7a-c6-original-features-recheck.log`.


# Test-only helper boundary follow-up

On the working tree based on `a6b5d75e2`, a repository-wide Rust reference check identified
four private helpers whose callers exist only in unit-test configurations:
`FamilyThemeProgram::resolve_text_style`, `SequenceConfigView::resolve_role_typography`,
`CssFontSizeContext::with_medium_px`, and `OperationWorkMeter::preflight_parsed_render`.
Each helper now uses `#[cfg(test)]`, matching its consumers. Their implementations and test
callers remain intact; production continues to use the existing metered and typed-property
paths. This is a build-boundary cleanup, not a measured runtime or artifact-size improvement.
Qualification metadata and historical acceptance helpers were not deleted merely because
an ordinary production build reports them as unused.

Validation on this working tree:

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render --lib --no-default-features -E 'test(diagram_theme::resolved::tests::) | test(sequence::config::tests::) | test(mermaid_style::tests::) | test(resources::tests::)' --test-threads 2` passed 72/72 selected tests; 2,483 tests were filtered out.
- `CARGO_BUILD_JOBS=1 cargo check --locked -p merman-render --no-default-features` passed and no longer reports these four unused methods. Other warnings remain; this is not a complete dead-code audit.

Logs: `/tmp/merman-c7a-test-helper-scope.log` and
`/tmp/merman-c7a-test-helper-production-check.log`.

The remaining Apple compiler-floor gate has a separate owner: `.github/workflows/ci.yml`
job `apple-swift-5-9-smoke` selects Xcode 15.2 and verifies Swift 5.9 before building and
checking the generated source. The `apple-xcframework` job in `release-preflight.yml` uses
`macos-15` without that compiler-floor selection. A passing preflight Apple build alone
cannot close the Swift 5.9 obligation. This is an inspection of workflow responsibilities,
not evidence of a new hosted run.

The release-contract decision recorded in the release-line assessment remains pending.
No compatibility exception, package-line change or C7a freeze was applied by this cleanup.

# Current candidate acceptance rerun at `88b23ad77`

The release-mode theme acceptance targets were rerun sequentially with the workspace acceptance
configuration and `png,layout-cytoscape`: C6 runtime, preset qualification, legacy projection
retirement, Block title retirement, Class edge-label-background retirement, Flowchart marker
retirement and route-cutover runtime. Nextest ran 15 tests across 7 binaries; all 15 passed with
zero skips. This revalidates the completed Block/provider retirement and current manifest digest;
it does not close the alpha.6 compatibility decision or the missing hosted artifact matrix.

# Cross-transport current-source rerun at `de9c3f5f1`

The current source reran the cross-transport contract lanes without changing fixtures:

- Node API and Web catalog tests: 48/48 passed.
- Python qualification, catalog and CLI archive contract tests: 67/67 passed.
- Rust bindings-core authoring/error/resource vectors plus Typst, Native C ABI and UniFFI shared support golden: 15/15 passed, with 393 unrelated tests filtered.
- Flutter ABI 3 contract runner (`dart run tool/abi3_contract_test.dart`): passed with `ABI 3 Dart contract tests passed`.

These are current-source contract and golden checks. They do not establish installed package
provenance, Windows execution, Apple Swift 5.9 execution, or the unresolved alpha.6 facade
compatibility contract.
