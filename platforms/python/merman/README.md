# Merman For Python

[![PyPI](https://img.shields.io/pypi/v/merman)](https://pypi.org/project/merman/)
[![Python](https://img.shields.io/pypi/pyversions/merman)](https://pypi.org/project/merman/)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Parse, analyze, lay out, and render Mermaid diagrams from Python without a browser or JavaScript runtime. The package ships Merman's Rust engine and exposes it through UniFFI.

> **Alpha:** Python and native APIs may break before the stable release. The package currently targets direct UniFFI binding API `3`, which is independent from the C ABI and text-measurement protocol. Install the Python wheel and native library as one artifact rather than mixing releases. The source-tree README describes `Unreleased`, while each PyPI artifact preserves the documentation for that published version.

## Install

Install the current prerelease from PyPI:

```sh
python -m pip install --pre merman
```

Published wheels currently target CPython-compatible Python `3.9+` on macOS arm64, manylinux x86_64, and Windows x86_64. Other platforms must build the UniFFI library locally.

## Render A Diagram

```python
import merman

engine = merman.MermanEngine()
merman.require_text_measurement_protocol_version(
    merman.TEXT_MEASUREMENT_PROTOCOL_VERSION
)

source = "flowchart TD\nA[Hello] --> B[World]"
svg = engine.render_svg(source, None)
print(svg[:4])  # <svg
```

The same engine exposes `render_png`, `render_jpeg`, `render_pdf`, `render_ascii`, `parse_json`, `layout_json`, `analyze_json`, `validate`, theme and lint metadata, ASCII support grades, and the complete diagram-family capability catalog. `MermanOperationRequest` plus `engine.execute()` is the generic, descriptor-owned form of those named methods and returns binary-safe data with media type and operation metadata. Generic options belong in `MermanOperationRequest.options_json`; `execute()` has no separate options argument.

## Reuse An Engine

Use a reusable engine when calls share baseline options:

```python
reusable = engine.reusable_engine('{"svg":{"pipeline":"readable"}}')
svg = reusable.render_svg(source, '{"svg":{"diagram_id":"preview"}}')
facts = reusable.analyze_document_facts_json(
    "```mermaid\n" + source + "\n```",
    None,
    "file:///workspace/README.md",
)
```

`options_json` is optional and follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md). Invalid options and engine failures raise typed `MermanError` variants.
Reusable request options deeply merge over the construction baseline for one operation without
mutating it. They cannot change the constructor-owned `runtime_policy`.
`MermanError.Binding.kind` distinguishes `UNKNOWN_OPERATION` from `MISSING_CAPABILITY`; only the
latter carries a non-null, descriptor-owned `capability_id`.

Omitting `runtime_policy` always selects deterministic runtime state, even though release wheels
compile native adapters. Use `{"runtime_policy":"native"}` only when an operation should consult
the host clock, time-zone rules, and random source. Generic operation metadata records the selected
policy; a custom slim build missing a requested adapter raises the generated unsupported-operation
error.

Choose a profile from the shared [resource decision table](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md#resource-options), then use the generated builder:

```python
from merman import ResourceOptionsBuilder, ResourceProfile

resource_options = (
    ResourceOptionsBuilder()
    .profile(ResourceProfile.CONSTRAINED)
    .build()
    .to_options_json()
)
svg = engine.render_svg(source, resource_options)
```

Use `CONSTRAINED` for untrusted, public, or multi-tenant input; `INTERACTIVE` is for cooperative
local editing. The native CLI's default is intentionally separate (`trusted-native`).

Call `engine.runtime_catalog_json()` to inspect the loaded runtime catalog and exact profile values
instead of duplicating limits in application code. `merman.get_runtime_catalog(engine)` strictly
validates its flat schema `1` artifact facts, package identity, transport API, sorted stable IDs,
and local output/operation relations as one atomic response. New stable IDs remain forward
compatible. This direct binding API version is `3` and is independent from native C ABI and the
text-measurement protocol version.

Diagnostics and parser facts both use their final schema `1`, independently of UniFFI binding API
`3`. Other facts versions are rejected at the boundary; consumers of the removed TextScan shape
must migrate to parser-backed items and explicit unavailable bodies.

## Text Measurement

Merman owns a deterministic vendored text measurer by default. Keep it for servers, CLIs, CI, and documentation builds.

GUI, browser automation, and WebView hosts can implement `MermanTextMeasurer` and install it with `reusable_engine_with_text_measurer(...)` or `set_text_measurer(...)`; `clear_text_measurer()` restores the built-in measurer. Text-measurement protocol 1 exposes 19 exact operations (`0..18`), and each handled `MermanTextMeasureResult` must use the `MermanTextMeasurementResultKind` required by `request.operation`. Return `None` for operations that cannot be measured synchronously and faithfully. Invalid results and callback exceptions fall back for that operation.

Use a real font API from the surface that displays the SVG rather than estimating width from character counts. Keep callbacks fast, and do not re-enter or replace the measurer on the same reusable engine while a callback is active. The [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md) documents every operation and fallback; the repository's [Python smoke example](https://github.com/Latias94/merman/blob/main/platforms/python/merman/examples/smoke.py) exercises all generated callback shapes for contract testing.

The generated callback is `measure(self, request)`, not `measure_text`. One-shot calls pass
`options_json` as their second argument; reusable calls inherit options from construction and accept
only the source. The bindgen test executes the linked smoke example against a freshly generated
module and its matching native library, so it is the authoritative copy-paste reference.

## Output And Platform Limits

- SVG may contain styles, markers, and `foreignObject` HTML labels; the final viewer must support the selected SVG pipeline.
- All APIs are synchronous. Run expensive rendering outside a GUI event loop.
- Wheels bundle a platform-specific native library and are not portable across operating systems or CPU families.
- Merman targets structural and semantic Mermaid compatibility; browser font rendering and DOM measurements can still differ unless the host provides matching metrics.

Query `diagram_family_capabilities()` and `ascii_capabilities()` at runtime instead of assuming that every build profile or output format supports every family.

## Local Development

Generate bindings and the adjacent native library from this checkout:

```sh
cargo build -p merman-uniffi --release --no-default-features --features analysis,ascii,jpeg,layout-cytoscape,layout-elk,math,pdf,png,svg,system-clock,system-random,system-timezone
cargo run -p merman-uniffi --no-default-features --features 'analysis,ascii,jpeg,layout-cytoscape,layout-elk,math,pdf,png,svg,system-clock,system-random,system-timezone,bindgen-smoke' --example generate_python_package -- \
  --cdylib target/release/libmerman_uniffi.dylib \
  --package-dir platforms/python/merman
PYTHONPATH=platforms/python/merman/src python platforms/python/merman/examples/smoke.py
```

Build and install-smoke a platform wheel with:

```sh
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

The helper resolves the descriptor-owned `python-uniffi-native` recipe. The bundled release
library excludes `bindgen-smoke`; only the source-generation process enables that feature. It also
embeds the checked-in Rust dependency license report for the selected target rather than the union
of every published wheel target.

## Documentation And Releases

- [Python binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)
- [Package changelog](https://github.com/Latias94/merman/blob/main/platforms/python/merman/CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Issue tracker](https://github.com/Latias94/merman/issues)
- [Source repository](https://github.com/Latias94/merman)

PyPI is the supported registry channel for the Python package. Release wheels are also attached to the corresponding GitHub release; this README does not imply support for platforms without a listed wheel.

## License And Notices

This package is available under MIT or Apache-2.0. The installed distribution carries the exact
release license, notices, and upstream texts under its `.dist-info/licenses/` directory. Online
copies live in the repository's [Python package directory](https://github.com/Latias94/merman/tree/main/platforms/python/merman).
