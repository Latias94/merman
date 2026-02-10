# merman

Headless Mermaid in Rust.

This crate is a convenience wrapper around:

- `merman-core` (parsing + semantic JSON model)
- `merman-render` (layout + parity-focused SVG)

Baseline: Mermaid `@11.12.2` (upstream Mermaid is treated as the spec).

## Features

- `render`: enable layout + SVG rendering APIs
- `raster`: enable PNG/JPG/PDF output via pure-Rust SVG rasterization/conversion

## Quickstart

```rust
use merman_core::{Engine, ParseOptions};
use merman::render::{headless_layout_options, render_svg_sync, sanitize_svg_id, SvgRenderOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let layout = headless_layout_options();
    let svg_opts = SvgRenderOptions {
        diagram_id: Some(sanitize_svg_id("example-diagram")),
        ..Default::default()
    };

    let svg = render_svg_sync(
        &engine,
        "flowchart TD; A-->B;",
        ParseOptions::default(),
        &layout,
        &svg_opts,
    )?
    .unwrap();

    println!("{svg}");
    Ok(())
}
```

For parity policy and gates, see `docs/alignment/STATUS.md` in the repository.

