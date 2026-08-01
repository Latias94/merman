# Merman

<p align="center">
  <img
    src="https://raw.githubusercontent.com/Latias94/merman/main/assets/readme/hero.svg"
    width="100%"
    alt="Merman turns Mermaid source into SVG, image, text, and editor outputs through a headless Rust pipeline"
  />
</p>

<p align="center">
  <a href="https://github.com/Latias94/merman/actions/workflows/ci.yml"><img alt="CI status" src="https://github.com/Latias94/merman/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://crates.io/crates/merman"><img alt="merman on crates.io" src="https://img.shields.io/crates/v/merman.svg"></a>
  <a href="https://docs.rs/merman"><img alt="Rust API documentation" src="https://docs.rs/merman/badge.svg"></a>
  <a href="https://www.npmjs.com/package/@mermanjs/web"><img alt="@mermanjs/web on npm" src="https://img.shields.io/npm/v/%40mermanjs%2Fweb?label=npm"></a>
  <a href="#license-and-attribution"><img alt="MIT or Apache 2.0 license" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg"></a>
</p>

<p align="center">
  <a href="https://frankorz.com/merman/">Playground</a> |
  <a href="#quick-start">Quick start</a> |
  <a href="https://github.com/Latias94/merman/blob/main/docs/FEATURES.md">Choose capabilities</a> |
  <a href="https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md">Compatibility</a> |
  <a href="https://github.com/Latias94/merman/blob/main/CHANGELOG.md">Changelog</a>
</p>

