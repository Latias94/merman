# merman-typst-plugin

`merman-typst-plugin` is the experimental Typst WebAssembly plugin bridge for
`merman`.

The crate exports Typst-compatible `wasm-minimal-protocol` functions and delegates
rendering to `merman-bindings-core`.

Current exported functions:

- `abi_version() -> bytes`
- `package_version() -> bytes`
- `render_svg_json(source: bytes, options_json: bytes) -> bytes`
- `validate_json(source: bytes, options_json: bytes) -> bytes`

`render_svg_json` returns a stable JSON payload with `ok`, `code`,
`code_name`, `message`, and `svg` fields so the Typst package can render
placeholder or text errors without failing compilation.

Build the default minimal Typst probe artifact with:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
```

Build the larger full-config/full-sanitization no-host artifact with:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown --features core-full
```

Then check the Typst wasm surface with:

```bash
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```
