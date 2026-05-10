# Fearless Refactor Gates

This page records the standard command sets for refactor, parity, and release work.

## Refactor Gate

Use this for focused ownership changes inside `merman-core` or `merman-render`:

```sh
cargo fmt
cargo check -p merman-core -p merman-render
cargo clippy -p merman-core -p merman-render --all-targets -- -D warnings
cargo nextest run -p merman-core -p merman-render
```

## Parity Gate

Use this for layout or SVG changes that can affect DOM output:

```sh
cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3
```

Use narrower `compare-*` commands when the change touches only one diagram family.

## Override Gate

Use this when a change touches generated override data, manual raw SVG/path bridges, or text
measurement fallback tables:

```sh
cargo run -p xtask -- report-overrides --check-no-growth
```

The gate fails when any override category grows beyond the explicit budget encoded in
`xtask report-overrides`. Real growth is allowed only when the budget and
`OVERRIDE_FOOTPRINT.md` are updated with reviewable evidence.

When deleting text metric lookups, also prove every consumer path is safe. For layout-affecting
lookups, run the relevant layout snapshot test in addition to the diagram DOM parity commands;
Block labels are not safe to prune solely because the vendored SVG/HTML measurer matches the
stored value.

## Feature Gate

Use this when touching public feature flags or optional render/raster dependencies:

```sh
cargo run -p xtask -- verify --feature-matrix
```

This checks `merman` with no default features, `render`, and `raster`, plus `merman-core` without
its default feature set.

## Performance Gate

Use this when the change is meant to reduce allocations or render time:

```sh
cargo bench -p merman --features render
```

Add targeted Criterion runs when the benchmarked path is small enough to isolate.

## Release Gate

Use this before landing broad cleanup or public-surface changes:

```sh
cargo run -p xtask -- verify --strict
```

This is the release-level superset of the other gates and includes fmt, all-features check, public
feature matrix, workspace clippy, override no-growth, nextest, and SVG DOM parity.
