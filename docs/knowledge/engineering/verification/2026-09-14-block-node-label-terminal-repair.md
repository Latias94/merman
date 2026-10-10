# Block NodeLabel terminal repair — 2026-09-14

## Scope and source

Base commit: `6365279d6995b8e5bd6c17423afcb85bbf9213f1`, with the file contents recorded below. This is a local-worktree
verification. The clean-checkout replay below binds the subsequent commit. Neither run is
a full-workspace, browser, or release-profile claim.
C7a and the remaining Block bridge retirement stay open.

The previous label receipt compared a recomputed CSS value with itself, counted any nonempty
label, and replaced shared label CSS. Three new public-render tests first failed on shared
CSS leakage, empty-label rejection, and source-owned color being reported as Applied.

The repair resolves NodeLabel paint per terminal after layout, preserves Text author order,
and checks each checkpointed HTML/SVG label fragment. Missing, duplicate, mismatched, or
zero-area HTML terminals cannot seal the receipt. Empty/source-owned and overwritten fills
become NotApplicable; unconsumed winning sibling facets remain residual/incomplete. Unthemed
and themed requests without NodeLabel mechanisms do not build a label expectation cache.

KTD17 v86 binds 478 routes and 730 route-profile witnesses. The live legacy inventory has
28 routes, all Block. Generic Text still needs its compatibility provider; this change does
not retire that provider, its historical probes, or KTD23 authority.

## Validation

- Initial three regression tests: 0/3 passed before the fix; all passed after it.
- Final scoped Release nextest run: 3157 passed, 2 existing skips.
- Full SVG structure comparison passed with `--check-dom --dom-mode structure
  --dom-decimals 3 --diagnostic-browser-text-layout`.
- The run includes actual route-cutover SVG/PNG authorization, the Block title retirement
  integration target, the legacy projection inventory and all Block SVG tests.
- The optional `-D clippy::all` check failed on existing cross-module lints plus one new
  collapsible-if warning. The new warning was fixed; this strict crate-wide gate is not
  claimed as passing. Ordinary Clippy completed with existing warnings and no remaining
  warning in the new label plan.
- New coverage includes source/config/class ownership, mixed ownership, Text author order,
  overwritten rules, unsupported ordinal winners, strict mixed-facet rejection, and receipt
  mutations. Two independent read-only reviews found no confirmed code defects; their
  ownership and strict-admission test gaps were added before the final run.

Command:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance -p merman-bindings-core \
  --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg \
  --lib --test block_svg_test --test legacy_projection_retirement \
  --test block_title_legacy_projection --test route_cutover_runtime --no-fail-fast
```

## Clean-checkout replay

Commit `44d81d033fa5c9f1305b36142551d8989d6c2738` was cloned into an independent local Git
checkout with an empty `git status --porcelain` before and after the Rust run. The same command
above, additionally selecting `--test class_edge_label_background_legacy_projection`, passed
**3158 tests with 2 existing skips**. Cargo reused the main worktree's target directory; Cargo
compiled the workspace packages from the independent checkout paths. This is source-isolation
evidence, not a cold dependency-cache build. No tracked source was modified during this replay.

## Verified file identities

| Path | SHA-256 |
| --- | --- |
| `crates/merman-render/src/block/theme.rs` | `4c761cf029407eb9d4b65cfb5fd0838c538c4f70fcd5591f1c7e04f619b9de13` |
| `crates/merman-render/src/family/preparation.rs` | `4fb3afeae1b1d766396f5b9e3654e688d781efcd6a44fd70ff73c34ad5be9bb3` |
| `crates/merman-render/src/svg/parity/block/render.rs` | `b799b2a04d402ed5b736e04f257bec7991d29415076009e9c7daf6b78c2c1827` |
| `crates/merman-render/src/svg/parity/block/render/tests.rs` | `08c9f561e74be5ec3d48f8b5290eeade340cc76a9e10af29e1ee640839adb1ec` |
| `crates/merman-render/tests/block_svg_test.rs` | `436cb3331f55fb9422a9ec25e46ad45fe9a27d6478d0928d8fea82e508454f4e` |
| `crates/merman-theme-acceptance/src/cutover.rs` | `7b328c0568a8eba60259374f3c7c6e1242d2174904122e05037ed2c5ad7f598d` |
| `crates/merman-theme-acceptance/src/cutover_manifest.rs` | `cdcab00e4eabb4680526bf506ddc3a8e3bf9c551ac29e02c126ca69061034a39` |
| `crates/merman-theme-acceptance/tests/legacy_projection_retirement.rs` | `4ab8e90f89664dd5a77fe2904a1aef9a7a3d7b69bcb3ffdd14f32dbb3cd5c881` |
