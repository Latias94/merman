# Merman Rust Examples

Every user-facing example in this directory is a self-contained Rust file. It declares its own Mermaid source, uses no shared `support` module, and can be copied into another crate's `examples/` directory.

Run commands below from the Merman repository root. Start with the default complete SVG build:

```sh
cargo run -p merman --example render_svg > diagram.svg
```

## Choose The Right Entry Point

| Your task | Start with | Why |
| --- | --- | --- |
| Render one source string to SVG | [`render_svg.rs`](render_svg.rs) | Uses `Renderer` with a typed `SvgRequest` and keeps the operation boundary explicit. |
| Embed several SVGs in one HTML document | [`embed_multiple_svgs.rs`](embed_multiple_svgs.rs) | Uses typed SVG requests with caller-owned IDs that remain unique after normalization. |
| Render many independent files with one policy | [`render_many.rs`](render_many.rs) | Reuses one configured `Renderer` across operations. |
| Export a bounded PNG | [`render_png.rs`](render_png.rs) | Selects a fit box, scale, background, and allocation limits before rasterization. |
| Render for terminals or logs | [`render_terminal.rs`](render_terminal.rs) | Selects Unicode or ASCII-only output explicitly. |
| Inspect the parsed semantic model | [`inspect_semantics.rs`](inspect_semantics.rs) | Uses `Engine` without requiring SVG rendering. |
| Inspect computed geometry and routes | [`inspect_layout.rs`](inspect_layout.rs) | Stops after typed layout and serializes the result. |
| Apply application-wide Mermaid defaults | [`configure_mermaid.rs`](configure_mermaid.rs) | Keeps host configuration outside user-authored diagram source. |
| Make relative dates deterministic | [`deterministic_gantt.rs`](deterministic_gantt.rs) | Pins "today" and the local offset for snapshots and reproducible builds. |
| Apply a ready-made product presentation | [`presentation_profile.rs`](presentation_profile.rs) | Combines the `merman-modern` profile with a semantic editor theme. |
| Map an application's own theme tokens | [`custom_presentation_theme.rs`](custom_presentation_theme.rs) | Builds a `HostTheme` from semantic roles instead of family-specific CSS. |
| Control consumer SVG cleanup and styling | [`custom_svg_pipeline.rs`](custom_svg_pipeline.rs) | Builds an explicit resvg-safe, background, and scoped-CSS pipeline. |

Use `Renderer` with a typed `RenderRequest` for every source-to-target operation. SVG request IDs can be normalized with `merman::svg::sanitize_svg_id`; dynamic integrations should use stable ASCII keys and ensure the normalized results are unique rather than deriving IDs only from display titles. The same request seam also carries layout, presentation, resource, pipeline, and cancellation policy.

## Run By Task

Render one standalone SVG or one HTML document containing multiple SVGs:

```sh
cargo run -p merman --example render_svg > diagram.svg
cargo run -p merman --example embed_multiple_svgs > diagrams.html
```

Reuse one renderer for several independent SVG files:

```sh
cargo run -p merman --example render_many -- target/example-svgs
```

Render PNG and terminal output. These capabilities are intentionally outside the default SVG feature set:

```sh
cargo run -p merman --features png --example render_png -- target/diagram.png
cargo run -p merman --features ascii --example render_terminal
cargo run -p merman --features ascii --example render_terminal -- --ascii
```

Inspect parser and layout results:

```sh
cargo run -p merman --example inspect_semantics
cargo run -p merman --example inspect_layout
cargo run -p merman --example deterministic_gantt
```

Configure host defaults, presentation, and output policy:

```sh
cargo run -p merman --example configure_mermaid > configured.svg
cargo run -p merman --example presentation_profile > presentation.svg
cargo run -p merman --example custom_presentation_theme > custom-theme.svg
cargo run -p merman --example custom_svg_pipeline > consumer-safe.svg
```

## Copy Into An Application

Use the exact alpha.6 dependency expected by these examples:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.6" }
```

Copy the relevant `.rs` file into your application's `examples/` directory and run it by filename:

```sh
cargo run --example render_svg
```

Enable `features = ["png"]` on the Merman dependency when copying `render_png`, or `features = ["ascii"]` when copying `render_terminal`. Add `serde_json = "1"` when copying `inspect_semantics`, `inspect_layout`, `configure_mermaid`, or `deterministic_gantt`.

## Minimize Features Later

The commands above favor a successful first run. Once the workflow is known, disable defaults and compile only the observable capabilities it needs:

| Examples | Minimal selection |
| --- | --- |
| `inspect_semantics`, `deterministic_gantt` | No Merman features |
| `render_svg`, `embed_multiple_svgs`, `render_many`, `inspect_layout`, `configure_mermaid`, `custom_presentation_theme`, `custom_svg_pipeline` | `svg` |
| `presentation_profile` | `layout-elk` (also enables `svg`) |
| `render_terminal` | `ascii` |
| `render_png` | `png` (also enables `svg`) |

For example:

```sh
cargo run -p merman --no-default-features --features svg --example render_svg > diagram.svg
cargo run -p merman --no-default-features --example inspect_semantics
cargo run -p merman --no-default-features --features png --example render_png
```

A minimal SVG build returns a typed `missing-capability` error when an input needs an optional layout engine or math renderer. It never silently substitutes a different semantic result. See the [capability guide](../../../docs/FEATURES.md) for dependency declarations and feature forwarding.

## Profiling

`profile_render.rs` is a maintainer tool rather than a learning example. It keeps the CPU inside a selected render stage for profilers:

```sh
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph \
  --profile bench \
  -p merman \
  --no-default-features \
  --features layout-cytoscape \
  --example profile_render \
  -o target/bench/flamegraphs/profile_render_architecture_medium.svg \
  -- \
  --input crates/merman/benches/fixtures/architecture_medium.mmd \
  --stage render \
  --seconds 20
```
