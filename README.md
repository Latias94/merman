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

For a prebuilt renderer, [choose by host and required output](docs/release/PREBUILT_PRODUCTS.md).
The main-line Apple pilot includes a complete local SwiftPM package with mathematical SVG;
native C and WASI recommend their existing full SDKs. These source-level choices do not change
already published artifacts.

With Rust 1.95 or later, add Merman to your `Cargo.toml`:

```toml
[dependencies]
merman = "0.9.3"
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

## Diagram themes

The current source branch includes compiled visual presets such as Brutalist, Spotless,
Cyberpunk, and Ghibli. Open the Playground's **Theme** panel: **Mermaid theme** chooses the
base appearance shared by both engines; **Merman styling** adds a preset or imported recipe
to Merman. Cards describe the current diagram's design scope, and the selected preset shows
application guidance. Preview the styling, then **Apply** it. See the
[theme gallery](docs/rendering/theme-gallery/README.md) for six presets across Flowchart, Pie,
Sequence, Class, and XY Chart, or run the Rust example:

```sh
cargo run --locked -p merman --features svg --example theme_preset > themed.svg
```

Brutalist, Spotless, Cyberpunk, and Ghibli use visual design references from
[Modern Mermaid by gotoailab](https://github.com/gotoailab/modern_mermaid); see the
[design credits and retained MIT notice](docs/rendering/theme-gallery/README.md#design-references-and-license).

Use **Import JSON** in the panel to validate and preview a versioned theme recipe; download it
to edit with an external editor or the theme-design skill. Workspace share links preserve
the applied custom recipe and configuration. The same JSON can be used through Rust, Web,
Node, and native CLI APIs. The authoring format is alpha; see
[Create, Customize, and Share Diagram Themes](docs/rendering/custom-diagram-themes.md) for examples,
configuration precedence, and export limitations.

## One-shot and repeated rendering

A `Renderer` stores defaults. Each `RenderRequest` describes one independent operation.

For a batch or service, keep one configured `Renderer` when inputs share parse options, runtime
policy, site config, or resource limits. It shares settings, not parsed state: every call parses its
own source and carries a fresh `RenderRequest` and, normally, a fresh `OperationControl`.

If several SVGs will share one DOM, give every request a `diagram_id` that remains unique after
`merman::svg::sanitize_svg_id()` normalization. See the maintained examples for
[one-shot rendering](crates/merman/examples/render_svg.rs),
[renderer reuse](crates/merman/examples/render_many.rs), and
[embedding several SVGs](crates/merman/examples/embed_multiple_svgs.rs).

For redraws of unchanged source, current source builds expose `Renderer::prepare_source()` and
`render_prepared()`. Each output receives fresh controls, budgets, runtime state, and layout.
The [Rust embedding guide] covers semantic and presentation artifacts, output targets, editor
snapshots, and prepared rendering; the [development migration](docs/migrations/render-reuse-host-integration.md)
also covers Web prepared sources and persistent mounts.

## Cargo features

The current-source `merman` dependency defaults to `all-diagrams`, `svg`, `layout-cytoscape` and
`layout-elk`: all built-in families and SVG rendering with both layouts. Math labels require the
explicit `math` feature. The convenience features `complete-svg` (SVG and Cytoscape) and
`complete-svg-elk` (also ELK) remain available but no longer enable math; add `math` explicitly
when migrating a math-enabled aggregate selection. Neither aggregate selects diagram families. Distributed artifacts
include the ELK implementation's EPL-2.0 notices and source provenance. Analysis, editor APIs,
terminal output, binary export, and ambient system adapters remain opt-in.

To keep SVG, Cytoscape, and math but **exclude the compiled ELK implementation**, use this dependency declaration:

```toml
[dependencies]
merman = { version = "0.9.3", default-features = false, features = ["all-diagrams", "svg", "layout-cytoscape", "math"] }
```

For command-line use, follow the [CLI installation guide][CLI install], then render a Mermaid file:

```sh
merman-cli render diagram.mmd --output diagram.svg
```

Selecting Dagre at runtime does not remove compiled ELK or its distribution notices. Explicit presentation settings do not guarantee pixel-identical output to earlier Mermaid versions; see the [stable upgrade guide](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#presentation-and-snapshots).

In the current source, positive `diagram-*` features select built-in families independently of
outputs and engines. Low-level crates and facade consumers disabling defaults must select
`all-diagrams` or their required families explicitly. Packages at alpha.6 and earlier predate this
feature/API migration; see the [capability guide] for current-source recipes.

Themes select font-family names and typography values. Browsers and native export backends resolve
host fonts. Caller-supplied font bytes in theme recipes are rejected; no fonts are bundled.

The [capability guide] documents feature forwarding, artifact profiles, system adapters, and
resource policy. Parser-only applications can depend on `merman-core` directly. Applications that
need lower-level layout or SVG pipeline control can use the re-exports under `merman::svg` or depend
on `merman-render`.

## Determinism, cancellation, and limits

`Renderer::new()` uses deterministic engine defaults. `SvgRequest::default()` supplies the default
headless SVG environment. System clock, time-zone, random, and timing adapters are separate Cargo
features and must also be selected explicitly at runtime.

Every render request owns an `OperationControl`. Its clones share cooperative cancellation and an
optional monotonic deadline. Cancellation is observed at operation checkpoints; a synchronous host
callback already in progress may return before Merman reaches the next checkpoint.

Resource limits are part of the request contract. Missing capabilities and exhausted limits return
typed errors rather than partial output or a silent fallback. The
[host integration guide](docs/integration/RESOURCE_POLICY.md) shows how to select a finite policy,
run cancellable background previews, and keep layout work separate from host memory/concurrency.
See the [resource and options guide] for the complete policy model.

## Internal flow

```text
Mermaid source
    |-- tree-sitter-mermaid
    |      `-- tolerant CST ---------------------> highlighting, folding, syntax selection
    |
    `-- Merman semantic parser
           |-- typed model ----------------------> diagnostics, navigation, refactoring
           |-- typed layout ---------------------> Mermaid-style SVG
           |-- validated SVG --------------------> PNG, JPEG, and PDF
           `-- supported typed diagram models ---> ASCII and Unicode
```

The two parsers have different contracts. Tree-sitter keeps useful syntax structure while a document
is incomplete; Merman remains the strict semantic and rendering authority. The semantic model is
shared by analysis and rendering. Binary export starts from validated SVG, not a browser screenshot.

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

For the 0.8.0 to 0.9.0 API and output changes, use the [upgrade guide](docs/release/V080_TO_V090_UPGRADE_GUIDE.md). CLI SVG text measurement remains deterministic by default. Mermaid-to-PNG/JPEG/PDF exports share native fonts between layout and painting when built with `system-fonts`; use `--font-measurement deterministic` without font sources to retain previous layout estimates and CSS support. See the [CLI font guide](crates/merman-cli/README.md#explicit-fonts).

Published Merman `0.9.3` carries selected fixes on the `0.9.x` line and retains Mermaid `12.1.0`,
the Presentation API, and Options JSON schema `2`.
The development branch targets Mermaid `12.2.0` and includes the unreleased theme APIs described
here. Use the [release index](docs/release/README.md) and documentation at the matching release tag
for installed packages. See the [alignment dashboard] for current source-backed checks of semantic
models, layout, and SVG structure.

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
[resource and options guide]: https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md
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
