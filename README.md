# Merman

**Parse, analyze, and render [Mermaid] diagrams in Rust without a browser.**

[![CI status](https://github.com/Latias94/merman/actions/workflows/ci.yml/badge.svg)](https://github.com/Latias94/merman/actions/workflows/ci.yml) [![merman on crates.io](https://img.shields.io/crates/v/merman.svg)](https://crates.io/crates/merman) [![Rust API documentation](https://docs.rs/merman/badge.svg)](https://docs.rs/merman) [![@mermanjs/web alpha on npm](https://img.shields.io/npm/v/%40mermanjs%2Fweb/alpha?label=npm%20alpha)](https://www.npmjs.com/package/@mermanjs/web) [![MIT or Apache 2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](#license-and-attribution)

[Playground] · [Choose a surface](#choose-a-surface) · [Quick start](#quick-start) · [Compatibility](#compatibility-boundary) · [Documentation](#documentation)

Merman is an independent, parity-focused Rust implementation of Mermaid. Its native render path does not start Node.js, Puppeteer, Chromium, or another JavaScript runtime. It currently targets `mermaid@11.16.1`.

The same parser-owned semantic model powers the Rust library, CLI, browser WASM packages, experimental Node.js package, analysis and editor APIs, language server, and native SDKs.

> [!NOTE]
> The current `main` branch targets the 0.8 prerelease line. Package channels publish independently, so check [Releases] and the [package surface guide] before pinning Merman in CI or production.

> **Used by Zed.** Zed adopted Merman as its Rust Mermaid backend after evaluating rendering accuracy. [Read the merged integration](https://github.com/zed-industries/zed/pull/57644).

## Choose a surface

Start with the product surface that owns your workflow. Foundational workspace crates are implementation modules, not additional entry points.

| You want to | Start with | Boundary |
| --- | --- | --- |
| Render or inspect Mermaid from Rust | [`merman`](https://crates.io/crates/merman) | Complete Rust facade for parsing, layout, SVG, and optional outputs |
| Render from a shell, CI job, or docs build | [`merman-cli`](https://crates.io/crates/merman-cli) or [Homebrew](https://formulae.brew.sh/formula/merman-cli) | Native rendering, linting, Markdown batches, export, and `mmdc` compatibility |
| Run in a browser | [`@mermanjs/web-render`](https://github.com/Latias94/merman/blob/main/platforms/web/packages/render/README.md) for SVG; [`@mermanjs/web`](https://www.npmjs.com/package/@mermanjs/web) for the complete SDK | Browser-only WASM packages |
| Render in Node.js or a static-site build | [`@mermanjs/node`](https://github.com/Latias94/merman/tree/main/platforms/node#readme) | Experimental native Node.js 22+ SVG package |

Specialized entry points:

- **Analysis and editors:** [`merman-analysis`](https://crates.io/crates/merman-analysis), [`merman-lsp`](https://crates.io/crates/merman-lsp), and the [VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme).
- **Native bindings:** [Python](https://pypi.org/project/merman/), [C/C++](https://github.com/Latias94/merman/tree/main/crates/merman-ffi#readme), [Flutter/Dart](https://pub.dev/packages/merman), [Android](https://github.com/Latias94/merman/tree/main/platforms/android#readme), and [Apple](https://github.com/Latias94/merman/tree/main/platforms/apple#readme).
- **Documentation systems:** [`merman-rustdoc`](https://crates.io/crates/merman-rustdoc) and the [Typst package](https://github.com/Latias94/merman/tree/main/distribution/typst/merman#readme).

See the [package surface guide] for the complete delivery matrix, artifact profiles, and release boundaries.

## Rendered output

| Architecture | Mindmap | Sankey |
| :-: | :-: | :-: |
| <img width="280" alt="Architecture diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/architecture.png"> | <img width="280" alt="Mindmap rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/mindmap.png"> | <img width="280" alt="Sankey diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/sankey.png"> |

These files were produced headlessly by `merman-cli`. The [Playground] contains searchable examples for every admitted diagram family.

## Quick start

### Rust

The current source tree uses the operation-scoped `Renderer` facade introduced after the
published `0.8.0-alpha.5` tag. Run the maintained example from a source checkout:

```sh
cargo run --locked -p merman --example render_svg > diagram.svg
```

Embed the same API in Rust:

```rust
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = Renderer::new().render(RenderRequest::svg(
        "flowchart TD\n  A[Start] --> B[Done]",
        OperationControl::new(),
        SvgRequest::default(),
    ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    print!("{}", svg.svg());
    Ok(())
}
```

For an external source dependency, pin a reviewed full commit. Published `0.8.0-alpha.5` users
should follow that release's [tagged README](https://github.com/Latias94/merman/blob/v0.8.0-alpha.5/README.md)
instead of copying source-tree APIs across the version boundary.

| Task | Start with |
| --- | --- |
| Render one standalone SVG | `Renderer::render(RenderRequest::svg(...))` |
| Embed several SVGs in one document | Set a unique `SvgRequest.options.diagram_id` for each request |
| Parse or inspect semantics without rendering | `Renderer::prepare_semantic` or `RenderTarget::Semantic` |
| Inspect layout or export another format | `Renderer` with a typed `RenderRequest`/`RenderTarget` |

`Renderer` uses deterministic defaults. An undetected diagram is represented by the selected
`RenderOutput` variant containing `None`; cancellation, resource exhaustion, parse failure, and
unsupported targets remain distinct structured errors. When several SVGs share one DOM, supply
diagram IDs that remain unique after `merman::svg::sanitize_svg_id` normalization.

The [Rust examples](https://github.com/Latias94/merman/tree/main/crates/merman/examples) cover
same-document IDs, renderer reuse, semantic and layout inspection, PNG and terminal output,
deterministic dates, configuration, themes, and custom SVG pipelines.

### Command line

Install the published complete CLI and render from standard input:

```sh
cargo install merman-cli --version 0.8.0-alpha.5 --locked
printf 'flowchart LR\n  Source --> Merman --> SVG\n' | \
  merman-cli render - --output diagram.svg
```

Common native workflows are explicit:

```sh
merman-cli render diagram.mmd
merman-cli render diagram.mmd --format png --theme dark --background transparent
merman-cli batch README.md
merman-cli mmdc -i diagram.mmd -o diagram.svg
```

See the [`merman-cli` guide](https://github.com/Latias94/merman/tree/main/crates/merman-cli#readme) for installation channels, Markdown batches, analysis, export formats, resource policy, and `mmdc` migration details.

### Browser

Install one browser-only WASM package and initialize it once per browser realm:

```sh
npm install @mermanjs/web@alpha
```

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web";

await initMerman();
const target = document.querySelector("#diagram");
if (!target) throw new Error("missing #diagram mount point");

renderSvgToElement(target, `flowchart TD
  A[Start] --> B[Done]`);
```

`renderSvgToElement()` validates and mounts the generated SVG at the actual document boundary. Use
`renderSvg()` when the host needs the serialized SVG string instead.

Use [`@mermanjs/web-render`](https://github.com/Latias94/merman/blob/main/platforms/web/packages/render/README.md) when the browser needs SVG but not analysis, ASCII, or editor APIs. Browser packages do not provide a Node.js or SSR fallback; see the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md).

### Node.js

The experimental native loader supports Node.js 22 or newer and deterministic static SVG rendering:

```sh
npm install @mermanjs/node@alpha
```

```js
import { createNodeEngine } from "@mermanjs/node";

const engine = await createNodeEngine();
try {
  console.log(await engine.renderSvg("flowchart TD\nA --> B"));
} finally {
  await engine.dispose();
}
```

The loader selects an exact-version native package for supported hosts. It does not download binaries during installation or fall back to browser WASM. See the [Node.js package guide](https://github.com/Latias94/merman/tree/main/platforms/node#readme) for the supported platform and capability boundary.

## Engineering model

Merman keeps Mermaid syntax facts in one typed semantic core and projects them into product-specific operations:

```text
Mermaid source
    |
    v
parser-owned semantic model
    |-- diagnostics, fixes, editor facts, and LSP features
    |-- typed layout ----------------------------> Mermaid-style SVG
    |-- validated SVG ---------------------------> PNG, JPEG, and PDF
    `-- typed diagram models --------------------> ASCII and Unicode
```

- **Parity is evidence-backed.** Semantic JSON, typed layout snapshots, and pinned upstream SVG DOM baselines catch different classes of drift. The current primary matrix covers 35 built-in Mermaid families.
- **The native path is browserless.** Applications and build systems do not need to bundle Chromium or a JavaScript runtime just to render a diagram.
- **Capabilities are explicit.** Missing layout engines, math support, exporters, runtime adapters, or other optional capabilities return typed errors instead of silently selecting different behavior.
- **Outputs have separate contracts.** Mermaid-style SVG, export-safe SVG, raster/vector export, text output, semantic JSON, and layout JSON are deliberately distinct surfaces.

## Choose capabilities

Cargo features select observable capabilities and output backends, not diagram families. Every parser-capable build retains the same Mermaid language catalog.

| Goal | Cargo selection |
| --- | --- |
| Complete deterministic SVG | `merman` defaults, or `complete-svg` |
| Basic SVG without optional layout engines or math | `default-features = false, features = ["svg"]` |
| Diagnostics and editor APIs | `default-features = false, features = ["analysis", "editor"]` |
| Terminal output | `default-features = false, features = ["ascii"]` |
| Binary export | Add only the required `png`, `jpeg`, or `pdf` feature |

For the published `0.8.0-alpha.5` feature closure, a basic SVG-only dependency is:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.5", default-features = false, features = ["svg"] }
```

The [capability guide] documents exact feature forwarding, browser packages, artifact profiles, and runtime/resource policy.

## Compatibility boundary

Merman prioritizes source-backed convergence in parsing, semantic models, layout, theming, sanitization, and SVG DOM structure. It does not claim byte-for-byte Chromium pixels.

- Browser font fallback, `getBBox()` floats, `foreignObject`, HTML labels, and RoughJS path geometry can remain documented residuals where no robust headless derivation exists.
- Mermaid-parity SVG can contain HTML labels. Select `SvgPipeline::resvg_safe()` on a typed SVG
  request, or use a PNG, JPEG, or PDF target, when a raster consumer cannot render
  `foreignObject`; browser DOM insertion still requires an explicit host admission policy.
- PNG, JPEG, and PDF are bounded integration outputs with explicit allocation and resource limits,
  not browser screenshot parity contracts.
- ASCII and Unicode support is capability-checked by diagram family.

Read the [alignment dashboard], [SVG output pipeline], [rendering security guide], and [benchmark methodology] for the exact evidence and safety boundaries.

## Documentation

| Topic | Entry point |
| --- | --- |
| Capabilities, features, and artifact profiles | [Choosing Merman capabilities][capability guide] |
| Diagram coverage and parity evidence | [Alignment dashboard][alignment dashboard] |
| Packages, registries, and release delivery | [Package surface guide][package surface guide] |
| CLI workflows and compatibility | [`merman-cli` guide](https://github.com/Latias94/merman/tree/main/crates/merman-cli#readme) |
| Browser and Node.js packages | [Browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) · [Node.js package guide](https://github.com/Latias94/merman/tree/main/platforms/node#readme) |
| Analysis, editors, and host integrations | [Integration guide](https://github.com/Latias94/merman/blob/main/docs/integrations/README.md) |
| Rendering and security contracts | [SVG output pipeline][SVG output pipeline] · [Rendering security][rendering security guide] |
| Maintainer and operator documentation | [Documentation index](https://github.com/Latias94/merman/blob/main/docs/README.md) |
| Release history | [Changelog](CHANGELOG.md) · [Releases] |

## Development

```sh
cargo nextest run --workspace
cargo fmt --all -- --check
cargo run -p xtask -- verify --strict
```

The strict gate verifies generated contracts, all-family SVG evidence, package surfaces, browser tests, and release legal material against the pinned reference bundle. See the [documentation index](https://github.com/Latias94/merman/blob/main/docs/README.md) for architecture records, contributor procedures, and release operations.

## License and attribution

Merman is available under the [Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT).

Source translations, fixtures, embedded resources, behavioral references, and their exact revisions are recorded in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) and the [machine-readable component inventory](docs/release/THIRD_PARTY_COMPONENTS.json).

Merman is independent of, and not affiliated with, endorsed by, or sponsored by the Mermaid project or its maintainers.

[Mermaid]: https://mermaid.js.org/
[Playground]: https://frankorz.com/merman/
[Releases]: https://github.com/Latias94/merman/releases
[package surface guide]: https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md
[capability guide]: https://github.com/Latias94/merman/blob/main/docs/FEATURES.md
[alignment dashboard]: https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md
[SVG output pipeline]: https://github.com/Latias94/merman/blob/main/docs/rendering/SVG_OUTPUT_PIPELINE.md
[rendering security guide]: https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md
[benchmark methodology]: https://github.com/Latias94/merman/blob/main/docs/performance/BENCHMARKING.md
