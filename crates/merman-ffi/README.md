# merman-ffi

[![Crates.io](https://img.shields.io/crates/v/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Documentation](https://docs.rs/merman-ffi/badge.svg)](https://docs.rs/merman-ffi)
[![Crates.io Downloads](https://img.shields.io/crates/d/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

C ABI bindings for embedding `merman` in non-Rust hosts.

`merman` is a headless Rust implementation of Mermaid diagram parsing, layout, and rendering. It is
intended for servers, CLIs, mobile apps, desktop apps, and other environments that need Mermaid
output without launching a browser. The main library can produce semantic JSON, layout JSON, SVG,
terminal text, and raster formats depending on enabled features.

Start with the main project README for product scope and diagram coverage:

- Repository: <https://github.com/Latias94/merman>
- Project README: <https://github.com/Latias94/merman>
- Rust library: <https://crates.io/crates/merman>
- CLI: <https://crates.io/crates/merman-cli>
- Coverage status:
  <https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md>

This crate exposes the low-level stable boundary described by
[`docs/bindings/FFI_PROTOCOL.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md).
Higher-level generated bindings such as UniFFI should sit above the same behavior, not replace this
C ABI.

## Build

From the workspace:

```sh
cargo build -p merman-ffi --release
```

The crate builds `cdylib`, `staticlib`, and `rlib` artifacts. Include
[`include/merman.h`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/include/merman.h)
from C or C-compatible hosts.

Optional features:

```sh
cargo build -p merman-ffi --release --features ratex-math
cargo build -p merman-ffi --release --features raster,ratex-math
```

The C ABI exposes SVG, ASCII text, semantic JSON, layout JSON, validation JSON, binding metadata,
and an optional host text-measurement callback for reusable engines. Native raster byte outputs are
intentionally split into a later ABI lane.

## Minimal C Usage

```c
#include "merman.h"

static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";

MermanResult result = merman_render_svg(source, sizeof(source) - 1, NULL, 0);
if (result.code == MERMAN_OK) {
    /* result.data contains UTF-8 SVG bytes. */
}
merman_buffer_free(result.data);
```

Every non-empty `MermanResult.data` buffer must be freed exactly once with `merman_buffer_free`.
Do not use `free`, `delete`, or a host runtime allocator for buffers returned by Rust.

For repeated calls with the same options, create a reusable engine:

```c
MermanEngineResult engine = merman_engine_new(NULL, 0);
if (engine.code != MERMAN_OK) {
    /* engine.data contains UTF-8 JSON error bytes. */
    merman_buffer_free(engine.data);
    return;
}

MermanResult result = merman_engine_render_svg(engine.engine, source, sizeof(source) - 1);
merman_buffer_free(result.data);
merman_engine_free(engine.engine);
```

Hosts that already own a font stack can install a text measurement callback on a reusable engine:

```c
MermanResult set_result =
    merman_engine_set_text_measure_callback(engine.engine, measure_text, user_data);
merman_buffer_free(set_result.data);
```

Return `handled=0` for measurement requests your host does not support. `merman` will fall back
to its vendored Mermaid-compatible measurer for that request. The callback may be invoked from any
thread that renders with the engine, so shared host font state must be thread-safe.

## Headless Font Measurement

Mermaid normally measures labels in a browser after CSS and font fallback have been resolved. A
headless renderer has to estimate those metrics before there is a DOM. That means browser and host
differences can show up as slightly wider labels, clipped `foreignObject` content, or layout drift
when the final display font differs from merman's vendored compatibility profile.

`merman` keeps Flowchart HTML labels non-clipping by default, which avoids losing trailing
characters when a browser chooses a wider fallback font. For hosts that need accurate geometry,
install `merman_engine_set_text_measure_callback` and measure with the same text stack that will
display the SVG:

- Browser/WebView hosts can use their DOM or canvas text measurement path.
- Native editors can use their own shaping and font fallback system.
- Android native previews should use `TextPaint` and `StaticLayout`.
- Apple native previews should use Core Text or matching `NSAttributedString` layout.
- Flutter/Dart previews should keep the measured reusable engine on the same isolate and use the
  same Flutter paragraph/text layout, WebView cache, or SVG widget measurement path as the final
  display surface.
- Unsupported requests can return `handled=0` and let merman fall back per request.

Treat the callback as a fidelity opt-in, not a required dependency. CLIs, documentation builds, CI,
and server-side batch renderers should usually keep the default vendored metrics because they are
deterministic and do not require host UI APIs. Editors, preview panes, design tools, and WebView
integrations should consider the callback when clipping or host-specific font fallback matters.
If a request would require async UI-thread work that is not already cached, return `handled=0`
instead of blocking the render thread.

The callback request includes the UTF-8 text, font family, size, weight, style, line height,
spacing, wrap mode, direction, white-space mode, and optional max width. See
[`include/merman.h`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/include/merman.h)
and
[`docs/bindings/FFI_PROTOCOL.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md#host-text-measurement)
for the exact ABI contract.

## Example

[`examples/render_svg.c`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/examples/render_svg.c)
is a small C consumer that renders a flowchart to SVG through the C ABI.
[`examples/render_svg_engine.c`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/examples/render_svg_engine.c)
shows the reusable engine/context API and a minimal text measurement callback for repeated calls
with shared options.

On macOS or Linux:

```sh
cargo build -p merman-ffi --release
cc -I crates/merman-ffi/include \
  crates/merman-ffi/examples/render_svg.c \
  -L target/release -lmerman_ffi \
  -Wl,-rpath,"$PWD/target/release" \
  -o target/merman-ffi-render-svg
target/merman-ffi-render-svg
```

To compile the reusable-engine example, replace `render_svg.c` with `render_svg_engine.c` in the
same command.

## Entry Points

- `merman_abi_version`
- `merman_package_version`
- `merman_buffer_struct_size`
- `merman_result_struct_size`
- `merman_engine_result_struct_size`
- `merman_host_text_measure_request_struct_size`
- `merman_host_text_measure_result_struct_size`
- `merman_engine_new`
- `merman_engine_free`
- `merman_engine_set_text_measure_callback`
- `merman_engine_render_svg`
- `merman_engine_render_ascii`
- `merman_engine_parse_json`
- `merman_engine_layout_json`
- `merman_engine_validate_json`
- `merman_render_svg`
- `merman_render_ascii`
- `merman_parse_json`
- `merman_layout_json`
- `merman_validate_json`
- `merman_supported_diagrams_json`
- `merman_ascii_supported_diagrams_json`
- `merman_supported_themes_json`
- `merman_supported_host_theme_presets_json`
- `merman_buffer_free`

See
[`include/merman.h`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/include/merman.h)
for declarations and
[`docs/bindings/FFI_PROTOCOL.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md)
for result codes, options JSON, threading, and compatibility rules.

Higher-level platform wrappers:

- Apple SwiftPM:
  [`docs/bindings/APPLE_SWIFT.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- Android JNI/Kotlin:
  [`docs/bindings/ANDROID_JNI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- Flutter/Dart FFI:
  [`docs/bindings/FLUTTER_DART_FFI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- Python UniFFI:
  [`docs/bindings/PYTHON_UNIFFI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)
