# merman-uniffi Python Package

Experimental Python package scaffold for UniFFI-generated merman bindings.

This directory intentionally does not commit generated binding source or native libraries. Generate
them from a local `merman-uniffi` cdylib:

```bash
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- \
  --package-dir bindings/python/merman-uniffi
```

The generator writes:

- `src/merman_uniffi/merman_uniffi.py`
- `src/merman_uniffi/merman_uniffi.dll` on Windows
- `src/merman_uniffi/libmerman_uniffi.so` on Linux
- `src/merman_uniffi/libmerman_uniffi.dylib` on macOS

The native library must sit beside the generated module because UniFFI's Python loader resolves the
library relative to the generated file.

After generation, a local smoke can import the package by putting `src` on `PYTHONPATH`:

```bash
PYTHONPATH=bindings/python/merman-uniffi/src python -c "import merman_uniffi; print(merman_uniffi.MermanEngine().render_svg('flowchart TD\nA[Hello]', None)[:4])"
```

Build a local platform wheel and run an install smoke:

```bash
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

PyPI publishing is follow-on work; this scaffold is the package staging shape used by the Rust smoke
tests and local wheel checks.
