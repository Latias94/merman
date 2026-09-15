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
contract also exercises future profile/admission catalog cells and remains fail-closed for usable
support claims.

These checks strengthen the cross-transport evidence for qualified catalog binding. Packaged Web,
Node, Flutter, Typst, UniFFI and Native C ABI artifacts still require their final same-source owner
matrix and release-preflight receipts before C7a contract freeze. The next workspace version/date
also remains unselected.
