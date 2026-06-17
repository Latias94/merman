#![cfg(feature = "render")]

use merman::MermaidConfig;
use merman::render::HeadlessRenderer;

fn render_svg(renderer: &HeadlessRenderer, name: &str, source: &str) -> String {
    renderer
        .render_svg_sync(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn render_resvg_safe(renderer: &HeadlessRenderer, name: &str, source: &str) -> String {
    renderer
        .render_svg_resvg_safe_sync(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn assert_xml_parseable(name: &str, svg: &str) {
    roxmltree::Document::parse(svg)
        .unwrap_or_else(|err| panic!("{name}: output should be XML-parseable: {err}\n{svg}"));
}

#[test]
fn diagram_level_css_config_cannot_reach_effective_svg() {
    let source = r##"%%{init: {"themeCSS": ".node rect { outline: 13px solid rgb(1, 2, 3); }", "fontFamily": "x;a{b} :not(&){background:green !important} c{d}"}}%%
flowchart TD
    A[Start] --> B[Done]
"##;
    let renderer = HeadlessRenderer::new().with_diagram_id("security-config");

    let svg = render_svg(&renderer, "security-config", source);

    assert_xml_parseable("security-config", &svg);
    assert!(!svg.contains("outline: 13px"), "{svg}");
    assert!(!svg.contains("background:green"), "{svg}");
    assert!(!svg.contains("x;a{b}"), "{svg}");
}

#[test]
fn strict_click_javascript_url_does_not_emit_renderable_href() {
    let source = r#"flowchart TD
    A[Click me]
    click A "javascript:alert(1)" "bad" _blank
"#;
    let renderer = HeadlessRenderer::new().with_diagram_id("security-url");

    let svg = render_svg(&renderer, "security-url", source);

    assert_xml_parseable("security-url", &svg);
    assert!(!svg.to_ascii_lowercase().contains("javascript:"), "{svg}");
    assert!(!svg.contains(r#"xlink:href="about:blank""#), "{svg}");
}

#[test]
fn resvg_safe_pipeline_removes_loose_html_label_foreign_object() {
    let source = r#"flowchart TD
    A["<b onclick='alert(1)'>Hello</b><br/><img src=x onerror='alert(1)'>"] --> B[Done]
"#;
    let renderer = HeadlessRenderer::new()
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose",
            "flowchart": {
                "htmlLabels": true
            }
        })))
        .with_diagram_id("security-html-label");

    let svg = render_resvg_safe(&renderer, "security-html-label", source);

    assert_xml_parseable("security-html-label", &svg);
    let lower = svg.to_ascii_lowercase();
    assert!(!lower.contains("<foreignobject"), "{svg}");
    assert!(!lower.contains("onclick"), "{svg}");
    assert!(!lower.contains("onerror"), "{svg}");
    assert!(!lower.contains("<img"), "{svg}");
    assert!(svg.contains("Hello"), "{svg}");
}

#[test]
fn resvg_safe_pipeline_strips_trusted_theme_css_raster_hazards() {
    let source = "flowchart TD\n    A[Start] --> B[Done]";
    let renderer = HeadlessRenderer::new()
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeCSS": ".node rect { animation: pulse 1s infinite; } @keyframes pulse { to { opacity: 0.5; } } :root { --bad: 1; }"
        })))
        .with_diagram_id("security-resvg-css");

    let svg = render_resvg_safe(&renderer, "security-resvg-css", source);

    assert_xml_parseable("security-resvg-css", &svg);
    let lower = svg.to_ascii_lowercase();
    assert!(!lower.contains("@keyframes"), "{svg}");
    assert!(!lower.contains(":root"), "{svg}");
    assert!(!lower.contains("animation:"), "{svg}");
}

#[test]
#[cfg(feature = "raster")]
fn default_raster_plan_caps_large_viewbox_before_pixmap_allocation() {
    use merman::render::raster::{
        DEFAULT_MAX_RASTER_PIXELS, DEFAULT_MAX_RASTER_SIDE_LENGTH, RasterOptions, svg_raster_plan,
    };

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30000 20000"><rect width="30000" height="20000" fill="black"/></svg>"#;

    let plan = svg_raster_plan(svg, &RasterOptions::default()).unwrap();

    assert_eq!(plan.requested_width_px, 30000);
    assert_eq!(plan.requested_height_px, 20000);
    assert!(plan.limited, "{plan:?}");
    assert!(plan.effective_scale < plan.requested_scale, "{plan:?}");
    assert!(plan.width_px <= DEFAULT_MAX_RASTER_SIDE_LENGTH, "{plan:?}");
    assert!(plan.height_px <= DEFAULT_MAX_RASTER_SIDE_LENGTH, "{plan:?}");
    assert!(
        u64::from(plan.width_px) * u64::from(plan.height_px) <= DEFAULT_MAX_RASTER_PIXELS,
        "{plan:?}"
    );
}

#[test]
#[cfg(feature = "raster")]
fn custom_raster_size_limit_caps_actual_png_dimensions() {
    use merman::render::raster::{RasterOptions, RasterSizeLimit, svg_raster_plan, svg_to_png};

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30000 20000"><rect width="30000" height="20000" fill="black"/></svg>"#;
    let options = RasterOptions::default()
        .with_size_limit(RasterSizeLimit::new(Some(128), Some(128), Some(16_384)))
        .with_background("white");

    let plan = svg_raster_plan(svg, &options).unwrap();
    let png = svg_to_png(svg, &options).unwrap();

    assert!(plan.limited, "{plan:?}");
    assert!(plan.width_px <= 128, "{plan:?}");
    assert!(plan.height_px <= 128, "{plan:?}");
    assert!(u64::from(plan.width_px) * u64::from(plan.height_px) <= 16_384);
    assert_eq!(png_dimensions(&png), (plan.width_px, plan.height_px));
}

#[test]
#[cfg(feature = "raster")]
fn default_pdf_conversion_rejects_oversized_intrinsic_svg() {
    use merman::render::raster::{RasterOptions, svg_to_pdf_with_options};

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30000 20000"><rect width="30000" height="20000" fill="black"/></svg>"#;

    let err = svg_to_pdf_with_options(svg, &RasterOptions::default()).unwrap_err();

    assert!(
        err.to_string()
            .contains("PDF output exceeds configured size_limit"),
        "{err}"
    );
}

#[cfg(feature = "raster")]
fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    let decoder = png::Decoder::new(bytes);
    let reader = decoder.read_info().expect("valid PNG header");
    let info = reader.info();
    (info.width, info.height)
}
