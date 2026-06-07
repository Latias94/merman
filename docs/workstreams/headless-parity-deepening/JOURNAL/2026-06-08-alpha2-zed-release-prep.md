# HPD-050 - Alpha.2 Zed Release Prep

Task: HPD-050 release-boundary compatibility and release-preflight cleanup

## Context

`repo-ref/zed` currently pins Zed's wrapper to a patched `v0.6.2` fork:

`git+https://github.com/zed-industries/merman?tag=v0.6.2-with-patches#06094471f97acb10d0eebf8b92bac19ba2928eea`

That wrapper is a concrete downstream host integration for the next alpha. It uses merman as a
headless editor-preview renderer with Zed theme variables, vendored text measurement, configured
SVG ids, `SvgPipeline::resvg_safe()`, `CssOverridePostprocessor::strip_existing_important()`, and
sync rendering.

## Changes

- Added `crates/merman/tests/zed_editor_contract.rs` to lock Zed's public API combination before
  `0.7.0-alpha.2`.
- Updated `docs/alignment/ZED_MERMAID_ISSUE_AUDIT.md` with the current Zed fork pin and alpha.2
  contract boundary.
- Finalized `CHANGELOG.md` and platform changelogs for `0.7.0-alpha.2`.
- Bumped workspace and platform package metadata to `0.7.0-alpha.2`; Python uses `0.7.0a2`.
- Updated release docs for the alpha.2 publish target and publish-order dependency blockers.

## Verification

- `python scripts/release-version.py check --version 0.7.0-alpha.2` - passed.
- `cargo +1.95 fmt --check` - passed.
- `git diff --check` - passed.
- `cargo +1.95 nextest run -p merman --features render --test zed_editor_contract` - passed,
  `3` tests run.
- `cargo +1.95 nextest run -p merman --features render --test zed_mermaid_issue_fixtures --test zed_pr_57644_corpus` -
  passed, `8` tests run.
- `cargo +1.95 package -p dugong-graphlib --allow-dirty` - passed.
- `cargo +1.95 package -p manatee --allow-dirty` - passed.
- `cargo +1.95 package -p merman-core --allow-dirty` - passed.
- `cargo +1.95 package -p <crate> --allow-dirty --list` passed for `12` release-preflight crates.
- `cargo +1.95 nextest run --workspace` - passed, `2072` tests run, `2072` passed, `5` skipped.
- Filtered production scan across `merman-core`, `merman-render`, `dugong`, `dugong-graphlib`,
  and `manatee`, excluding tests, same-file `#[cfg(test)]` regions, and comments, reports only the
  generated/static `merman-core` JSON validity checks:
  - `crates/merman-core/src/generated/mod.rs:13`
  - `crates/merman-core/src/theme.rs:324`
  - `crates/merman-core/src/theme.rs:327`

## Boundary

This prepares the repository for `0.7.0-alpha.2`; it does not publish crates, tag the repository,
or run platform CI-only gates for Python wheels, Android AARs, Apple XCFrameworks, or Flutter dry
runs. Those remain release workflow responsibilities.
