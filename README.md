# merman

Mermaid syntax, rendered headlessly in Rust.

[![CI](https://github.com/Latias94/merman/actions/workflows/ci.yml/badge.svg)](https://github.com/Latias94/merman/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/merman.svg)](https://crates.io/crates/merman)
[![Documentation](https://docs.rs/merman/badge.svg)](https://docs.rs/merman)
[![Downloads](https://img.shields.io/crates/d/merman.svg)](https://crates.io/crates/merman)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license-and-attribution)

Merman is an independent, parity-focused Rust implementation of
[Mermaid.js](https://mermaid.js.org/). It parses, analyzes, lays out, and renders Mermaid source
without launching a browser or JavaScript runtime. The current compatibility target is
`mermaid@11.16.0`.

[Open the Playground](https://frankorz.com/merman/) |
[Coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md) |
[Changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md) |
[Documentation](#documentation)

## What It Does

- Produces Mermaid-compatible semantic JSON, typed layout JSON, and SVG.
- Exports PNG/JPG raster images and vector PDF through an explicit `resvg`-safe SVG pipeline.
- Renders supported diagrams as ASCII or Unicode for terminals and logs.
- Provides parser-backed diagnostics, completion, hover, navigation, rename, code actions,
  symbols, folding, and semantic tokens.
- Runs across Rust, CLI, browser WASM, C, Python, Flutter/Dart, Android, Apple, and Typst surfaces.
- Uses one family-owned semantic construction path for rendering, analysis, LSP, and Playground
  language intelligence.

The primary parity matrix contains 35 Mermaid families, 3,747 semantic goldens, 3,744 layout
goldens, and 3,696 pinned upstream SVG baselines. ZenUML has a separate, source-backed external
comparison lane based on the admitted ZenUML Core behavior graph. Query
`diagram_family_capabilities()` in integrations that need an exact runtime capability decision.

## Output

| Architecture | Mindmap | Sankey |
| --- | --- | --- |
| <img width="260" alt="Merman Architecture output" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/architecture.png" /> | <img width="260" alt="Merman Mindmap output" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/mindmap.png" /> | <img width="260" alt="Merman Sankey output" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/sankey.png" /> |

These images are rendered headlessly by `merman-cli`. The
[Playground](https://frankorz.com/merman/) contains a generated, searchable example for every
admitted family.

SVG remains vector markup and has no global width or height cap. PNG/JPG use `RasterOptions` to
bound their final pixel allocation; PDF uses a separate `PdfOptions` page policy and budgets only
localized filter bitmaps and embedded raster images. See the
[output sizing guide](https://github.com/Latias94/merman/blob/main/docs/rendering/RASTER_OUTPUT.md)
before enabling an unbounded mode for trusted oversized exports. Resvg-safe PNG/JPG/PDF conversion
also has a non-optional resolved-tree depth capability (256 native levels, 64 WebAssembly levels)
and native conversion uses a bounded worker stack; raw parity SVG remains available beyond that
backend boundary.

Bindings expose runtime-contract schema `1` and a descriptor-derived capability vocabulary so hosts
can discover the loaded transport/package/options versions, compiled capability/output IDs and
their implications, registry facts, stable resource-limit IDs, and exact profile values. General
bindings default to `interactive`, the CLI to `trusted-native`, and Typst enforces `constrained`;
Cargo features and raster/PDF/image allocation budgets remain separate concerns.

## Install

The commands below use the currently published `0.8.0-alpha.3` artifacts. Source on this branch may
describe `Unreleased` behavior in the
[changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md).

```sh
# CLI
cargo install merman-cli --version 0.8.0-alpha.3

# Rust library
cargo add merman@0.8.0-alpha.3 --features svg

# Browser / TypeScript
npm install @mermanjs/web@alpha

# Python
python -m pip install --pre merman

# Flutter
flutter pub add 'merman:0.8.0-alpha.3'
```

Homebrew also provides the latest non-prerelease CLI:

```sh
brew install merman-cli
```

MSRV is Rust `1.95`.

## Rust Quickstart

```rust
use merman::svg::HeadlessRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessRenderer::new().with_diagram_id("readme-example");
    let svg = renderer
        .render_svg_sync("flowchart TD\nA[Start] --> B[Done]")?
        .expect("diagram detected");

    println!("{svg}");
    Ok(())
}
```

`HeadlessRenderer` is the canonical public render facade. Use
`prepare_render_sync()` when the same operation needs matching typed layout and SVG artifacts.
Use `render_svg_sync()` for Mermaid-parity SVG,
`render_svg_readable_sync()` for fallback text while retaining `foreignObject`, or
`render_svg_resvg_safe_sync()` before rasterization.

More runnable examples are ordered by use case in
[`crates/merman/examples/README.md`](https://github.com/Latias94/merman/blob/main/crates/merman/examples/README.md).

## CLI Quickstart

```sh
# Detect and parse
merman-cli detect diagram.mmd
merman-cli parse diagram.mmd --pretty

# Layout and render
merman-cli layout diagram.mmd --pretty
merman-cli render diagram.mmd --out diagram.svg
merman-cli render --format unicode diagram.mmd
merman-cli render --format png --out diagram.png diagram.mmd

# Lint Mermaid files or Markdown/MDX Mermaid fences
merman-cli lint README.md
```

## Choose A Surface

| Use case | Surface | Status |
| --- | --- | --- |
| Mermaid parsing and typed models | [`merman-core`](https://crates.io/crates/merman-core) | Published |
| Rust parsing and rendering | [`merman`](https://crates.io/crates/merman) | Published |
| Command line | [`merman-cli`](https://crates.io/crates/merman-cli) / [Homebrew](https://formulae.brew.sh/formula/merman-cli) | Published |
| Browser and TypeScript | [`@mermanjs/web`](https://www.npmjs.com/package/@mermanjs/web) | Published |
| Analysis without SVG rendering | [`merman-analysis`](https://crates.io/crates/merman-analysis) | Published |
| Language Server Protocol | [`merman-lsp`](https://crates.io/crates/merman-lsp) | Published server |
| VS Code integration | [VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme) | Repository preview; VSIX artifacts built by CI |
| C or C++ | [`merman-ffi`](https://crates.io/crates/merman-ffi) | Versioned C ABI 3 for the 0.8 package line |
| Python | [`merman` on PyPI](https://pypi.org/project/merman/) | Published |
| Flutter and Dart | [`merman` on pub.dev](https://pub.dev/packages/merman) | Published |
| Android and Kotlin | [Android package](https://github.com/Latias94/merman/tree/main/platforms/android#readme) | GitHub release artifact; not Maven Central |
| Apple and Swift | [Apple package](https://github.com/Latias94/merman/tree/main/platforms/apple#readme) | Repository package plus XCFramework release artifact |
| Typst | [Typst package](https://github.com/Latias94/merman/tree/main/packages/typst/merman#readme) | Manual registry package and WASM transport |
| Rust API documentation | [`merman-rustdoc`](https://crates.io/crates/merman-rustdoc) | Published |

The [release surface contract](https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md)
records which channels are published, repository-only, or blocked.

## Browser And Editor Support

`@mermanjs/web` publishes separate `core`, `render`, `render-only`, `ascii`, `editor`,
and `full` entry points backed by capability-specific WASM artifacts. The `editor` surface
provides the complete 35-family parser and language-intelligence catalog without SVG, ASCII, host,
or ELK dependencies.

The Playground runs the editor surface in a local module Worker and projects the same Rust-owned
document snapshot into Monaco. It does not use regex fallback for semantic tokens or diagram
detection. The native LSP projects the same token descriptor and parser-backed facts into LSP
ranges, so browser and editor behavior share one semantic source.

The VS Code extension is implemented but not yet published to Marketplace. It can be built and
installed from this repository or from CI-generated VSIX artifacts.

## Native ABI And Text Measurement

C/C++, Android JNI, and Flutter/Dart use the native C ABI `3`; Apple Swift and Python use the
direct UniFFI binding API `3`; browser WebAssembly has its own transport API. A host must pair its
headers or generated bindings with the native library from the same release. The current
text-measurement contract contains 19 exact operations (`0..18`) with operation-specific result
kinds.

Merman's default measurer is deterministic and suitable for servers, CLIs, CI, and documentation
builds. A GUI or WebView that needs geometry matching its own fonts should install the host
measurement callback and use the same font/layout system as the final display. Unsupported
operations must return unsupported so Merman can use the named vendored fallback; character-count
width estimates are not a faithful replacement.

Browser hosts use `createBrowserTextMeasurementSession()`. Retain its synchronous `measure`
callback for the owned session and call `dispose()` when the realm ends.

Python hosts implement `MermanTextMeasurer` and install it with
`reusable_engine_with_text_measurer(...)` or `set_text_measurer(...)` on an existing reusable
engine.

See the
[host text-measurement contract](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md)
and [C ABI protocol](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md).

## Feature Selection

| Feature | Adds |
| --- | --- |
| `svg` | Typed layout and SVG |
| `ascii` | ASCII and Unicode output |
| `png` | PNG byte output |
| `jpeg` | JPEG byte output |
| `pdf` | Vector PDF byte output |
| `analysis` | Diagnostics and lint metadata on transport crates |
| `layout-cytoscape` | Architecture and non-`tidy-tree` Mindmap layout |
| `layout-elk` | Source-translated ELK layered layout |
| `math` | Pure-Rust math layout and embedded KaTeX font assets |
| `system-clock` | Capture wall-clock values into an operation policy |
| `system-timezone` | Resolve a complete system time zone, including DST rules |
| `system-random` | Seed an operation from the operating system |
| `system-timing` | Enable explicitly requested operation timing diagnostics |

The ergonomic `merman` facade defaults to `complete-svg`: SVG plus the native layouts and math
needed for normal headless rendering, without compiling ambient system adapters. For a deliberately
smaller source build, disable defaults and select a direct capability set explicitly. Cargo features
are additive, so absence claims must be made with an exact artifact profile using
`default-features = false`, not by adding another alias. Constrained WASM hosts should select a
documented build profile instead of assembling an accidental feature combination. The Typst package
enforces the fixed `constrained` resource policy and does not accept trusted or unbounded profiles
from document input.

System adapters are independent of Mermaid language support and do not authorize ambient reads
during parsing or rendering. A native caller captures them once into an operation policy; a
deterministic caller supplies explicit values and leaves the adapters disabled.

All builds share the complete pinned Mermaid language catalog, including configuration,
sanitization, detection, semantic parsing, and editor facts. A `merman-lsp --no-default-features`
build remains a protocol-neutral library; add `stdio` when that build must include the bundled
stdio binary. See the [complete feature matrix](docs/FEATURES.md) for crate-specific defaults and
forwarding edges.

## Compatibility And Limits

Merman prioritizes parser, model, layout, render, configuration, theme, and SVG DOM convergence
with pinned upstream source. Browser-only text metrics, `getBBox()` floats, font fallback,
`foreignObject`, and RoughJS path geometry can remain documented residuals when there is no
robust headless derivation.

Important boundaries:

- Primary-matrix admission is structural and semantic evidence, not pixel identity with Chromium.
- SVG can contain `foreignObject`; choose an export-safe pipeline for non-browser consumers.
- PNG/JPG and PDF export are best-effort integration outputs, not browser pixel-parity contracts;
  their resource policies are intentionally independent.
- ASCII support varies by family; query `ascii_capabilities()` instead of assuming full coverage.
- Inputs still consume CPU inside layout engines after admission. Use the resource profile
  appropriate for the trust boundary.
- Merman is independent of, and not endorsed by, Mermaid or its maintainers.

Current family status, exact corpus counts, and accepted residuals live in
[`docs/alignment/STATUS.md`](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
Benchmark reports separate import, initialization, first render, warm render, invalidation, and
failure evidence; see the
[benchmark methodology](https://github.com/Latias94/merman/blob/main/docs/performance/BENCHMARKING.md)
instead of comparing one engine's load time with another engine's warmed render.

## Development

```sh
# Fast local verification
cargo nextest run --workspace
cargo fmt --all -- --check

# Full Rust lint surface
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Complete release, parity, Web, Playground, VS Code, and legal-material gates
cargo run -p xtask -- verify --strict
```

The strict gate expects the pinned Mermaid reference bundle described in
[`tools/upstreams/README.md`](https://github.com/Latias94/merman/blob/main/tools/upstreams/README.md).
It verifies generated contracts, all-family SVG structure/parity/root evidence, package surfaces,
browser tests, and release legal materials.

Maintainers changing a checked-in grammar should follow the
[parser generation guide](https://github.com/Latias94/merman/blob/main/docs/development/PARSER_GENERATION.md).

## Documentation

- [Diagram coverage and parity](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Integration index](https://github.com/Latias94/merman/blob/main/docs/integrations/README.md)
- [LSP capabilities](https://github.com/Latias94/merman/blob/main/docs/lsp/README.md)
- [SVG output pipelines](https://github.com/Latias94/merman/blob/main/docs/rendering/SVG_OUTPUT_PIPELINE.md)
- [ASCII support matrix](https://github.com/Latias94/merman/blob/main/docs/rendering/ASCII_SUPPORT_MATRIX.md)
- [Binding options](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md)
- [Rendering security](https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md)
- [Architecture ownership](https://github.com/Latias94/merman/blob/main/docs/adr/0073-family-owned-diagram-architecture.md)
- [Release surfaces](https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md)
- [Changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)

## License And Attribution

Merman is available under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

This repository contains source translations, copied fixtures, embedded resources, and behavioral
references from other projects. Exact revisions, relationships, license files, and artifact scopes
are recorded in
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)
and the machine-readable
[`docs/release/THIRD_PARTY_COMPONENTS.json`](docs/release/THIRD_PARTY_COMPONENTS.json).

Merman is not affiliated with, endorsed by, or sponsored by the Mermaid project.
