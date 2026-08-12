# Publishing to crates.io

This workspace is intended to be published as multiple crates (no monorepo submodules).

## Why `cargo package -p merman` fails before the first publish

Crates like `merman-analysis`, `merman-editor-core`, `merman-lsp`, `merman`, and `merman-cli`
depend on other workspace crates (for example `merman-core`, `merman-render`, and
`merman-bindings-core`). When packaging/publishing, Cargo rewrites `*.workspace = true`
dependencies into registry dependencies (version-only). Before the matching release version of those
dependency crates exists on crates.io, `cargo package -p <dependent-crate>` (or
`cargo publish --dry-run -p <dependent-crate>`) will fail.

This is expected. Publish in dependency order.

## Publish checklist

- `cargo fmt`
- `cargo nextest run`
- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored`
- `cargo run -p xtask -- verify-generated`
- Confirm `docs/alignment/STATUS.md` is up to date.
- Bump versions (workspace + crates as needed) and tag the release.

## Derived publish order

Do not maintain a prose copy. Cargo metadata owns the workspace dependency graph and the publish
helper derives the current topological order:

```bash
python3 tools/publish.py --list-crates-io-packages
```

Notes:

- `xtask` is `publish = false` and should not be published.
- Run local package or publish dry-runs only for the currently registry-satisfiable batch. The
  release preflight owns registry-independent dry-runs for the initial batch.
- Normal publication runs through `release-crates.yml`. Its trusted `publish-receipted` operator
  packages an entire topological batch, writes the exact `.crate` receipt, publishes each missing
  version once, and requires matching crates.io checksums before entering the next batch.
- A `pending_recovery` or `mismatch` receipt is a stop condition. Retry only from the same immutable
  source; do not rerun `cargo publish` manually and do not yank automatically.
