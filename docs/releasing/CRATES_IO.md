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

## Recommended publish order

Publish leaf crates first, then the crates that depend on them:

1. `dugong-graphlib`
2. `manatee`
3. `merman-core`
4. `merman-elk-layered`
5. `roughr-merman`
6. `dugong`
7. `merman-analysis`
8. `merman-ascii`
9. `merman-layout-elk`
10. `merman-editor-core`
11. `merman-render`
12. `merman`
13. `merman-lsp`
14. `merman-bindings-core`
15. `merman-cli`
16. `merman-rustdoc`
17. `merman-ffi`
18. `merman-typst-plugin`
19. `merman-uniffi`
20. `merman-wasm`

Example:

```bash
cargo publish -p dugong-graphlib
cargo publish -p manatee
cargo publish -p merman-core
cargo publish -p merman-elk-layered
cargo publish -p roughr-merman
cargo publish -p dugong
cargo publish -p merman-analysis
cargo publish -p merman-ascii
cargo publish -p merman-layout-elk
cargo publish -p merman-editor-core
cargo publish -p merman-render
cargo publish -p merman
cargo publish -p merman-lsp
cargo publish -p merman-bindings-core
cargo publish -p merman-cli
cargo publish -p merman-rustdoc
cargo publish -p merman-ffi
cargo publish -p merman-typst-plugin
cargo publish -p merman-uniffi
cargo publish -p merman-wasm
```

Notes:

- `xtask` is `publish = false` and should not be published.
- If you prefer to validate without publishing, run `cargo publish --dry-run -p <crate>` in the
  same order.
