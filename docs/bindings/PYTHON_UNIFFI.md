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

`scripts/build-python-uniffi-wheel.py` resolves the `python-uniffi-native` artifact profile. It
builds the release cdylib with the complete direct feature list, then enables `bindgen-smoke` in
the separate generator process that consumes that production library. The builder rejects hosts
outside the profile's published target set and replaces the package scaffold's release-set legal
report with the checked-in single-target report before building the wheel.

```bash
cargo build -p merman-uniffi --release --no-default-features \
  --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
cargo run -p merman-uniffi --no-default-features \
  --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --example generate_python_package -- \
  --cdylib target/release/libmerman_uniffi.dylib \
  --package-dir platforms/python/merman
```

On Windows PowerShell, use the same command on one line:

```powershell
cargo build -p merman-uniffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --example generate_python_package -- --cdylib target/release/merman_uniffi.dll --package-dir platforms/python/merman
```

## API

The package re-exports the generated UniFFI API:

```python
import json
import merman

api = merman.Merman()
merman.require_text_measurement_protocol_version(
    merman.TEXT_MEASUREMENT_PROTOCOL_VERSION
)
print(api.package_version())
assert api.binding_api_version() == 3
catalog = merman.get_runtime_catalog(api)
capabilities = catalog["capabilities"]
assert catalog["schema_version"] == 1
assert catalog["transport_api_version"] == api.binding_api_version()
assert "svg" in capabilities["capability_ids"]

svg = api.render_svg("flowchart TD\nA[Hello] --> B[World]", None)
png = api.render_png("flowchart TD\nA[Hello] --> B[World]", None)
jpeg = api.render_jpeg("flowchart TD\nA[Hello] --> B[World]", None)
pdf = api.render_pdf("flowchart TD\nA[Hello] --> B[World]", None)
ascii_text = api.render_ascii("flowchart TD\nA[Hello] --> B[World]", None)
semantic_json = api.parse_json("flowchart TD\nA[Hello] --> B[World]", None)
layout_json = api.layout_json("flowchart TD\nA[Hello] --> B[World]", None)
document_json = api.analyze_document_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    "file:///tmp/example.md",
    None,
)
document_facts_json = api.analyze_document_facts_json(
    "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
    "file:///tmp/example.md",
    None,
)
validation = api.validate("flowchart TD\nA[Hello] --> B[World]", None)
diagrams = api.supported_diagrams()
ascii_capabilities = api.ascii_capabilities()
themes = api.supported_themes()
presentation_catalog = json.loads(api.presentation_catalog_json())
family_capabilities = api.diagram_family_capabilities()
lint_rules = api.lint_rule_catalog()

class PreviewMeasurer(merman.MermanTextMeasurer):
    def measure(self, request):
        # Use the final display surface's font API here. Returning None asks
        # Merman to use its operation-specific vendored fallback.
        return None

services = merman.MermanEngineServices(None, PreviewMeasurer())
engine = merman.MermanEngine(None, services)
try:
    svg_with_host_metrics = engine.render_svg(
        "flowchart TD\nA[Hello] --> B[World]",
        None,
    )
    reusable_document_json = engine.analyze_document_json(
        "```mermaid\nflowchart TD\nA[Hello] --> B[World]\n```",
        "file:///tmp/example.md",
        None,
    )
finally:
    engine.close()
```

Errors are exposed through the generated `MermanError` type. `MermanError.Binding` carries the
underlying status code/name, `MermanErrorKind`, optional `capability_id`, optional
`MermanResourceErrorDetails`, and message from `merman-bindings-core`. `UNKNOWN_OPERATION` has no
capability ID; `MISSING_CAPABILITY` preserves the exact descriptor ID. Resource failures expose the
stable limit ID, phase, actual value, effective maximum, and selected profile. Consumers should not
parse the message to distinguish these cases.
The optional `options_json` argument uses the shared contract documented in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).
`ResourceOptionsBuilder` emits Options JSON schema `2`; omit its profile for a reusable request that must inherit the constructor ceiling, and use `ResourceOverrideId` rather than the full catalog-only `ResourceLimitId` when adding overrides.
`Merman.lint_rule_catalog()` returns structured analyzer rule metadata, including evidence
references, for editor settings, diagnostic explanations, or LSP rule configuration.

The direct UniFFI binding API is `3`, independently versioned from the native C ABI and the
text-measurement protocol. `get_runtime_catalog()` reads one atomic catalog, validates
flat schema `1`, artifact identity, sorted stable IDs, and local output/operation and
adapter/capability relations before returning it. Do not infer availability from Cargo feature
names or copy an ID table into Python; inspect the loaded catalog instead.

## Text Measurement

The Python UniFFI package uses Merman's built-in headless text measurer by default. This is the
right default for CLI tools, documentation generation, tests, and server-side batch rendering
because it is deterministic and does not require GUI or browser dependencies.

