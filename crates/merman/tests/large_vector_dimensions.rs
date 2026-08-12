#![cfg(feature = "svg")]

use merman::svg::HeadlessRenderer;

const HUGE_VECTOR_SOURCE: &str = r#"---
config:
  xyChart:
    width: 100000
    height: 100000
---
xychart-beta
  x-axis [a, b]
  y-axis 0 --> 10
  line [1, 9]
"#;

#[test]
fn huge_mermaid_dimensions_remain_compact_vector_svg() {
    let svg = HeadlessRenderer::new()
        .render_svg_sync(HUGE_VECTOR_SOURCE)
        .unwrap()
        .expect("XYChart should render");
    let document = roxmltree::Document::parse(&svg).expect("SVG should remain valid XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 100000 100000"));
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("max-width: 100000px")),
        "{svg}"
    );
    assert!(
        svg.len() < 64 * 1024,
        "vector dimensions must not imply a full-page pixel buffer"
    );
}

#[test]
#[cfg(all(feature = "pdf", feature = "png"))]
fn huge_mermaid_dimensions_use_vector_pdf_and_bounded_bitmap_planning() {
    use merman::svg::{
        SvgPipeline,
        export::{DEFAULT_MAX_RASTER_PIXELS, RasterOptions},
    };

    let renderer = HeadlessRenderer::new();
    let pdf = renderer
        .render_pdf_sync(HUGE_VECTOR_SOURCE)
        .unwrap()
        .expect("XYChart should render to PDF");
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(
        String::from_utf8_lossy(&pdf).contains("100000"),
        "PDF should retain the intrinsic vector page size"
    );

    let svg = renderer
        .render_resvg_compatible_svg_with_pipeline_sync(
            HUGE_VECTOR_SOURCE,
            &SvgPipeline::resvg_safe(),
        )
        .unwrap()
        .expect("XYChart should render to sealed SVG");
    let plan = svg.raster_plan(&RasterOptions::default()).unwrap();

    assert_eq!(plan.requested_width_px, 100_000.0);
    assert_eq!(plan.requested_height_px, 100_000.0);
    assert!(plan.limited, "{plan:?}");
    assert!(
        u64::from(plan.width_px) * u64::from(plan.height_px) <= DEFAULT_MAX_RASTER_PIXELS,
        "{plan:?}"
    );
}

#[test]
#[cfg(feature = "png")]
fn raster_limits_apply_before_integer_encoder_dimensions() {
    use merman::svg::{
        SvgPipeline,
        export::{RasterOptions, RasterSizeLimit},
    };

    let svg = HeadlessRenderer::new()
        .render_resvg_compatible_svg_with_pipeline_sync(
            HUGE_VECTOR_SOURCE,
            &SvgPipeline::resvg_safe(),
        )
        .unwrap()
        .expect("large XYChart should remain valid vector SVG");
    let bounded = RasterOptions::default()
        .with_scale(50_000.0)
        .with_size_limit(RasterSizeLimit::new(Some(512), Some(512), Some(512 * 512)));
    let plan = svg.raster_plan(&bounded).unwrap();

    assert!(plan.requested_width_px > f64::from(u32::MAX), "{plan:?}");
    assert!(plan.requested_height_px > f64::from(u32::MAX), "{plan:?}");
    assert_eq!((plan.width_px, plan.height_px), (512, 512));
    assert!(plan.limited, "{plan:?}");

    let err = svg
        .raster_plan(
            &RasterOptions::default()
                .with_scale(50_000.0)
                .with_unbounded_size(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("u32 encoder capability"), "{err}");
}
