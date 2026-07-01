# merman-typst-plugin

`merman-typst-plugin` is the experimental Typst WebAssembly plugin bridge for
`merman`.

The crate exports Typst-compatible `wasm-minimal-protocol` functions and delegates
rendering to `merman-bindings-core`.

Current exported functions:

- `abi_version() -> bytes`
- `package_version() -> bytes`
- `capabilities_json() -> bytes`
- `render_svg_json(source: bytes, options_json: bytes) -> bytes`
- `validate_json(source: bytes, options_json: bytes) -> bytes`

`render_svg_json` returns a stable JSON payload with `ok`, `code`,
`code_name`, `message`, and `svg` fields so the Typst package can render
placeholder or text errors without failing compilation.

## ABI Boundary

Current Typst plugin ABI version: `1`.

The plugin ABI covers the exported `wasm-minimal-protocol` function names and
their byte payload contracts. That includes the `abi_version`,
`package_version`, `capabilities_json`, `render_svg_json`, and `validate_json`
exports, plus the JSON schemas consumed or produced by those exports.

Typst wrapper API changes in `packages/typst/merman/src/*.typ` do not require a
plugin ABI bump when this WebAssembly surface remains stable. Bump
`TYPST_PLUGIN_ABI_VERSION` and update the ABI tests and package README mapping
when an export is added, removed, renamed, changes argument or return bytes, or
changes the render, validate, or capabilities JSON contract.

The `package_version` export reports the Rust crate version. The Typst package
version in `packages/typst/merman/typst.toml` is a separate `@preview` wrapper
version.

Build the default Typst render artifact with:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
```

Build the bridge-only protocol artifact with:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown --no-default-features
```

Build the larger full-config/full-sanitization no-host artifact with:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown --features core-full
```

Then check the Typst wasm surface with:

```bash
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```

Smoke the plugin through a Typst-compatible `wasmi` host call with:

```bash
cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```
