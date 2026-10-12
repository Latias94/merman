# Merman

**A headless Rust implementation of [Mermaid] for parsing, layout, and rendering.**

[![CI status](https://github.com/Latias94/merman/actions/workflows/ci.yml/badge.svg)](https://github.com/Latias94/merman/actions/workflows/ci.yml) [![merman on crates.io](https://img.shields.io/crates/v/merman.svg)](https://crates.io/crates/merman) [![Rust API documentation](https://docs.rs/merman/badge.svg)](https://docs.rs/merman) [![MIT or Apache 2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](#license-and-attribution)

[Quick start](#quick-start) · [Rendered output](#rendered-output) · [Ecosystem](#ecosystem) · [Compatibility](#compatibility) · [Documentation][documentation index]

Merman turns Mermaid source into diagrams without starting Node.js, Puppeteer, Chromium, or
another JavaScript runtime in the native rendering path. Embed it in a Rust application, render
from the command line, or use the browser and native bindings.

- **Browserless rendering.** Generate SVG in native applications, CI jobs, and documentation builds.
- **Mermaid compatibility.** Parsing, layout, configuration, theming, and SVG structure are checked
  against pinned upstream source and fixtures.
- **Selectable capabilities.** Use typed models and SVG by default; add diagnostics, editor support,
  PNG, JPEG, PDF, or ASCII/Unicode for supported diagram families.

> **Used by Zed.** Zed uses Merman as its Rust Mermaid backend. [Read the merged integration](https://github.com/zed-industries/zed/pull/57644).

## Rendered output

| Architecture | Mindmap | Sankey |
| :-: | :-: | :-: |
| <img width="280" alt="Architecture diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/architecture.png"> | <img width="280" alt="Mindmap rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/mindmap.png"> | <img width="280" alt="Sankey diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/sankey.png"> |

These diagrams were rendered headlessly by [`merman-cli`] using the same Rust engine.
Try the [Playground] for more examples; its hosted build follows its deployed package version.

## Quick start

With Rust 1.95 or later, add Merman to your `Cargo.toml`:

```toml
[dependencies]
merman = "0.9.0"
```

Render a source string to SVG:

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

From a source checkout, run the same maintained example:

```sh
cargo run --locked -p merman --example render_svg > diagram.svg
```

Default Rust features include all built-in diagram families, SVG, Cytoscape and ELK layouts,
and math labels. Other outputs and analysis/editor APIs are opt-in. See the [capability guide]
for smaller builds, including how to omit ELK, and the [Rust examples] for renderer reuse,
cancellation, and host integration.

For command-line use, follow the [CLI installation guide][CLI install], then render a Mermaid file:

```sh
merman-cli render diagram.mmd --output diagram.svg
```

## Ecosystem

| Your task | Start here |
| --- | --- |
| Embed parsing, layout, and rendering in Rust | [`merman` API documentation][Rust API] and [Rust embedding guide] |
| Render files, export images, process Markdown, or lint from the shell | [`merman-cli`] |
| Render or analyze in a browser or Worker | [Browser packages] |
| Render from native Node.js | [Node.js package] (experimental) |
| Add incremental syntax trees, highlighting, or folding | [`tree-sitter-mermaid`] (independently versioned) |
| Add diagnostics, completion, navigation, or rename | [`merman-lsp`] or [VS Code extension] |
| Add Mermaid diagrams to Rustdoc | [CLI fragments][CLI Rustdoc] or [`merman-rustdoc`] |
| Integrate C/C++, Python, Flutter, Android, Apple, or Typst | [Package surface guide] |

Use the [documentation index] for task-specific guides, the [release and upgrade index] when
upgrading, and the [Changelog] for user-facing changes. Each package channel publishes separately;
check its guide for available versions and capabilities.

## Compatibility

For the 0.8.0 to 0.9.0 API and output changes, use the [upgrade guide](docs/release/V080_TO_V090_UPGRADE_GUIDE.md). CLI text measurement remains deterministic by default; explicit font files or directories opt into actual-font layout.

Merman follows Mermaid `12.1.0`, with source-backed checks for semantic models, layout, and SVG
structure. See the [alignment dashboard] for diagram coverage and documented differences.

Text measurement, browser font fallback, HTML labels, `foreignObject`, and hand-drawn paths can
produce differences from browser output. Pixel-identical Chromium rendering is not guaranteed.
For consumers that cannot display HTML labels, choose the resvg-safe SVG pipeline or a typed
PNG, JPEG, or PDF target; see the [SVG output pipeline] and [rendering security guide].

## Development

See the [contributor and CI guide] for local checks and the [documentation index] for architecture,
parity evidence, and release procedures.

## License and attribution

Merman's own code is available under the [Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT).
Distributed third-party components have their own notices: default Rust rendering features include EPL-2.0 ELK,
and math may include OFL-1.1 font data. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
for attribution and the feature-dependent notice requirements.

Merman is independent of, and not affiliated with, endorsed by, or sponsored by the Mermaid
project or its maintainers.

[Mermaid]: https://mermaid.js.org/
[Playground]: https://frankorz.com/merman/
[Rust API]: https://docs.rs/merman
[Rust examples]: https://github.com/Latias94/merman/tree/main/crates/merman/examples
[Rust embedding guide]: https://github.com/Latias94/merman/blob/main/docs/rendering/RUST_EMBEDDING.md
[capability guide]: https://github.com/Latias94/merman/blob/main/docs/FEATURES.md
[alignment dashboard]: https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md
[SVG output pipeline]: https://github.com/Latias94/merman/blob/main/docs/rendering/SVG_OUTPUT_PIPELINE.md
[rendering security guide]: https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md
[`merman-cli`]: https://github.com/Latias94/merman/tree/main/crates/merman-cli#readme
[CLI install]: https://github.com/Latias94/merman/tree/main/crates/merman-cli#install
[CLI Rustdoc]: https://github.com/Latias94/merman/tree/main/crates/merman-cli#rustdoc-fragments
[Browser packages]: https://github.com/Latias94/merman/blob/main/platforms/web/README.md
[Node.js package]: https://github.com/Latias94/merman/tree/main/platforms/node#readme
[`merman-lsp`]: https://github.com/Latias94/merman/tree/main/crates/merman-lsp#readme
[VS Code extension]: https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme
[`tree-sitter-mermaid`]: https://github.com/Latias94/merman/tree/main/distribution/tree-sitter-mermaid#readme
[Package surface guide]: https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md
[`merman-rustdoc`]: https://github.com/Latias94/merman/tree/main/crates/merman-rustdoc#readme
[documentation index]: https://github.com/Latias94/merman/blob/main/docs/README.md
[release and upgrade index]: https://github.com/Latias94/merman/blob/main/docs/release/README.md
[Changelog]: https://github.com/Latias94/merman/blob/main/CHANGELOG.md
[contributor and CI guide]: https://github.com/Latias94/merman/blob/main/docs/development/CI.md
