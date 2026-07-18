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
merman.require_abi_version(engine.abi_version())
print(engine.package_version())

svg = engine.render_svg("flowchart TD\nA[Hello] --> B[World]", None)
ascii_text = engine.render_ascii("flowchart TD\nA[Hello] --> B[World]", None)
semantic_json = engine.parse_json("flowchart TD\nA[Hello] --> B[World]", None)
layout_json = engine.layout_json("flowchart TD\nA[Hello] --> B[World]", None)
document_json = engine.analyze_document_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    None,
    "file:///tmp/example.md",
)
document_facts_json = engine.analyze_document_facts_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    None,
    "file:///tmp/example.md",
)
validation = engine.validate("flowchart TD\nA[Hello] --> B[World]", None)
diagrams = engine.supported_diagrams()
ascii_capabilities = engine.ascii_capabilities()
themes = engine.supported_themes()
host_presets = engine.supported_host_theme_presets()
family_capabilities = engine.diagram_family_capabilities()
lint_rules = engine.lint_rule_catalog()

class Measurer(merman.MermanTextMeasurer):
    def measure(self, request):
        print(request.phase)
        width = max(len(request.text) * 8.0, 1.0)
        if request.operation == merman.MermanTextMeasurementOperation.CANVAS_MEASURE_TEXT_WIDTH:
            return merman.MermanTextMeasureResult(
                result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
                width=0.0,
                height=0.0,
                length=width,
                line_count=0,
                bbox_left=None,
                bbox_right=None,
                raw_width=None,
            )
        if request.operation not in {
            merman.MermanTextMeasurementOperation.MEASURE,
            merman.MermanTextMeasurementOperation.WRAPPED,
            merman.MermanTextMeasurementOperation.MERMAID_CALCULATE_TEXT_DIMENSIONS,
        }:
            return None
        return merman.MermanTextMeasureResult(
            result_kind=merman.MermanTextMeasurementResultKind.METRICS,
            width=width,
            height=max(request.line_height, 1.0),
            length=0.0,
            line_count=1,
            bbox_left=None,
            bbox_right=None,
            raw_width=None,
        )

reusable = engine.reusable_engine_with_text_measurer(None, Measurer())
svg_with_host_metrics = reusable.render_svg("flowchart TD\nA[Hello] --> B[World]")

reusable = engine.reusable_engine(None)
reusable_document_json = reusable.analyze_document_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    "file:///tmp/example.md",
)
reusable.set_text_measurer(Measurer())
svg_with_host_metrics = reusable.render_svg("flowchart TD\nA[Hello] --> B[World]")
reusable.clear_text_measurer()
```

Errors are exposed through the generated `MermanError` type. The underlying status code, status
name, and message still come from `merman-bindings-core`.
The optional `options_json` argument uses the shared contract documented in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).
`MermanEngine.lint_rule_catalog()` returns structured analyzer rule metadata, including evidence
references, for editor settings, diagnostic explanations, or LSP rule configuration.

## Text Measurement

The Python UniFFI package uses Merman's built-in headless text measurer by default. This is the
right default for CLI tools, documentation generation, tests, and server-side batch rendering
because it is deterministic and does not require GUI or browser dependencies.

Python GUI or WebView hosts that need label geometry to match their own font stack can use
`MermanEngine.reusable_engine_with_text_measurer` when constructing a reusable engine, or call
`MermanReusableEngine.set_text_measurer` on an existing reusable engine. Use
`MermanReusableEngine.clear_text_measurer` to restore the engine's original built-in measurer.
Inspect `request.phase` for the routing stage and `request.operation` for the exact platform
primitive. Return a record tagged with the matching `MermanTextMeasurementResultKind`: metrics,
length, horizontal extents, or wrapped metrics with raw width. `None`, wrong-kind or invalid
results, and callback exceptions use the operation's vendored fallback for that request instead of
failing the reusable render/layout call. Follow
[`HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md) for the
shared callback rules around caching, natural width, and avoiding async UI-thread blocking.
ABI 2 exposes 19 text-measurement operations with contiguous codes 0 through 18.
`CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET` returns the signed `length` for Architecture's
`createFormattedText(...)` bbox y under inherited `dominant-baseline="middle"`.
`CREATE_TEXT_B_BOX_Y_OFFSET` remains the ordinary createText probe; the two operations are not
interchangeable and both may return a finite negative value. `RAW_B_BOX_HEIGHT` returns the
non-negative height from a direct raw SVG `<text>.getBBox()` probe.

## Verification

```bash
cargo check -p merman-uniffi --features bindgen-smoke --examples
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

The nextest smoke stages a temporary package, generates `merman_uniffi.py`, copies the cdylib next to
it, imports `merman` with Python, then calls `MermanEngine.render_svg`,
`MermanEngine.render_ascii`, `MermanEngine.parse_json`, `MermanEngine.layout_json`,
`MermanEngine.analyze_document_json`, `MermanEngine.analyze_document_facts_json`,
`MermanEngine.validate`, metadata methods, `MermanEngine.ascii_capabilities`,
`MermanEngine.diagram_family_capabilities`,
`MermanEngine.lint_rule_catalog`, `MermanEngine.configurable_lint_rule_catalog`,
`MermanEngine.reusable_engine_with_text_measurer`,
`MermanReusableEngine.analyze_document_json`,
`MermanReusableEngine.analyze_document_facts_json`, `MermanReusableEngine.set_text_measurer`,
`MermanReusableEngine.clear_text_measurer`, `MermanEngine.abi_version`, `MermanEngine.package_version`,
and checks `MermanError.Binding` fields for invalid options JSON. The generated-package smoke also
asserts ABI 2, all 19 operation variants, a distinct signed
`CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET` callback result, and the `RAW_B_BOX_HEIGHT` variant.

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

`release-python.yml` is a manual release workflow that accepts a `v*` release tag and source ref,
builds and smokes wheels on Linux, macOS, and Windows, repairs the Linux wheel with `auditwheel`,
checks wheel metadata with `twine`, attaches wheels to the GitHub Release, and publishes to PyPI
through Trusted Publishing.

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
