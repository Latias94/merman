# merman Python Package

Experimental Python bindings for Merman through UniFFI.

Merman renders Mermaid diagrams without a browser. It can parse Mermaid source, return semantic
JSON, compute layout JSON, and render SVG through a headless Rust engine. See the
[project README](https://github.com/Latias94/merman),
[Python binding notes](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md),
and [diagram coverage status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
for the main library contract.

## API

```python
import merman

engine = merman.MermanEngine()
assert engine.abi_version() == 1
print(engine.package_version())

source = "flowchart TD\nA[Hello] --> B[World]"
svg = engine.render_svg(source, None)
ascii_text = engine.render_ascii(source, None)
semantic_json = engine.parse_json(source, None)
layout_json = engine.layout_json(source, None)
validation = engine.validate(source, None)
diagrams = engine.supported_diagrams()
themes = engine.themes()

try:
    engine.render_svg(source, "{")
except merman.MermanError.Binding as error:
    print(error.code_name, error.message)
```

`options_json` is optional. Pass `None` for defaults, or a JSON string with `parse`, `layout`, and
`svg` options. The shared schema is documented in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).

## Generate Locally

This directory intentionally does not commit generated binding source or native libraries. Generate
them from a local `merman-uniffi` cdylib:

```bash
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- \
  --package-dir platforms/python/merman
```

The generator writes:

- `src/merman/merman_uniffi.py`
- `src/merman/merman_uniffi.dll` on Windows
- `src/merman/libmerman_uniffi.so` on Linux
- `src/merman/libmerman_uniffi.dylib` on macOS

The native library must sit beside the generated module because UniFFI's Python loader resolves the
library relative to the generated file.

After generation, a local smoke can import the package by putting `src` on `PYTHONPATH`:

```bash
PYTHONPATH=platforms/python/merman/src python -c "import merman; print(merman.MermanEngine().render_svg('flowchart TD\nA[Hello]', None)[:4])"
```

Or run the example script:

```bash
PYTHONPATH=platforms/python/merman/src python platforms/python/merman/examples/smoke.py
```

Build a local platform wheel and run an install smoke:

```bash
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

The wheel is platform-specific because it bundles `merman-uniffi` as a native `.so`, `.dylib`, or
`.dll`. Tag releases run `release-python.yml`, attach platform wheels to the GitHub Release, and
publish the `merman` distribution to PyPI when Trusted Publishing is configured.

## License

This package is dual-licensed under either Apache-2.0 or MIT. See `LICENSE` for the full license
texts. Mermaid compatibility and upstream Mermaid MIT attribution are documented in
[`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
