# UniFFI Bindings

Status: experimental generated-binding surface.

`merman-uniffi` exposes a small UniFFI object API over `merman-bindings-core`:

- `MermanEngine::new()`
- `MermanEngine::abi_version()`
- `MermanEngine::package_version()`
- `MermanEngine::render_svg(source, options_json)`
- `MermanEngine::parse_json(source, options_json)`
- `MermanEngine::layout_json(source, options_json)`
- `MermanError::Binding { code, code_name, message }`

The C ABI in `merman-ffi` remains the canonical low-level protocol. UniFFI is a convenience layer for
Swift, Kotlin, Python, and Ruby package lanes.
The optional `options_json` argument uses the shared contract documented in
`docs/bindings/OPTIONS_JSON.md`.

## Bindgen Smoke

Run:

```bash
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

This test builds the `merman-uniffi` cdylib, generates Python bindings from the embedded UniFFI
metadata into a temporary package, copies the native library beside the generated module, imports
the package with Python, and calls `MermanEngine.abi_version`, `MermanEngine.package_version`,
`MermanEngine.render_svg`, `MermanEngine.parse_json`, `MermanEngine.layout_json`, plus a
`MermanError.Binding` error-path check.

Generated Swift, Kotlin, Python, or Ruby files are not committed by this lane. Platform-specific
package layouts should be split into follow-on lanes.

## Other UniFFI Targets

UniFFI 0.31.1 can generate Kotlin, Python, Ruby, and Swift bindings. Merman currently ships only the
Python package scaffold through UniFFI. Android/Kotlin and Apple/Swift already have C ABI wrappers
under `platforms/android` and `platforms/apple`, so generated UniFFI Kotlin or Swift should be a
deliberate follow-on if the generated API is meant to replace or supplement those wrappers. Ruby is
not currently a release surface.

## Python Package Scaffold

See `docs/bindings/PYTHON_UNIFFI.md` for the current Python package layout and local generation
command. The package scaffold lives under `platforms/python/merman`; generated Python source
and native libraries are ignored.
