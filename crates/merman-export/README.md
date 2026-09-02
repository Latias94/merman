# merman-export

`merman-export` is the bounded binary-export layer behind Merman's PNG, JPEG, and PDF output. It encodes SVG that has already passed Merman's terminal resvg-compatible finalizer; it does not parse Mermaid source or choose a layout engine.

Most applications should depend on [`merman`](https://crates.io/crates/merman) and select its
typed PNG, JPEG, or PDF target. Use this crate directly only when the application needs to retain a
validated SVG artifact, inspect an allocation plan, or schedule encoding separately from Mermaid
rendering.

## Choose A Feature

The crate has no default features. Enable only the formats the application emits:

| Feature | Output | Main API |
| --- | --- | --- |
| `png` | Bounded PNG bitmap | `svg_to_png`, `prepare_raster`, `RasterOptions`, `RasterPlan` |
| `jpeg` | Bounded JPEG bitmap | `svg_to_jpeg`, `prepare_raster`, `RasterOptions`, `RasterPlan` |
| `pdf` | Vector PDF with bounded localized raster work | `svg_to_pdf`, `svg_to_pdf_with_options`, `prepare_pdf`, `PdfOptions` |

`png` and `jpeg` share private bitmap preparation. `pdf` is a separate vector export capability. Features are additive, but one output does not implicitly expose another output's API. The published `merman-export`, `merman`, and `merman-render` versions must match because the sealed SVG type crosses their crate boundaries.

## First Export

The `merman` facade owns the shortest source-to-output path. This dependency enables PNG and its
required basic SVG path without Cytoscape, ELK, or math engines:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.6", default-features = false, features = ["png"] }
```

```rust
use merman::svg::export::{RasterFitBox, RasterOptions};
use merman::{OperationControl, PngRequest, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = RasterOptions::default()
        .with_fit_to(RasterFitBox::contain(960, 540))
        .with_scale(2.0)
        .with_background("white");

    let output = Renderer::new().render(RenderRequest::png(
        "flowchart LR\n  Source --> PNG",
        OperationControl::new(),
        PngRequest {
            svg: SvgRequest::default(),
            options,
        },
    ))?;
    let RenderOutput::Png(Some(png)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    std::fs::write("diagram.png", png.bytes)?;
    Ok(())
}
```

Replace `png` with `jpeg` or `pdf` when only that format is required. JPEG uses `RasterOptions`; PDF uses its independent `PdfOptions` page and filter policy.

## Direct Encoding

A host that already owns SVG can run the terminal finalizer and encoder under one caller-owned
operation control:

```toml
[dependencies]
merman-core = { version = "=0.8.0-alpha.6", default-features = false }
merman-render = { version = "=0.8.0-alpha.6", default-features = false }
merman-export = { version = "=0.8.0-alpha.6", default-features = false, features = ["png"] }
```

```rust
use merman_core::OperationControl;
use merman_export::{RasterOptions, svg_to_png_controlled};
use merman_render::{environment::RenderEnvironment, svg::finalize_resvg_svg};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())?;
    let sealed = finalize_resvg_svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40">
              <text x="8" y="24">Render then encode</text>
            </svg>"#,
        &session,
    )?;
    let png = svg_to_png_controlled(&sealed, &RasterOptions::default(), control)?;
    std::fs::write("diagram.png", png)?;
    Ok(())
}
```

The encoder APIs accept `merman_render::svg::ResvgCompatibleSvg`, whose inner string cannot be forged directly. This type attests that the terminal resvg-compatible finalizer ran. It does not attest that the input originated from Mermaid: `merman_render::svg::finalize_resvg_svg` intentionally accepts arbitrary SVG and returns the sealed type after finalization. Applications that accept untrusted SVG still need appropriate source, time, memory, and concurrency policy.

The sealed producer contract intentionally keeps `merman-render` in this crate's resolved dependency closure. `merman-export` is therefore not a standalone arbitrary-SVG converter, and its crate boundary alone is not evidence that parsing or rendering dependencies were removed. Exact PNG, JPEG, and PDF closure claims are verified from the repository's artifact profiles.

All formats keep explicit allocation, embedded-image, and structural conversion limits. See the main project's [SVG, PNG, JPEG, and PDF output guide](https://github.com/Latias94/merman/blob/main/docs/rendering/RASTER_OUTPUT.md) for sizing policy, resource controls, PDF behavior, and known parity gaps.

## License

Licensed under either of Apache License, Version 2.0 or MIT at your option.
