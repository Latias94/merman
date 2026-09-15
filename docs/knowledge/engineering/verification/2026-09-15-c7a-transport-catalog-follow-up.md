---
type: Verification Evidence
title: C7a transport catalog contract follow-up
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 3fe3b1b970a1d5ae211a1f05510adb7f03efa4ba
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
matrix and release-preflight receipts before C7a contract freeze. The next workspace version/date
also remains unselected.


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
