# merman Python Package

Experimental Python package scaffold for UniFFI-generated merman bindings.

## API

```python
import merman

engine = merman.MermanEngine()
assert engine.abi_version() == 1
print(engine.package_version())

source = "flowchart TD\nA[Hello] --> B[World]"
svg = engine.render_svg(source, None)
semantic_json = engine.parse_json(source, None)
layout_json = engine.layout_json(source, None)

try:
    engine.render_svg(source, "{")
except merman.MermanError.Binding as error:
    print(error.code_name, error.message)
```

`options_json` is optional. Pass `None` for defaults, or a JSON string with `parse`, `layout`, and
`svg` options.

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

PyPI publishing is follow-on work. This scaffold is the package staging shape used by the Rust smoke
tests, local wheel checks, and the `release-python.yml` wheel artifact workflow.
