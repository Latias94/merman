# Merman

**A headless Rust implementation of [Mermaid] for parsing, layout, and rendering.**

[![CI status](https://github.com/Latias94/merman/actions/workflows/ci.yml/badge.svg)](https://github.com/Latias94/merman/actions/workflows/ci.yml) [![merman on crates.io](https://img.shields.io/crates/v/merman.svg)](https://crates.io/crates/merman) [![Rust API documentation](https://docs.rs/merman/badge.svg)](https://docs.rs/merman) [![MIT or Apache 2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](#license-and-attribution)

[Quick start](#quick-start) · [One-shot and reuse](#one-shot-and-repeated-rendering) · [Output targets](#output-targets) · [Cargo features](#cargo-features) · [Ecosystem](#ecosystem) · [Compatibility](#compatibility)

`merman` is the main Rust crate in this repository. It parses Mermaid source into a typed semantic
model, computes layout, and renders SVG. Optional features add diagnostics, editor facts,
ASCII/Unicode output, PNG, JPEG, and PDF. The native path does not start Node.js, Puppeteer,
Chromium, or another JavaScript runtime.

Hosts provide explicit output, theme, viewport, and resource policy; Merman returns an artifact,
metadata, or a typed error. Terminal detection, pager interaction, scheduling, caches, and save/open
or clipboard actions belong to the host. Start with the
[Rust embedding guide](docs/rendering/RUST_EMBEDDING.md) for migration and family/backend selection, or the
[host integration recipes](crates/merman/examples/README.md#host-integration-recipes) for terminal
text, agent/log output, browser SVG, and raster export using the existing request types.

For incremental editor syntax, the repository also publishes [`tree-sitter-mermaid`]: a tolerant
grammar and query package for Rust, Node.js, browser Workers, and editor integrations.

This README describes the **`0.8.0` source candidate**, aligned with Mermaid `12.1.0` at commit `21f72f07ea22c0af48a3149c550654e80d8e40cb`. The latest published workspace release is **`0.8.0-alpha.7`** (Mermaid `12.0.0`); `0.8.0` is not yet available from package registries. Start with the [stable upgrade guide](docs/release/V070_TO_V080_UPGRADE_GUIDE.md) when coming from `0.7.0`, the [alpha.7 upgrade guide](docs/release/ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md) when already on the prerelease, or the [versioned index](docs/release/README.md) for earlier alphas. The [comparison report](docs/release/V070_TO_V080_RELEASE_REPORT.md) covers measured changes and their limits.

> **Used by Zed.** Zed uses Merman as its Rust Mermaid backend. [Read the merged integration](https://github.com/zed-industries/zed/pull/57644).

## Quick start

For the published release, add `merman = "=0.8.0-alpha.7"` to your Cargo dependencies and use its [tagged documentation](https://github.com/Latias94/merman/blob/v0.8.0-alpha.7/README.md). The examples below target this source checkout; upgrade coupled Merman crates together when `0.8.0` becomes available.

Run the maintained SVG example from a source checkout:

```sh
cargo run --locked -p merman --example render_svg > diagram.svg
```

The same operation in Rust is:

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

The selected `RenderOutput` variant contains `None` when the source has no Mermaid diagram.
Cancellation, resource exhaustion, parse errors, and unavailable output capabilities remain
separate structured errors.

## One-shot and repeated rendering

A `Renderer` stores defaults. Each `RenderRequest` describes one independent operation.

| Type | Role | Typical lifetime |
| --- | --- | --- |
| `Renderer` | Holds the engine and shared defaults | One call or many calls |
| `RenderRequest` | Borrows the source and selects a target, overrides, and control | One operation |
| `OperationControl` | Carries cooperative cancellation and an optional deadline | One operation; clone it for the cancelling task or thread |
| `SemanticArtifact` | Pairs the parsed render model with its operation context | Inspect it, then consume it into one output target |

The quick start is the one-shot form: create a renderer, issue one request, and drop it. Merman does
not hide this behind a separate source-to-SVG helper; one-shot and repeated work both use
`Renderer::render()`.

For a batch or service, keep one configured `Renderer` when inputs share parse options, runtime
policy, site config, or resource limits. It shares settings, not parsed state: every call parses its
own source and carries a fresh `RenderRequest` and, normally, a fresh `OperationControl`.

If several SVGs will share one DOM, give every request a `diagram_id` that remains unique after
`merman::svg::sanitize_svg_id()` normalization. If an editor needs to retain analysis between
queries, use a host-owned snapshot from `merman::editor`; a reused renderer is not an editor
session.

The self-contained [Rust examples] cover these cases: [`render_svg.rs`] is one-shot,
[`render_many.rs`] reuses a configured renderer, and [`embed_multiple_svgs.rs`] assigns stable IDs
for a shared document.

## Output targets

Choose one typed target for each request:

| Need | Start with | Cargo feature |
| --- | --- | --- |
| Parse a typed Mermaid model | `Engine` and `ParseOptions` | Required `diagram-*` selectors, or `all-diagrams` |
| Prepare or inspect the semantic artifact | `Renderer::prepare_semantic()` or `RenderTarget::Semantic` | Required `diagram-*` selectors, or `all-diagrams` |
| Render Mermaid-style SVG | `RenderRequest::svg()` | `svg` |
| Inspect layout JSON or an SVG capability plan | `RenderRequest::layout_json()` or `RenderRequest::svg_plan()` | `svg` |
| Render terminal text for supported families | `RenderRequest::ascii()` | `ascii` |
| Export PNG, JPEG, or PDF | `RenderRequest::png()`, `jpeg()`, or `pdf()` | matching output feature |
| Produce diagnostics or analyze Markdown and MDX fences | `merman::analysis::Analyzer` | `analysis` |
| Build parser-backed editor snapshots | APIs under `merman::editor` | `editor` |

Cargo features remove unavailable target types at compile time. Within a compiled target, a
missing layout engine or runtime adapter returns a typed `missing-capability` error instead of
silently choosing a different result.

## Cargo features

The current-source `merman` dependency defaults to `all-diagrams` and `complete-svg-elk`: all
built-in families, SVG rendering, Cytoscape and ELK layouts, and math labels. Distributed artifacts
include the ELK implementation's EPL-2.0 notices and source provenance. Analysis, editor APIs,
terminal output, binary export, and ambient system adapters remain opt-in.

To keep SVG, Cytoscape, and math but **exclude the compiled ELK implementation**, use this declaration in a consumer checked out beside this repository:

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = ["all-diagrams", "complete-svg"] }
```

`all-diagrams` retains every built-in parser; `complete-svg` selects outputs and engines, not languages. Replace the path with a version only after that version is published. Cargo features are additive: another dependency can re-enable ELK, so inspect the final application graph with `cargo tree -e features -i merman-elk-layered`.

To **keep ELK available but render a diagram with Dagre**, set the top-level layout in Mermaid source (`theme` and `look` are optional):

```text
---
config:
  layout: dagre
  theme: default
  look: classic
---
flowchart TD
  A[Start] --> B[Done]
```

Selecting Dagre at runtime does not remove compiled ELK or its distribution notices. Explicit presentation settings do not guarantee pixel-identical output to earlier Mermaid versions; see the [stable upgrade guide](docs/release/V070_TO_V080_UPGRADE_GUIDE.md#presentation-and-snapshots).

In the current source, positive `diagram-*` features select built-in families independently of
outputs and engines. Low-level crates and facade consumers disabling defaults must select
`all-diagrams` or their required families explicitly. Packages at alpha.6 and earlier predate this
feature/API migration; see the [capability guide] for current-source recipes.

| Goal | Cargo selection |
| --- | --- |
| Complete deterministic SVG with ELK | defaults, or `default-features = false, features = ["all-diagrams", "complete-svg-elk"]` |
| SVG, Cytoscape, and math without ELK | `default-features = false, features = ["all-diagrams", "complete-svg"]` |
| Basic SVG without optional layout engines or math | `default-features = false, features = ["all-diagrams", "svg"]` |
| Diagnostics and editor APIs | `default-features = false, features = ["all-diagrams", "analysis", "editor"]` |
| Terminal output | `default-features = false, features = ["all-diagrams", "ascii"]` |
| Flowchart + Gantt SVG only | `default-features = false, features = ["svg", "diagram-flowchart", "diagram-gantt"]` |
| Binary export | Add only the required `png`, `jpeg`, or `pdf` feature |

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

## Rendered output

| Architecture | Mindmap | Sankey |
| :-: | :-: | :-: |
| <img width="280" alt="Architecture diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/architecture.png"> | <img width="280" alt="Mindmap rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/mindmap.png"> | <img width="280" alt="Sankey diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/sankey.png"> |

These examples were rendered headlessly by `merman-cli`, which uses the same Rust parser and
rendering pipeline. The source-checkout Playground covers all 37 built-in diagram types, including
Agentflow and Usecase. The hosted [Playground] reflects its deployed package version.

## Ecosystem

Choose the surface that owns the job instead of pulling the complete renderer into every host:

| Need | Start with |
| --- | --- |
| Parse, lay out, and render from Rust | `merman` |
| Run shell commands, Markdown batches, linting, or `mmdc` compatibility | [`merman-cli`] |
| Use WebAssembly in a browser or Worker | [Browser packages] |
| Use native Node.js bindings | [Node.js package] |
| Build incremental CSTs, syntax highlighting, folding, or selections | [`tree-sitter-mermaid`] on [crates.io] or [`@mermanjs/tree-sitter-mermaid`] on npm |
| Add diagnostics, completion, navigation, and rename | [`merman-lsp`] or the [VS Code extension] |
| Integrate C/C++, Python, Flutter, Android, Apple, Typst, or other delivery surfaces | [Package surface guide] |

`tree-sitter-mermaid` is independently versioned because editor syntax trees and queries have a
different compatibility contract from Merman's semantic model and renderer. Its [package README]
covers Node.js, browser, Rust, C/C++, query, and downstream-editor integration.

The [documentation index] covers architecture records, contributor procedures, parity evidence,
and release operations. The [Typst package] and other independently delivered integrations remain
listed in the [package surface guide].

## Rustdoc integrations

Merman offers two static-SVG paths for Rustdoc; neither loads JavaScript or fetches diagrams when a
reader opens the generated documentation.

| Choose | When |
| --- | --- |
| [`merman-cli` Rustdoc guide] | Generate and commit checked Markdown fragments without adding a renderer to the documented crate's Cargo graph |
| [`merman-rustdoc`] | Render annotated Mermaid blocks during `cargo doc` through an opt-in procedural macro and native renderer closure |

The dedicated guides cover configuration, CI freshness, docs.rs, packaging, generated ownership,
and migration. The two paths are explicit alternatives; neither silently falls back to the other.

## Compatibility

Merman aims for source-backed agreement in parsing, semantic models, layout, configuration,
theming, sanitization, and SVG DOM structure. It does not promise byte-for-byte Chromium pixels.

Browser font fallback, `getBBox()` floats, `foreignObject`, HTML labels, and RoughJS path geometry
can still produce documented differences where a robust headless equivalent is unavailable.
Merman's built-in text measurer is deterministic and font-agnostic; products with a host
measurement service should use the final display stack as the primary authority and retain the
built-in measurer as a per-request fallback. Rust applications that can always measure a complete
string can use `DeterministicTextMeasurer::with_width_callback(...)`; Merman still handles wrapping.
See [`render_svg_monospace.rs`](crates/merman/examples/render_svg_monospace.rs). If measurement can
fail or must vary by operation, use `HostTextMeasurer`. The Typst plugin has no synchronous host
import and uses deterministic measurement only.
Mermaid-style SVG may contain HTML labels. Use `SvgPipeline::resvg_safe()` or a typed PNG, JPEG, or
PDF target when the consumer cannot render `foreignObject`.
The resvg-safe fallback resolves supported typography from the original SVG/XHTML context before
removing HTML; host styles that change font metrics should therefore enter the same pipeline before
fallback generation. See the [fallback typography audit] for the bounded CSS subset and residuals.

Read the [alignment dashboard], [SVG output pipeline], [rendering security guide], and [benchmark
methodology] for the current evidence boundary.

## Development

```sh
cargo nextest run --workspace
cargo fmt --all -- --check
cargo run -p xtask -- verify --strict
```

The strict gate checks generated contracts, all-family SVG evidence, package surfaces, browser
tests, and release legal material against the pinned reference bundle.

## License and attribution

Merman is available under the [Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT).

Source translations, fixtures, embedded resources, behavioral references, and their exact
revisions are recorded in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) and the
[machine-readable component inventory](docs/release/THIRD_PARTY_COMPONENTS.json).
The crate-level `MIT OR Apache-2.0` grant describes Merman's own code; a shipped artifact's actual
notice set follows its selected Cargo features and artifact profile. In particular, ELK is an
explicit EPL-2.0 closure and math may include OFL-1.1 font data.

Merman is independent of, and not affiliated with, endorsed by, or sponsored by the Mermaid
project or its maintainers.

[Mermaid]: https://mermaid.js.org/
[Playground]: https://frankorz.com/merman/
[Rust examples]: https://github.com/Latias94/merman/tree/main/crates/merman/examples
[`render_svg.rs`]: https://github.com/Latias94/merman/blob/main/crates/merman/examples/render_svg.rs
[`render_many.rs`]: https://github.com/Latias94/merman/blob/main/crates/merman/examples/render_many.rs
[`embed_multiple_svgs.rs`]: https://github.com/Latias94/merman/blob/main/crates/merman/examples/embed_multiple_svgs.rs
[capability guide]: https://github.com/Latias94/merman/blob/main/docs/FEATURES.md
[resource and options guide]: https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md
[alignment dashboard]: https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md
[SVG output pipeline]: https://github.com/Latias94/merman/blob/main/docs/rendering/SVG_OUTPUT_PIPELINE.md
[fallback typography audit]: https://github.com/Latias94/merman/blob/main/docs/alignment/RESVG_SAFE_FALLBACK_TYPOGRAPHY_AUDIT.md
[rendering security guide]: https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md
[benchmark methodology]: https://github.com/Latias94/merman/blob/main/docs/performance/BENCHMARKING.md
[`merman-cli`]: https://github.com/Latias94/merman/tree/main/crates/merman-cli#readme
[`merman-cli` Rustdoc guide]: https://github.com/Latias94/merman/tree/main/crates/merman-cli#rustdoc-fragments
[Browser packages]: https://github.com/Latias94/merman/blob/main/platforms/web/README.md
[Node.js package]: https://github.com/Latias94/merman/tree/main/platforms/node#readme
[`merman-lsp`]: https://github.com/Latias94/merman/tree/main/crates/merman-lsp#readme
[VS Code extension]: https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme
[crates.io]: https://crates.io/crates/tree-sitter-mermaid
[`@mermanjs/tree-sitter-mermaid`]: https://www.npmjs.com/package/@mermanjs/tree-sitter-mermaid
[`tree-sitter-mermaid`]: https://github.com/Latias94/merman/tree/main/distribution/tree-sitter-mermaid#readme
[package README]: https://github.com/Latias94/merman/tree/main/distribution/tree-sitter-mermaid#readme
[Package surface guide]: https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md
[`merman-rustdoc`]: https://github.com/Latias94/merman/tree/main/crates/merman-rustdoc#readme
[Typst package]: https://github.com/Latias94/merman/tree/main/distribution/typst/merman#readme
[documentation index]: https://github.com/Latias94/merman/blob/main/docs/README.md