Python GUI or WebView hosts that need label geometry to match their own font stack place a
`MermanTextMeasurer` in `MermanEngineServices` and pass that value to the direct
`MermanEngine(options_json, services)` constructor. The callback is immutable for that engine;
construct another engine to change it or return to the built-in measurer. Inspect `request.phase`
for the routing stage and `request.operation` for the exact platform primitive. Return a record
tagged with the matching
`MermanTextMeasurementResultKind`: metrics, length, horizontal extents, or wrapped metrics with raw
width. `None`, wrong-kind or invalid results, and Python exceptions reported through UniFFI's
generated callback trampoline use the operation's vendored fallback for that request instead of
failing the reusable render/layout call. Merman does not catch arbitrary foreign unwinds that
bypass that generated boundary. Follow
[`HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md) for the
shared callback rules around caching, natural width, and avoiding async UI-thread blocking.
Text-measurement protocol 1 exposes 19 operations with contiguous codes 0 through 18.
`CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET` returns the signed `length` for Architecture's
`createFormattedText(...)` bbox y under inherited `dominant-baseline="middle"`.
`CREATE_TEXT_B_BOX_Y_OFFSET` remains the ordinary createText probe; the two operations are not
interchangeable and both may return a finite negative value. `RAW_B_BOX_HEIGHT` returns the
non-negative height from a direct raw SVG `<text>.getBBox()` probe.

The generated callback method is `measure(self, request)`, not `measure_text`. One-shot facade
methods receive `options_json` explicitly (`api.render_svg(source, options_json)`). Generic
operations put the same value in `MermanOperationRequest.options_json` and call
`api.execute(request)`. A reusable engine accepts baseline options at construction and
request-local overrides on each operation, such as `engine.render_svg(source, options_json)`;
those overrides are deeply merged without changing the baseline or its runtime policy. Do not
estimate width from character count in production; return `None` when the host cannot reproduce
the requested operation with the display surface's real font API.

Callback-free reusable engines admit concurrent calls. A callback engine serializes operation
admission and raises typed `BUSY` for a competing call. Same-engine entry while the callback is
active raises typed `REENTRANT_CALL`, including an attempted call dispatched from the callback to
another thread. A close from the callback also raises `REENTRANT_CALL` and preserves the complete
engine for retry. `close()` is explicit and idempotent; always call it for a callback-owning engine
because the callback may capture the engine and form a reference cycle. There is no callback
mutation lifecycle.

## Migrating From The Previous Prerelease API

- Replace the old one-shot `MermanEngine()` with `Merman()`.
- Delete `MermanReusableEngine`, `reusable_engine(...)`, and
  `reusable_engine_with_text_measurer(...)` usage. Construct `MermanEngine(options_json, services)`
  directly.
- Put text measurement and immutable icon registries in `MermanEngineServices`.
- Call `close()` deterministically; busy and re-entrant failures retain the engine for retry.
- Use the result-returning binary methods when callers need typed operation metadata or the
  effective output plan.
- Preserve unknown runtime discovery IDs instead of treating generated known constants as a closed
  enum.

## Verification

```bash
cargo check -p merman-uniffi --no-default-features \
  --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --examples
cargo nextest run -p merman-uniffi --no-default-features \
  --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --test bindgen_smoke
```

The nextest smoke stages a temporary package, generates `merman_uniffi.py`, copies the cdylib next to
it, and runs the repository's canonical
[`platforms/python/merman/examples/smoke.py`](../../platforms/python/merman/examples/smoke.py)
against that fresh package. The executable example calls the `Merman` one-shot, metadata, and
runtime-catalog methods; constructs `MermanEngine(options_json, services)` directly; exercises
generic and named operations, binary result metadata, icon registry reuse, callback-backed
execution, deterministic close, and post-close failure; and checks
`MermanError.Binding` fields for invalid options JSON.
The generated-package smoke also asserts direct UniFFI binding API 3, flat runtime catalog schema 1,
local runtime ID relations, SVG/PNG/JPEG/PDF output signatures, all 19 operation
variants, a distinct signed
`CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET` callback result, and the `RAW_B_BOX_HEIGHT` variant.

## Build A Local Wheel

```bash
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

The script builds `merman-uniffi`, copies the checked-in Python package scaffold into a temporary
staging directory, generates UniFFI Python files only in that staging directory, and builds a
platform wheel under `target/python-wheels`. With `--run-smoke`, it installs the wheel into a
temporary venv and exercises SVG, ASCII, parse, layout, validation, metadata, PNG, JPEG, and PDF
calls through the installed package. The build fails when generated support files differ from
their checked-in projections or when setuptools emits a universal `py3-none-any` wheel, because
the package carries a native library.

## Release

`release-python.yml` is a manual release workflow that accepts a `v*` release tag, resolves and
verifies the matching immutable tag commit and tree, builds and smokes wheels on Linux, macOS, and
Windows, repairs the Linux wheel with `auditwheel`, checks wheel metadata with `twine`, attaches
wheels to the GitHub Release, and publishes to PyPI through Trusted Publishing. Dispatching an
updated workflow definition from `main` never changes the artifact source.

Configure the PyPI project `merman` with a Trusted Publisher for this repository and
`.github/workflows/release-python.yml`. No PyPI API token is required for the OIDC path.

## Example

Build and exercise the installed wheel:

```bash
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

## Not Yet Done

- Broader architecture matrix beyond the default GitHub hosted runner architecture for each OS.
- macOS universal2 wheel assembly.
- Windows wheel signing or installer metadata.
