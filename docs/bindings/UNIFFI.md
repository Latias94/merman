# UniFFI Bindings

Status: experimental generated-binding surface.

`merman-uniffi` exposes a small UniFFI object API over `merman-bindings-core`:

- `MermanEngine::new()`
- `MermanEngine::render_svg(source, options_json)`
- `MermanEngine::parse_json(source, options_json)`
- `MermanEngine::layout_json(source, options_json)`
- `MermanError::Binding { code, code_name, message }`

The C ABI in `merman-ffi` remains the canonical low-level protocol. UniFFI is a convenience layer for
Swift, Kotlin, Python, and Ruby package lanes.

## Bindgen Smoke

Run:

```bash
cargo test -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

This test builds the `merman-uniffi` cdylib, generates Python bindings from the embedded UniFFI
metadata into a temporary directory, and asserts that the generated source exposes `MermanEngine`,
`render_svg`, and `MermanError`.

Generated Swift, Kotlin, Python, or Ruby files are not committed by this lane. Platform-specific
package layouts should be split into follow-on lanes.
