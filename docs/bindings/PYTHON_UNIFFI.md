# Python UniFFI Bindings

Status: experimental publishable Python package.

The Python binding is generated from the `merman-uniffi` cdylib with UniFFI. The package shape is:

```text
platforms/python/merman/
  pyproject.toml
  src/merman/
    __init__.py
    merman_uniffi.py            generated, not committed
    merman_uniffi.dll           generated/copy on Windows, not committed
    libmerman_uniffi.so         generated/copy on Linux, not committed
    libmerman_uniffi.dylib      generated/copy on macOS, not committed
```

The generated Python module and native library must live in the same package directory because the
UniFFI Python loader resolves the cdylib relative to `__file__`.

Merman itself is a browserless Rust engine for Mermaid diagrams. Start from the
[project README](https://github.com/Latias94/merman) for product scope, the
[UniFFI binding notes](https://github.com/Latias94/merman/blob/main/docs/bindings/UNIFFI.md) for
the shared wrapper layer, and
[diagram coverage status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
for current Mermaid parity.

## Generate Locally

```bash
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- \
  --package-dir platforms/python/merman
```

On Windows PowerShell, use the same command on one line:

```powershell
cargo build -p merman-uniffi --features bindgen-smoke
cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- --package-dir platforms/python/merman
```

## API

The package re-exports the generated UniFFI API:

```python
import merman

engine = merman.MermanEngine()
assert engine.abi_version() == 2
print(engine.package_version())

svg = engine.render_svg("flowchart TD\nA[Hello] --> B[World]", None)
ascii_text = engine.render_ascii("flowchart TD\nA[Hello] --> B[World]", None)
semantic_json = engine.parse_json("flowchart TD\nA[Hello] --> B[World]", None)
layout_json = engine.layout_json("flowchart TD\nA[Hello] --> B[World]", None)
validation = engine.validate("flowchart TD\nA[Hello] --> B[World]", None)
diagrams = engine.supported_diagrams()
```

Errors are exposed through the generated `MermanError` type. The underlying status code, status
name, and message still come from `merman-bindings-core`.
The optional `options_json` argument uses the shared contract documented in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).

## Verification

```bash
cargo check -p merman-uniffi --features bindgen-smoke --examples
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

The nextest smoke stages a temporary package, generates `merman_uniffi.py`, copies the cdylib next to
it, imports `merman` with Python, then calls `MermanEngine.render_svg`,
`MermanEngine.render_ascii`, `MermanEngine.parse_json`, `MermanEngine.layout_json`,
`MermanEngine.validate`, metadata methods, `MermanEngine.abi_version`,
`MermanEngine.package_version`, and checks `MermanError.Binding` fields for invalid options JSON.

## Build A Local Wheel

```bash
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

The script builds `merman-uniffi`, stages generated UniFFI Python files into
`platforms/python/merman`, builds a platform wheel under `target/python-wheels`, then
optionally installs it into a temporary venv and exercises SVG, ASCII, parse, layout, validation,
and metadata calls. The build script fails if setuptools emits a universal `py3-none-any` wheel,
because the package carries a native library.

## Release

`release-python.yml` runs on `v*` tags, builds and smokes wheels on Linux, macOS, and Windows,
repairs the Linux wheel with `auditwheel`, checks wheel metadata with `twine`, attaches wheels to
the GitHub Release, and publishes to PyPI through Trusted Publishing.

Configure the PyPI project `merman` with a Trusted Publisher for this repository and
`.github/workflows/release-python.yml`. No PyPI API token is required for the OIDC path.

## Example

After generation, run:

```bash
PYTHONPATH=platforms/python/merman/src python platforms/python/merman/examples/smoke.py
```

## Not Yet Done

- Broader architecture matrix beyond the default GitHub hosted runner architecture for each OS.
- macOS universal2 wheel assembly.
- Windows wheel signing or installer metadata.
