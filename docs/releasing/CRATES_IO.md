# Publishing to crates.io

This workspace is intended to be published as multiple crates (no monorepo submodules).

## Why `cargo package -p merman` fails before the first publish

Crates like `merman` and `merman-cli` depend on other workspace crates (e.g. `merman-core`,
`merman-render`). When packaging/publishing, Cargo rewrites `*.workspace = true` dependencies into
registry dependencies (version-only). Before the first release, those dependency crates do not yet
exist on crates.io, so `cargo package -p merman` (or `cargo publish --dry-run -p merman`) will fail.

This is expected. Publish in dependency order.

## Publish checklist

- `cargo fmt`
- `cargo nextest run`
- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored`
- `cargo run -p xtask -- verify-generated`
- Confirm `docs/alignment/STATUS.md` is up to date.
- Bump versions (workspace + crates as needed) and tag the release.

## Recommended publish order

Publish leaf crates first, then the crates that depend on them:

1. `dugong-graphlib`
2. `dugong`
3. `manatee`
4. `merman-core`
5. `merman-render`
6. `merman`
7. `merman-cli`

Example:

```bash
cargo publish -p dugong-graphlib
cargo publish -p dugong
cargo publish -p manatee
cargo publish -p merman-core
cargo publish -p merman-render
cargo publish -p merman
cargo publish -p merman-cli
```

Notes:

- `xtask` is `publish = false` and should not be published.
- If you prefer to validate without publishing, run `cargo publish --dry-run -p <crate>` in the
  same order.

