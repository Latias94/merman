# merman-ffi

[![Crates.io](https://img.shields.io/crates/v/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Documentation](https://docs.rs/merman-ffi/badge.svg)](https://docs.rs/merman-ffi)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Embed Merman's headless Mermaid parser, analyzer, layout engine, and SVG/terminal renderer in any C-compatible host. No browser or JavaScript runtime is required.

> **Alpha:** the library reports C ABI `2`, but prerelease ABI 2 records can still be replaced in place before the stable release. Always ship the header and native library from the same Merman release and keep the runtime ABI and struct-size checks.

## Build

The crates.io package contains source for `cdylib`, `staticlib`, and `rlib` artifacts; it is not a prebuilt native SDK.

```sh
cargo build -p merman-ffi --release
```

Include `include/merman.h` from the same crate archive or source release as the library, then link
the resulting `merman_ffi` artifact. Do not substitute a header from the repository's moving
`main` branch for a released native library.

## Minimal C Example

```c
#include "merman.h"
#include <stdio.h>

static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";

int main(void) {
    if (merman_abi_version() != MERMAN_ABI_VERSION) return 1;

    MermanResult result = merman_render_svg(source, sizeof(source) - 1, NULL, 0);
    if (result.code != MERMAN_OK) {
        merman_buffer_free(result.data);
        return 2;
    }

    fwrite(result.data.data, 1, result.data.len, stdout);
    merman_buffer_free(result.data);
    return 0;
}
```

Every non-empty `MermanResult.data` buffer must be released exactly once with
`merman_buffer_free`; never pass it to the host allocator. The crate archive includes
`examples/render_svg.c` for a complete consumer and `examples/render_svg_engine.c` for the
reusable engine, both matched to the packaged header.

## Public Surface

The stateless and reusable-engine entry points expose:

- Mermaid source to SVG or Unicode terminal text;
- semantic and layout JSON;
- diagram and Markdown/MDX document diagnostics;
- parser-backed document facts for editor integrations;
- validation, themes, lint rules, ASCII grades, and the complete diagram-family capability catalog.

Use `merman_engine_new` when several calls share one options document. Concurrent read-style calls require a thread-safe callback and `user_data`; callback replacement is an exclusive mutation. Do not re-enter the same engine from a callback, and release it exactly once with `merman_engine_free`.

`options_json` uses the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md). `NULL/0` selects defaults. The default SVG pipeline targets Mermaid parity; `readable` and `resvg-safe` are explicit alternatives for consumers with stricter SVG support.

## Compatibility Contracts

- Native ABI: `2`. Hosts must compare `merman_abi_version()` with `MERMAN_ABI_VERSION` and verify every exposed struct size before use.
- Diagnostics and parser-facts payload schemas: `1`. These schema numbers are independent of the native ABI. The current facts v1 contract is parser-only; the removed alpha TextScan shape is not accepted.
- Text measurement: ABI 2 defines 19 exact operations (`0..18`) and four tagged result kinds. A handled callback must return the result kind required by `request.operation`; do not infer a shape from zero-valued fields.
- Diagram discovery: query `merman_diagram_family_capabilities_json()` instead of hard-coding parser, editor, layout, or render availability.

This prerelease replaced the earlier ABI 2 text-measurement records without changing the numeric ABI. Rebuild the native library and host bindings together; old headers will fail the struct-size contract.

## Text Measurement Ownership

Merman owns a deterministic vendored text measurer by default. It is appropriate for servers, CLIs, CI, and documentation builds, and it keeps rendering available when no GUI font API exists.

Preview hosts can install `merman_engine_set_text_measure_callback` on a reusable engine when geometry must match the final font stack. The callback is synchronous and may run on any rendering thread. Measure with the same DOM, canvas, Core Text, Android text layout, or Flutter text stack that displays the SVG; return `handled = 0` when an operation cannot be answered immediately and faithfully. Unsupported, invalid, wrong-kind, or failed requests fall back per operation without transferring ownership of the render.

The exact request lifetimes, operation/result mapping, concurrency rules, and signed-length exceptions are specified in the [FFI protocol](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md#host-text-measurement) and [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md).

## Feature Selection

Default builds enable the full registry, host environment, SVG rendering, analysis, and terminal output. Minimal source builds can opt into the required surfaces:

```sh
cargo build -p merman-ffi --release --no-default-features --features analysis
cargo build -p merman-ffi --release --no-default-features --features render
cargo build -p merman-ffi --release --no-default-features --features ascii
cargo build -p merman-ffi --release --features elk-layout,ratex-math
```

Entry points remain exported when their feature is absent and return `MERMAN_UNSUPPORTED_FORMAT`. The `raster` Cargo feature prepares shared conversion support, but the C ABI does not yet expose raster byte-output functions.

## Platform Wrappers

- [Apple / Swift](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [Android / Kotlin](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Flutter / Dart](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Python / UniFFI](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)

Project scope and diagram coverage live in the [main README](https://github.com/Latias94/merman#readme) and [alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md). Release changes are recorded in the [project changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md).

## License And Notices

Merman is available under MIT or Apache-2.0. The crate archive includes the release-matched
`LICENSE-MIT` and `LICENSE-APACHE` texts. Project-wide source provenance and third-party legal
materials are recorded in [`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
