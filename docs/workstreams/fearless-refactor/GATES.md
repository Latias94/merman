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

Use this when root `viewBox` / `max-width` / export bounds can change:

```sh
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

Use narrower `compare-*` commands when the change touches only one diagram family.

## Fixture-Independence Gate

Fixtures and upstream SVGs verify production behavior; they must not provide production answers.
The architecture guard rejects generated root/text tables, complete-label metric lookups, direct
family root mutation, and the removed override-generation commands. When text or root behavior
changes, run the affected family tests plus normal and `parity-root` comparison modes.

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
feature matrix, workspace clippy, nextest, SVG DOM parity, and full SVG root parity.