Merman is an independent, parity-focused Rust implementation of [Mermaid.js](https://mermaid.js.org/). It targets `mermaid@11.16.0` and parses, analyzes, lays out, and renders Mermaid source without starting Node.js, Puppeteer, Chromium, or another JavaScript runtime in the native render path.

Use it as a Rust library, an `mmdc`-style CLI, a browser WASM package, an editor language engine, or a native SDK. The same parser-owned semantics drive every surface.

> **Adopted by Zed.** Zed replaced its previous Rust Mermaid backend with Merman after comparing real diagrams, citing Merman's rendering accuracy as the reason for the move. [Read the merged integration](https://github.com/zed-industries/zed/pull/57644).

## See The Output

| Architecture | Mindmap | Sankey |
| :-: | :-: | :-: |
| <img width="280" alt="Architecture diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/architecture.png"> | <img width="280" alt="Mindmap rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/mindmap.png"> | <img width="280" alt="Sankey diagram rendered by Merman" src="https://raw.githubusercontent.com/Latias94/merman/main/docs/assets/showcase/sankey.png"> |

These are headless `merman-cli` outputs. The [Playground](https://frankorz.com/merman/) has a searchable example for every admitted family.

## Why Merman

- **Parity is tested at multiple layers.** Source-backed semantic JSON, typed layout snapshots, and pinned upstream SVG DOM baselines catch different classes of drift. The current primary matrix covers 35 Mermaid families.
- **Rendering is browserless by design.** Native applications, CI jobs, documentation builds, and editors do not need a bundled browser just to turn diagram text into SVG.
- **One language model serves every workflow.** Rendering, diagnostics, LSP features, the Playground, and bindings share parser-owned facts instead of maintaining parallel regex or syntax implementations.
- **Outputs are explicit contracts.** Mermaid-style SVG, export-safe SVG, PNG, JPEG, vector PDF, ASCII/Unicode, semantic JSON, and layout JSON remain separately selectable.

## Quick Start

These examples use the published 0.8 prerelease channel. Check the selected package version when your integration depends on a newly introduced contract.

### Command Line

Install the complete CLI and render a diagram:

```sh
cargo install merman-cli --version '^0.8.0-alpha.3' --locked
printf 'flowchart LR\n  Source --> Merman --> SVG\n' | \
  merman-cli render - --output diagram.svg
```

Native commands use explicit single-diagram and Markdown workflows:

```sh
merman-cli render diagram.mmd
merman-cli render diagram.mmd --format png --theme dark --background transparent
merman-cli batch README.md
```

Scripts migrating from the official CLI use the pinned compatibility command:

```sh
merman-cli mmdc -i diagram.mmd -o diagram.svg
```

Root help and completions do not advertise `-i` / `-o`. Root invocations that begin with an `mmdc` option remain permanently supported as silent compatibility aliases and use the exact `merman-cli mmdc` parser and execution path; bare root inputs and native-only root options fail with guidance to an explicit workflow. New scripts should still choose `render`, `batch`, or explicit `mmdc` so their intended contract is visible.

Native `render` and `batch` use `-f/--format`; their hidden `-e` aliases share the `v0.9.0` removal date, but `mmdc -e/--outputFormat` remains part of the compatibility interface. See the [`merman-cli` guide](https://github.com/Latias94/merman/blob/main/crates/merman-cli/README.md) for the migration table, PDF, ASCII/Unicode, Iconify, runtime policy, and recoverable batch output.

### Rust

```sh
cargo add merman@^0.8.0-alpha.3
```

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

The default dependency enables complete deterministic SVG rendering, including both optional layout engines and math. It does not read ambient clock, time-zone, randomness, or timing state. More examples are organized by task in [`crates/merman/examples`](https://github.com/Latias94/merman/tree/main/crates/merman/examples).

### Browser

Install the complete browser package from the current alpha channel and initialize it once per browser realm:

```sh
npm install @mermanjs/web@alpha
```

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();
const svg = renderSvg(`flowchart TD
  A[Start] --> B[Done]`);
```

The browser package does not provide a Node.js or SSR fallback. See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for Worker lifecycle, custom WASM loading, and resource policy.

### From Source

Use an immutable commit when testing unreleased source behavior:

```sh
cargo install --git https://github.com/Latias94/merman --rev FULL_COMMIT_SHA --locked merman-cli
cargo add merman --git https://github.com/Latias94/merman --rev FULL_COMMIT_SHA
```

## Choose Your Surface

| You want to | Start with |
| --- | --- |
| Render from Rust | [`merman`](https://crates.io/crates/merman) |
| Render from a shell, CI job, or docs build | [`merman-cli`](https://crates.io/crates/merman-cli) or the [stable Homebrew formula](https://formulae.brew.sh/formula/merman-cli) |
| Render in a browser with SVG only | [`@mermanjs/web-render`](https://github.com/Latias94/merman/blob/main/platforms/web/packages/render/README.md) |
| Combine browser rendering, analysis, ASCII, and editor APIs | [`@mermanjs/web`](https://www.npmjs.com/package/@mermanjs/web) |
| Analyze Mermaid without SVG | [`merman-analysis`](https://crates.io/crates/merman-analysis) |
| Add editor intelligence | [`merman-lsp`](https://crates.io/crates/merman-lsp) or the [VS Code preview](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme) |
| Call Merman from another language | [Python](https://pypi.org/project/merman/), [C/C++](https://github.com/Latias94/merman/tree/main/crates/merman-ffi#readme), [Flutter/Dart](https://pub.dev/packages/merman), [Android](https://github.com/Latias94/merman/tree/main/platforms/android#readme), or [Apple](https://github.com/Latias94/merman/tree/main/platforms/apple#readme) |
| Render in Rustdoc or Typst | [`merman-rustdoc`](https://crates.io/crates/merman-rustdoc) or the [Typst package](https://github.com/Latias94/merman/tree/main/packages/typst/merman#readme) |

For a shell, `cargo binstall merman-cli` installs the registry-selected release, while `brew install merman-cli` follows the stable Homebrew formula. Those external channels can trail the current source documentation, so check `merman-cli --version` before depending on a new contract.

The source installation above pins an immutable commit. Starting with `0.8.0-alpha.4`, direct GitHub archives bundle checked completion and man-page assets, while the complete binary keeps `merman-cli completion <shell>` as the portable fallback. The [CLI guide](https://github.com/Latias94/merman/tree/main/crates/merman-cli#install) compares the installation channels and their on-disk support files.

Publication routes differ by platform. The [package surface guide](https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md) distinguishes registry packages from repository or CI artifacts.

## Bring Only What You Need

Cargo features select observable capabilities and output backends, not diagram families. Every parser-capable build keeps the same Mermaid language catalog.

| Goal | Selection |
| --- | --- |
| Complete deterministic SVG | `merman` defaults, or `complete-svg` |
| Basic SVG without optional layout engines or math | `default-features = false, features = ["svg"]` |
| Diagnostics and editor APIs | `default-features = false, features = ["analysis", "editor"]` |
| Terminal output | `default-features = false, features = ["ascii"]` |
| Binary export | Add only the required `png`, `jpeg`, or `pdf` features |

For example, a basic SVG dependency is:

```toml
[dependencies]
merman = { git = "https://github.com/Latias94/merman", default-features = false, features = ["svg"] }
```

A lint-only CLI can omit rendering and export dependencies:

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli \
  --no-default-features --features analysis
```

If an input needs a layout engine or math renderer that was not compiled, Merman returns a typed `missing-capability` error instead of silently changing the diagram. The [capability guide](https://github.com/Latias94/merman/blob/main/docs/FEATURES.md) documents exact feature forwarding, browser packages, artifact profiles, and runtime/resource policy.

## Compatibility, Honestly

Merman prioritizes parser, model, layout, theme, sanitizer, and SVG DOM convergence with pinned Mermaid source. It does not claim byte-for-byte Chromium pixels.

- Browser font fallback, `getBBox()` floats, `foreignObject`, and RoughJS path geometry can remain documented residuals where no robust headless derivation exists.
- Mermaid-parity SVG can contain HTML labels. Use `render_svg_resvg_safe_sync()` or an export command when the consumer cannot render `foreignObject`.
- PNG, JPEG, and PDF are integration outputs with explicit allocation and resource limits; they are not browser screenshot parity contracts.
- ASCII/Unicode support varies by diagram family and should be capability-checked.

See the current [alignment dashboard](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md), [SVG pipeline guide](https://github.com/Latias94/merman/blob/main/docs/rendering/SVG_OUTPUT_PIPELINE.md), and [benchmark methodology](https://github.com/Latias94/merman/blob/main/docs/performance/BENCHMARKING.md) for the exact evidence boundary.

## Documentation

- [Upgrade from 0.8.0-alpha.3 to 0.8.0-alpha.4](https://github.com/Latias94/merman/blob/main/docs/release/ALPHA3_TO_ALPHA4_UPGRADE_GUIDE.md)
- [Choose capabilities and build profiles](https://github.com/Latias94/merman/blob/main/docs/FEATURES.md)
- [Diagram coverage and parity](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [CLI reference](https://github.com/Latias94/merman/blob/main/crates/merman-cli/README.md)
- [Browser packages](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
- [Integrations and editor workflows](https://github.com/Latias94/merman/blob/main/docs/integrations/README.md)
- [Host text measurement](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md)
- [Rendering security](https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md)
- [Changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)

## Development

```sh
cargo nextest run --workspace
cargo fmt --all -- --check
cargo run -p xtask -- verify --strict
```

The strict gate verifies generated contracts, all-family SVG evidence, package surfaces, browser tests, and release legal material against the pinned reference bundle.

## License And Attribution

Merman is available under the [Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT).

Source translations, fixtures, embedded resources, behavioral references, and their exact revisions are recorded in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) and the [machine-readable component inventory](docs/release/THIRD_PARTY_COMPONENTS.json).

Merman is independent of, and not affiliated with, endorsed by, or sponsored by the Mermaid project or its maintainers.
