# merman Python Package

Experimental Python bindings for Merman through UniFFI.

Merman renders Mermaid diagrams without a browser. It can parse Mermaid source, return semantic
JSON, compute layout JSON, and render SVG through a headless Rust engine. See the
[project README](https://github.com/Latias94/merman),
[Python binding notes](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md),
and [diagram coverage status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
for the main library contract.

## Compatibility And Release Notes

This package tracks UniFFI ABI 2 and is regenerated from the `merman-uniffi` cdylib. The
published PyPI page shows this README together with the metadata links in `pyproject.toml`, so the
package page can point directly to the binding docs, issues, and changelog.

`MermanReusableEngine` exposes the reusable render path, and `MermanTextMeasurer` lets Python
hosts provide a callback when they need host-owned text measurement. `ascii_capabilities()` reports
ASCII support grades and summary fallback metadata; `diagram_family_capabilities()` reports
parser/render family availability. `analyze_document_json()` and
`analyze_document_facts_json()` expose Markdown/MDX-aware diagnostics and facts.

For package-specific release notes, see [`CHANGELOG.md`](CHANGELOG.md).

## API

```python
import merman

engine = merman.MermanEngine()
assert engine.abi_version() == 2
print(engine.package_version())

source = "flowchart TD\nA[Hello] --> B[World]"
svg = engine.render_svg(source, None)
ascii_text = engine.render_ascii(source, None)
semantic_json = engine.parse_json(source, None)
layout_json = engine.layout_json(source, None)
validation = engine.validate(source, None)
document_json = engine.analyze_document_json("```mermaid\n" + source + "\n```", None, "file:///tmp/example.md")
document_facts_json = engine.analyze_document_facts_json(
    "```mermaid\n" + source + "\n```",
    None,
    "file:///tmp/example.md",
)
diagrams = engine.supported_diagrams()
ascii_capabilities = engine.ascii_capabilities()
themes = engine.supported_themes()
host_presets = engine.supported_host_theme_presets()
family_capabilities = engine.diagram_family_capabilities()

class Measurer(merman.MermanTextMeasurer):
    def measure(self, request):
        return merman.MermanTextMeasureResult(
            width=max(len(request.text) * 8.0, 1.0),
            height=max(request.line_height, 1.0),
            line_count=1,
        )

reusable = engine.reusable_engine_with_text_measurer(None, Measurer())
assert "Hello" in reusable.render_svg(source)

reusable = engine.reusable_engine(None)
document_json = reusable.analyze_document_json(
    "```mermaid\n" + source + "\n```",
    "file:///tmp/example.md",
)
reusable.set_text_measurer(Measurer())
assert "Hello" in reusable.render_svg(source)
reusable.clear_text_measurer()

try:
    engine.render_svg(source, "{")
except merman.MermanError.Binding as error:
    print(error.code_name, error.message)
```

`options_json` is optional. Pass `None` for defaults, or a JSON string with `parse`, `layout`, and
`svg` options. The shared schema is documented in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).

## Text Measurement

The current Python package is generated through UniFFI and uses merman's built-in headless text
measurer by default. This is suitable for CLI tools, documentation builds, tests, and server-side
batch rendering.

If a Python GUI, browser automation host, or WebView application needs geometry that matches its
own font stack, create a `MermanReusableEngine` with `reusable_engine_with_text_measurer(...)` or
call `set_text_measurer(...)` on an existing reusable engine. Call `clear_text_measurer()` to
restore the engine's original built-in measurer. Return `None` from the callback when a request is
not handled so merman can fall back to its vendored metrics for that request. See
[`docs/bindings/HOST_TEXT_MEASUREMENT.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md).

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
