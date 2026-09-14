# Cross-transport theme catalog golden verification

Date: 2026-09-14

This record covers the current source revision and the shared authoring-v1 catalog vectors. It is
an execution record, not a qualification receipt or a C7a contract freeze.

## Verified consumers

- Node API contract: `node --test tests/api-contract.test.mjs` — 40/40 passed.
- Web catalog contract: `node --test scripts/theme-catalog.test.mjs` — 8/8 passed.
- Rust theme contract and binding-core suites — 161/161 passed in the current owner run.
- Typst plugin and smoke contracts: `cargo nextest run ... -E 'test(typst_plugin_smoke) | package(merman-typst-plugin)'` — 41/41 passed.
- Scoped CLI qualification contract: `python3 -m unittest scripts.test_qualify_theme_presets` — 10/10 passed.

Node and Web both preserve unknown profile and admission identifiers as open metadata, retain the
shared qualified-cells vectors without rewriting them, and reject malformed or conflicting cells.
The release-side CLI projection separately fails closed for unknown qualification profile, family,
output, or admission values before producing an artifact-bound catalog.

## Remaining coverage

This record does not claim current-source execution of UniFFI, Native C ABI, Flutter, or real
Web/WASM packaged artifacts. Those consumers remain owned by their platform CI and release
preflight lanes and require a same-revision matrix run before the C7a contract can freeze.
