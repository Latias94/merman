# Python UniFFI Bindings

Status: experimental local package scaffold.

The Python binding is generated from the `merman-uniffi` cdylib with UniFFI. The package shape is:

```text
bindings/python/merman-uniffi/
  pyproject.toml
  src/merman_uniffi/
    __init__.py
    merman_uniffi.py            generated, not committed
    merman_uniffi.dll           generated/copy on Windows, not committed
    libmerman_uniffi.so         generated/copy on Linux, not committed
    libmerman_uniffi.dylib      generated/copy on macOS, not committed
```

The generated Python module and native library must live in the same package directory because the
UniFFI Python loader resolves the cdylib relative to `__file__`.

## Generate Locally

```bash
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- \
  --package-dir bindings/python/merman-uniffi
```

On Windows PowerShell, use the same command on one line:

```powershell
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- --package-dir bindings/python/merman-uniffi
```

## API

The package re-exports the generated UniFFI API:

```python
import merman_uniffi

engine = merman_uniffi.MermanEngine()
svg = engine.render_svg("flowchart TD\nA[Hello] --> B[World]", None)
semantic_json = engine.parse_json("flowchart TD\nA[Hello] --> B[World]", None)
layout_json = engine.layout_json("flowchart TD\nA[Hello] --> B[World]", None)
```

Errors are exposed through the generated `MermanError` type. The underlying status code, status
name, and message still come from `merman-bindings-core`.

## Verification

```bash
cargo check -p merman-uniffi --features bindgen-smoke --examples
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

The nextest smoke stages a temporary package, generates `merman_uniffi.py`, copies the cdylib next to
it, imports `merman_uniffi` with Python, then calls `MermanEngine.render_svg` and
`MermanEngine.parse_json`.

## Not Yet Done

- PyPI publishing.
- Platform wheel matrix.
- manylinux/musllinux policy.
- macOS universal2 wheel assembly.
- Windows wheel signing or installer metadata.
