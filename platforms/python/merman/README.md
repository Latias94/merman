# Merman For Python

[![PyPI](https://img.shields.io/pypi/v/merman)](https://pypi.org/project/merman/) [![Python](https://img.shields.io/pypi/pyversions/merman)](https://pypi.org/project/merman/) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Parse, analyze, lay out, and render Mermaid diagrams from Python without a browser or JavaScript runtime. The package ships Merman's Rust engine and exposes it through UniFFI.

> **Alpha:** Python and native APIs may break before the stable release. The package currently targets direct UniFFI binding API `6`, which is independent from the C ABI and text-measurement protocol. Install the Python wheel and native library as one artifact rather than mixing releases. The repository documents the prepared `0.8.0a6` source candidate, while the published PyPI prerelease channel may still resolve alpha.5 until the matching wheel is authorized.

## Install

Install the published prerelease channel from PyPI (currently alpha.5):

```sh
python -m pip install --pre merman
```

For alpha.6 candidate work, build the wheel and native library from the exact commit accepted by release preflight and keep those artifacts paired.

Published wheels currently target CPython-compatible Python `3.9+` on macOS arm64, manylinux x86_64, and Windows x86_64. A platform without a listed wheel is not an officially packaged target; supporting another target requires a native-port contribution rather than only a local `pip` build.

## Render A Diagram

```python
import merman

client = merman.Merman()

source = "flowchart TD\nA[Hello] --> B[World]"
svg = client.render_svg(source, None)
print(svg[:4])  # <svg
```

The same one-shot facade exposes `render_png`, `render_jpeg`, `render_pdf`, `render_ascii`, `parse_json`, `layout_json`, `analyze_json`, `validate`, theme and lint metadata, ASCII support grades, and the complete diagram-family capability catalog. The default wheel supports SVG, ASCII, semantic/layout operations, analysis, validation, and document analysis. Math-bearing SVG and PNG, JPEG, or PDF methods remain available for custom current-contract libraries; the default wheel raises `MermanError.Binding` with `MISSING_CAPABILITY` and the exact capability ID. `MermanOperationRequestV4` plus `client.execute()` is the generic, descriptor-owned form of those named methods and returns binary-safe data with media type and typed operation metadata. Generic options belong in `MermanOperationRequestV4.options_json`; `execute()` has no separate options argument. The binding API is 6; `MermanOperationRequestV4` retains its record name and carries an optional `MermanOperationControl` for cooperative cancellation and relative deadlines. ASCII capability records expose layout/width/encoding/fallback admission arrays, and ASCII output plans use schema 2 with explicit encoding.

## Reuse An Engine

Construct `MermanEngine` directly when calls share baseline options:

````python
engine = merman.MermanEngine(
    options_json='{"svg":{"pipeline":"readable"}}',
    services=None,
)
svg = engine.render_svg(source, '{"svg":{"diagram_id":"preview"}}')
facts = engine.analyze_document_facts_json(
    "```mermaid\n" + source + "\n```",
    "file:///workspace/README.md",
    None,
)
engine.close()
````

`options_json` is optional and follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md). Invalid options and engine failures raise typed `MermanError` variants. Reusable request options deeply merge over the construction baseline for one operation without mutating it. They cannot change the constructor-owned `runtime_policy`. `MermanError.Binding.kind` distinguishes `UNKNOWN_OPERATION` from `MISSING_CAPABILITY`; only the latter carries a non-null, descriptor-owned `capability_id`.

Release wheels use deterministic runtime state and do not bundle native clock, time-zone, or random adapters. A custom source build may enable the atomic `native-runtime` feature and then select `{"runtime_policy":"native"}`. The default wheel raises the generated unsupported-operation error when native policy is requested, and runtime discovery reports concrete adapter IDs only when they are present.

Choose a profile from the shared [resource decision table](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md#resource-options), then use the generated builder:

```python
from merman import ResourceOptionsBuilder, ResourceOverrideId, ResourceProfile

resource_options = (
    ResourceOptionsBuilder()
    .profile(ResourceProfile.CONSTRAINED)
    .limit(ResourceOverrideId.MAX_SOURCE_BYTES, 4 * 1024 * 1024)
    .build()
    .to_options_json()
)
svg = client.render_svg(source, resource_options)
```

Use `CONSTRAINED` for untrusted, public, or multi-tenant input; `INTERACTIVE` is for cooperative local editing. Leave the profile unset when a reusable request must inherit its constructor ceiling. The native CLI's default is intentionally separate (`trusted-native`).

Call `client.runtime_catalog_json()` to inspect the loaded runtime catalog and exact resource profile values instead of duplicating limits in application code. Decode `client.presentation_catalog_json()` for the open-ended theme preset, presentation profile, aspect, and capability-availability catalog. `merman.get_runtime_catalog(client)` strictly validates its flat schema `1` artifact facts, package identity, transport API, supported options/payload schema IDs, named metadata IDs, sorted stable IDs, and local output/operation relations as one atomic response. New stable IDs remain forward compatible. This direct binding API version is `6` and is independent from native C ABI and the text-measurement protocol version.

Diagnostics use schema `1` and parser facts use schema `2`, independently of UniFFI binding API `6`. Other facts versions are rejected at the boundary; consumers of the removed TextScan shape and Flowchart-only rich graph must migrate to generic parser-backed items and explicit unavailable bodies.

For generic operations, construct `MermanOperationControl(timeout_ms=...)`, retain it in the host,
and put it in `MermanOperationRequestV4.control`. Calling `cancel()` from another thread requests
cooperative termination. `MermanError.Binding.cancellation` reports the reason and observed phase;
resource failures continue to use the separate `resource` field. Opaque callbacks may complete
before the next checkpoint, so hard preemption requires a worker or process boundary.

## Text Measurement

Merman owns a deterministic, font-agnostic text measurer by default. Keep it for servers, CLIs, CI, and documentation builds.

GUI, browser automation, and WebView hosts can implement `MermanTextMeasurer`, start with `MermanEngineServices()`, call `with_text_measurer(...)`, and pass the returned immutable bundle to `MermanEngine(options_json, services)`. The original bundle remains unchanged, and the callback is immutable for that engine; construct a different engine to change or remove it. Text-measurement protocol 1 exposes 19 exact operations (`0..18`), and each handled `MermanTextMeasureResult` must use the `MermanTextMeasurementResultKind` required by `request.operation`. Return `None` for operations that cannot be measured synchronously and faithfully. Invalid results and Python exceptions delivered through UniFFI's generated callback trampoline fall back for that operation; Merman does not claim to catch arbitrary foreign unwinds outside that generated boundary.

Use a real font API from the surface that displays the SVG rather than estimating width from character counts. Keep callbacks fast and do not re-enter the same reusable engine while its callback is active. Callback-free engines allow concurrent operations; callback engines serialize admission and report typed `BUSY` to a competing caller, while same-engine callback reentry reports `REENTRANT_CALL`. The [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md) documents every operation and fallback; the repository's [Python smoke example](https://github.com/Latias94/merman/blob/main/platforms/python/merman/examples/smoke.py) demonstrates one representative callback path while owner-local tests enforce the complete protocol.

The generated callback is `measure(self, request)`, not `measure_text`. One-shot and reusable operations accept request-local `options_json`; reusable values deeply merge over the construction baseline for that call. The wheel builder executes the linked smoke example against the installed final wheel, so it is the authoritative copy-paste reference.

## Output And Platform Limits

- SVG may contain styles, markers, and `foreignObject` HTML labels; the final viewer must support the selected SVG pipeline.
- All APIs are synchronous. Run expensive rendering outside a GUI event loop.
- Wheels bundle a platform-specific native library and are not portable across operating systems or CPU families.
- Merman targets structural and semantic Mermaid compatibility; browser font rendering and DOM measurements can still differ unless the host provides matching metrics.

Query `diagram_family_capabilities()` and `ascii_capabilities()` at runtime instead of assuming that every build profile or output format supports every family.

Custom artifacts with binary exports expose `MermanOutputPlan` as an open record. Switch on `kind`,
inspect `raster` or `pdf_filter_images` for known plans, and retain `raw_json` so a newer native
library can report a future plan without forcing Python into a closed enum.

## Local Development

Build the descriptor-owned native library, regenerate bindings, assemble a wheel, install it into an isolated environment, and run the smoke test with:

```sh
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

The helper resolves the `python-uniffi-native` artifact recipe and chooses the platform library name (`.dylib`, `.so`, or `.dll`). The bundled release library excludes `binding-generation`; only source generation enables that feature. It also embeds the checked-in Rust dependency license report for the selected target rather than the union of every published wheel target. For manual binding generation, follow the [UniFFI maintainer guide](https://github.com/Latias94/merman/blob/main/crates/merman-uniffi/README.md).

## Documentation And Releases

- [Python binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)
- [Package changelog](https://github.com/Latias94/merman/blob/main/platforms/python/merman/CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Issue tracker](https://github.com/Latias94/merman/issues)
- [Source repository](https://github.com/Latias94/merman)

PyPI is the supported registry channel for the Python package. Release wheels are also attached to the corresponding GitHub release; this README does not imply support for platforms without a listed wheel.

## License And Notices

This package is available under MIT or Apache-2.0. The installed distribution carries the exact release license, notices, and upstream texts under its `.dist-info/licenses/` directory. Online copies live in the repository's [Python package directory](https://github.com/Latias94/merman/tree/main/platforms/python/merman).
