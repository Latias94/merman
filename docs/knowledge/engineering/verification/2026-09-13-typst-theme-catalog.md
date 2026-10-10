---
type: Verification Evidence
title: Typst theme catalog discovery through the installed plugin
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,typst,catalog,verification
---

# Scope

Local verification on macOS ARM64 for the Typst catalog changes accompanying this record,
based on `870c9940f`. This was a working-tree build, not clean-checkout release qualification.
The plugin ABI changes from 3 to 4 because the closed export surface gains
`theme_catalog_json()`. The wrapper exposes `theme-catalog()` and runtime discovery advertises
`theme-catalog` in `metadata_ids`.

Catalog projection uses the shared binding core with the constrained theme resource policy,
without constructing a render engine. The installed SVG plugin reports ten preset metadata rows
matching the shared `authoring-v1/preset-catalog.json` vector. Output IDs and resource limits are
profile-specific; the theme input ceiling is 512 KiB. An artifact without SVG reports no available
structured spec, presets, or theme output/resource rows. A contract without catalog admission
cannot call the new policy-aware core projection.

# Checks

All Cargo commands used `CARGO_BUILD_JOBS=2` and ran sequentially.

```text
cargo run --locked -p xtask -- gen-typst-profile-constants
cargo nextest run --locked -p xtask -p merman-bindings-core -p merman-typst-plugin --features merman-typst-plugin/svg,merman-typst-plugin/analysis -E 'package(merman-typst-plugin) | test(theme_preset_availability_uses_the_effective_compiler_policy) | test(typst) | test(wasm_module_surface)'
cargo nextest run --locked -p merman-typst-plugin --no-default-features --lib
cargo run --locked -p xtask -- typst-package-smoke --profile publish
cargo fmt --all --check
git diff --check
```

- Owner tests: 90 passed; the remaining 909 tests were outside the selected filter.
- No-default-feature plugin tests: 14 passed, including empty theme catalog behavior.
- Package smoke: 22 Typst documents compiled and nine expected failures matched their diagnostics.
  The build validates the closed WASM surface and invokes the new export through the actual
  plugin, comparing the full preset array against the shared golden. The Typst API fixture then
  discovers a preset ID and uses it to export its complete recipe.
- Formatting and whitespace checks passed.

Raw local logs are `/tmp/typst-catalog-owner-tests.log`,
`/tmp/typst-catalog-empty-final.log`, and `/tmp/typst-catalog-package-smoke.log`.

The private artifact manifest at `target/typst-wasm-artifacts/publish/manifest.json` records
input digest `24637e2712f268810ea89c99cbb162a538a0ad1675c3d305df641d336f25d2ef`.
The optimized, stripped plugin is 11,535,833 bytes with SHA-256
`85d00e796ac87bcbb7fd2a5f7303bf564505a2fbd20636b802f3372a961ef14b`.
This exceeds the existing 10,200,000-byte stripped Typst budget. The complete compression/size
matrix was not rerun, and this observation does not attribute the excess to the catalog change.
Budgets were not changed.

# Remaining boundary

This adds public discovery and a real Typst consumer to the shared preset metadata comparison.
It does not populate `qualified_cells`, prove arbitrary profile/admission combinations, complete
C5, or close C7a. Full workspace, browser, and other platform artifact gates were not repeated for
this slice. No package was published. Local builds retain the 0.3.0 import path until the next
candidate version is assigned; the published 0.3.0 package remains ABI 3.
