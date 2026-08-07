#![cfg(feature = "svg")]

use merman::MermaidConfig;
use merman::svg::{HeadlessRenderer, RenderEnvironment, RenderResourcePolicy};
#[cfg(feature = "png")]
use std::io::Cursor;

fn render_svg(renderer: &HeadlessRenderer, name: &str, source: &str) -> String {
    renderer
        .render_svg_sync(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn render_resvg_safe(renderer: &HeadlessRenderer, name: &str, source: &str) -> String {
    renderer
        .render_resvg_compatible_svg_sync(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .map(merman::svg::ResvgCompatibleSvg::into_string)
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn assert_xml_parseable(name: &str, svg: &str) {
    roxmltree::Document::parse(svg)
        .unwrap_or_else(|err| panic!("{name}: output should be XML-parseable: {err}\n{svg}"));
}

#[cfg(any(feature = "png", feature = "pdf"))]
fn finalize_raster_input(svg: &str) -> merman::svg::ResvgCompatibleSvg {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    merman::svg::finalize_resvg_svg(svg, &session).expect("valid resvg-compatible fixture")
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
fn kanban_ticket_navigation_is_preserved_under_strict_mermaid_security() {
    let source = r#"---
config:
  kanban:
    ticketBaseUrl: 'https://mermaidchart.atlassian.net/browse/#TICKET#'
---
kanban
  Todo
    id4[Create parsing tests]@{ ticket: MC-2038 }
"#;
    let renderer = HeadlessRenderer::new().with_diagram_id("security-kanban-link");

    let parity = render_svg(&renderer, "security-kanban-link", source);
    let resvg_safe = render_resvg_safe(&renderer, "security-kanban-link-resvg", source);

    for svg in [&parity, &resvg_safe] {
        assert_xml_parseable("security-kanban-link-output", svg);
        assert!(
            svg.contains(r#"xlink:href="https://mermaidchart.atlassian.net/browse/MC-2038""#),
            "{svg}"
        );
        assert!(!svg.contains(r#"target="_blank""#), "{svg}");
    }
}

#[test]
fn kanban_strict_security_removes_an_unsafe_ticket_href_but_keeps_the_anchor() {
    let renderer = HeadlessRenderer::new()
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "strict",
            "kanban": {
                "ticketBaseUrl": "javascript:alert('#TICKET#')"
            }
        })))
        .with_diagram_id("security-kanban-strict");
    let source = "kanban\n  Todo\n    id4[Create parsing tests]@{ ticket: MC-2038 }\n";

    let svg = render_svg(&renderer, "security-kanban-strict", source);

    assert_xml_parseable("security-kanban-strict", &svg);
    assert!(svg.contains(r#"<a class="kanban-ticket-link">"#), "{svg}");
    assert!(!svg.contains("xlink:href"), "{svg}");
    assert!(!svg.contains(r#"target="_blank""#), "{svg}");
    assert!(!svg.to_ascii_lowercase().contains("javascript:"), "{svg}");
    assert!(!svg.contains("about:blank"), "{svg}");
    assert!(svg.contains("MC-2038"), "{svg}");
}

#[test]
fn kanban_loose_parity_and_resvg_safe_keep_separate_security_contracts() {
    let renderer = HeadlessRenderer::new()
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose",
            "kanban": {
                "ticketBaseUrl": "javascript:alert('#TICKET#')"
            }
        })))
        .with_diagram_id("security-kanban-loose");
    let source = "kanban\n  Todo\n    id4[Create parsing tests]@{ ticket: MC-2038 }\n";

    let parity = render_svg(&renderer, "security-kanban-loose-parity", source);
    let resvg_safe = render_resvg_safe(&renderer, "security-kanban-loose-resvg", source);

    assert!(parity.contains("javascript:alert("), "{parity}");
    assert!(parity.contains(r#"target="_blank""#), "{parity}");
    assert!(
        !resvg_safe.to_ascii_lowercase().contains("javascript:"),
        "{resvg_safe}"
    );
    assert!(resvg_safe.contains("MC-2038"), "{resvg_safe}");
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
fn resvg_safe_render_drops_xml_forbidden_control_chars_from_text() {
    let source = "C4Context\n\
title System Context\n\
Person(customer, \"Customer\", \"A\u{1f}customer\")\n\
System(system, \"System\", \"Does work\")\n\
Rel(customer, system, \"Uses\")\n";
    let renderer = HeadlessRenderer::new().with_diagram_id("security-xml-controls");

    let svg = render_resvg_safe(&renderer, "security-xml-controls", source);

    assert_xml_parseable("security-xml-controls", &svg);
    assert!(!svg.contains('\u{1f}'), "{svg}");
    assert!(svg.contains("Acustomer"), "{svg}");
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn mindmap_render_drops_xml_forbidden_control_chars_before_serialization() {
    let source = "mindmap\n  root((Root))\n    Parse\n    \u{1c}Layout\n";
    let renderer = HeadlessRenderer::new().with_diagram_id("security-mindmap-xml-controls");

    let parity_svg = render_svg(&renderer, "security-mindmap-xml-controls", source);
    assert_xml_parseable("security-mindmap-xml-controls", &parity_svg);
    assert!(!parity_svg.contains('\u{1c}'), "{parity_svg}");

    let resvg_svg = render_resvg_safe(&renderer, "security-mindmap-xml-controls", source);
    assert_xml_parseable("security-mindmap-resvg-xml-controls", &resvg_svg);
    assert!(!resvg_svg.contains('\u{1c}'), "{resvg_svg}");
}

#[test]
fn raw_svg_options_cannot_bypass_diagram_id_normalization() {
    let renderer = HeadlessRenderer::new().with_svg_options(merman::svg::SvgRenderOptions {
        diagram_id: Some("x]]>y".to_string()),
        ..Default::default()
    });

    let outputs = [
        render_svg(&renderer, "diagram-id-parity", "info"),
        renderer
            .render_svg_readable_sync("info")
            .expect("readable render")
            .expect("detected info diagram"),
        render_resvg_safe(&renderer, "diagram-id-resvg-safe", "info"),
    ];

    for (index, svg) in outputs.iter().enumerate() {
        assert_xml_parseable(&format!("diagram-id-output-{index}"), svg);
        assert!(svg.contains("x-y"), "normalized diagram id missing: {svg}");
        assert!(!svg.contains("x]]>y"), "raw diagram id survived: {svg}");
    }
}

#[test]
fn raw_resvg_safe_pipeline_strips_active_svg_content() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:q="http://www.w3.org/1999/xlink" xml:base="/tmp/" viewBox="0 0 16 16">
<script>alert(1)</script>
<a href="#safe"><use href="#shape" xlink:href="#shape"/></a>
<a href="https://example.com/docs"><text>docs</text></a>
<a href="javascript&colon;alert(1)" onclick="alert(1)"><text>bad</text></a>
<image href="data:image/png;base64,AAAA"/>
<image href="data:text/html;base64,PHNjcmlwdD4="/>
<image href="file:///etc/passwd"/>
<image href="/tmp/secret.png"/>
<image href="../secret.png"/>
<image q:href="https://example.com/remote.png"/>
<defs><path id="shape" d="M0 0H16V16H0z"/></defs>
<rect width="16" height="16" fill="black"/>
</svg>"##;

    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let out = merman::svg::finalize_resvg_svg(svg, &session)
        .unwrap()
        .into_string();

    assert_xml_parseable("raw-resvg-safe-active-content", &out);
    let lower = out.to_ascii_lowercase();
    assert!(!lower.contains("<script"), "{out}");
    assert!(!lower.contains("onclick"), "{out}");
    assert!(!lower.contains("javascript"), "{out}");
    assert!(!lower.contains("data:text/html"), "{out}");
    assert!(!lower.contains("file:///"), "{out}");
    assert!(!lower.contains("secret.png"), "{out}");
    assert!(!lower.contains("remote.png"), "{out}");
    assert!(!lower.contains("xml:base"), "{out}");
    assert!(out.contains(r##"href="#safe""##), "{out}");
    assert!(out.contains(r##"href="#shape""##), "{out}");
    assert!(out.contains(r##"xlink:href="#shape""##), "{out}");
    assert!(out.contains(r#"href="https://example.com/docs""#), "{out}");
    assert!(out.contains("data:image/png"), "{out}");
}

#[test]
fn resvg_safe_flowchart_images_cannot_delegate_file_or_network_io_to_the_host() {
    let source = r#"flowchart TD
    Local@{ img: "/tmp/merman-secret.png", label: "Local", h: 60 }
    Relative@{ img: "../merman-relative.png", label: "Relative", h: 60 }
    Remote@{ img: "https://example.com/merman-remote.png", label: "Remote", h: 60 }
    Local --> Relative --> Remote
"#;
    let renderer = HeadlessRenderer::new().with_diagram_id("security-image-resources");

    let parity = render_svg(&renderer, "security-image-resources-parity", source);
    assert!(parity.contains("/tmp/merman-secret.png"), "{parity}");
    assert!(parity.contains("../merman-relative.png"), "{parity}");
    assert!(
        parity.contains("https://example.com/merman-remote.png"),
        "{parity}"
    );

    let resvg_safe = render_resvg_safe(&renderer, "security-image-resources", source);
    assert_xml_parseable("security-image-resources", &resvg_safe);
    assert!(!resvg_safe.contains("merman-secret.png"), "{resvg_safe}");
    assert!(!resvg_safe.contains("merman-relative.png"), "{resvg_safe}");
    assert!(!resvg_safe.contains("merman-remote.png"), "{resvg_safe}");
    assert!(resvg_safe.contains(">Local<"), "{resvg_safe}");
    assert!(resvg_safe.contains(">Relative<"), "{resvg_safe}");
    assert!(resvg_safe.contains(">Remote<"), "{resvg_safe}");
}

#[test]
fn render_resource_limit_rejects_oversized_source() {
    let renderer = HeadlessRenderer::new().with_resource_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(merman::svg::ResourceLimitId::MaxSourceBytes, 8)
            .unwrap(),
    );

    let err = renderer
        .render_svg_sync("flowchart TD\nA --> B")
        .unwrap_err();

    assert!(err.to_string().contains("max_source_bytes"), "{err}");
}

#[test]
fn render_resource_limit_rejects_oversized_flowchart_model() {
    let renderer = HeadlessRenderer::new().with_resource_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(merman::svg::ResourceLimitId::MaxModelItems, 1)
            .unwrap(),
    );

    let err = renderer
        .render_svg_sync("flowchart TD\nA-->B\nB-->C")
        .unwrap_err();

    assert!(err.to_string().contains("max_model_items"), "{err}");
}

#[test]
fn resvg_safe_pipeline_strips_active_content_from_trusted_custom_icons() {
    let pack = br##"{
        "prefix":"test",
        "icons":{
            "active":{
                "body":"<script>alert(1)</script><path id=\"shape\" d=\"M0 0H16V16H0z\"/><use href=\"#shape\" onclick=\"alert(1)\"/><a href=\"javascript:alert(1)\"><path d=\"M1 1H2V2H1z\"/></a>"
            }
        }
    }"##;
    let registry = merman::svg::IconRegistry::from_packs([merman::svg::IconPack::new(pack)])
        .expect("valid Iconify pack");
    let renderer = HeadlessRenderer::new()
        .with_environment(RenderEnvironment::deterministic().with_icon_registry(registry))
        .with_svg_options(merman::svg::SvgRenderOptions {
            diagram_id: Some("security-icon".to_string()),
            ..Default::default()
        });
    let source = r#"flowchart TD
    A@{ icon: "test:active", label: "A" }
"#;

    let svg = render_resvg_safe(&renderer, "security-custom-icon", source);

    assert_xml_parseable("security-custom-icon", &svg);
    let lower = svg.to_ascii_lowercase();
    assert!(!lower.contains("<script"), "{svg}");
    assert!(!lower.contains("onclick"), "{svg}");
    assert!(!lower.contains("javascript:"), "{svg}");
    assert!(svg.contains("IconifyId"), "{svg}");
}

#[test]
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn repeated_maximum_icon_uses_one_svg_budget_across_svg_and_export_paths() {
    let maximum =
        usize::try_from(merman::svg::IconRegistryResourceLimitId::MaxBodyBytes.fixed_value())
            .expect("maximum icon body bytes fit usize");
    let prefix = r#"<path data-padding=""#;
    let suffix = r#"" d="M0 0H16V16H0z"/>"#;
    let body = format!(
        "{prefix}{}{suffix}",
        "x".repeat(maximum - prefix.len() - suffix.len())
    );
    assert_eq!(body.len(), maximum);
    let pack = format!(
        r#"{{"prefix":"test","icons":{{"max":{{"body":{}}}}}}}"#,
        serde_json::to_string(&body).unwrap()
    );
    let registry =
        merman::svg::IconRegistry::from_packs([merman::svg::IconPack::new(pack.as_bytes())])
            .expect("maximum body pack is admitted");
    let source = r#"flowchart TD
    A@{ icon: "test:max", label: "A" } --> B@{ icon: "test:max", label: "B" }
"#;

    let renderer = |budget: Option<usize>| {
        let mut policy = RenderResourcePolicy::unbounded_for_trusted_input();
        if let Some(budget) = budget {
            policy = policy
                .with_limit(merman::svg::ResourceLimitId::MaxSvgBytes, budget)
                .expect("positive SVG budget");
        }
        HeadlessRenderer::new()
            .with_environment(
                RenderEnvironment::deterministic().with_icon_registry(registry.clone()),
            )
            .with_resource_policy(policy)
            .with_diagram_id("maximum-icon-export")
    };

    let baseline = renderer(None)
        .render_svg_sync(source)
        .expect("unbounded parity render")
        .expect("flowchart detected");
    let mut low = 1usize;
    let mut high = baseline.len();
    assert!(
        renderer(Some(high))
            .render_resvg_compatible_svg_sync(source)
            .is_ok(),
        "the serialized baseline length must be a passing upper bound"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if renderer(Some(middle))
            .render_resvg_compatible_svg_sync(source)
            .is_ok()
        {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let exact_budget = low;
    assert!(exact_budget > 1);

    let exact = renderer(Some(exact_budget));
    exact
        .render_resvg_compatible_svg_sync(source)
        .expect("exact SVG budget")
        .expect("flowchart detected");
    let raster_options = merman::svg::export::RasterOptions::default();
    assert!(
        exact
            .render_png_sync(source, &raster_options)
            .expect("PNG exact SVG budget")
            .expect("flowchart detected")
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    assert!(
        exact
            .render_jpeg_sync(source, &raster_options)
            .expect("JPEG exact SVG budget")
            .expect("flowchart detected")
            .starts_with(b"\xff\xd8\xff")
    );
    assert!(
        exact
            .render_pdf_sync(source)
            .expect("PDF exact SVG budget")
            .expect("flowchart detected")
            .starts_with(b"%PDF-")
    );

    let one_less = renderer(Some(exact_budget - 1));
    let svg_error = one_less
        .render_resvg_compatible_svg_sync(source)
        .expect_err("SVG budget plus one must fail");
    assert!(
        svg_error.to_string().contains("max_svg_bytes"),
        "{svg_error}"
    );
    for error in [
        one_less
            .render_png_sync(source, &raster_options)
            .expect_err("PNG must share the SVG budget"),
        one_less
            .render_jpeg_sync(source, &raster_options)
            .expect_err("JPEG must share the SVG budget"),
        one_less
            .render_pdf_sync(source)
            .expect_err("PDF must share the SVG budget"),
    ] {
        assert!(error.to_string().contains("max_svg_bytes"), "{error}");
    }
}

#[test]
#[cfg(feature = "png")]
fn default_raster_plan_caps_large_viewbox_before_pixmap_allocation() {
    use merman::svg::export::{
        DEFAULT_MAX_RASTER_PIXELS, DEFAULT_MAX_RASTER_SIDE_LENGTH, RasterOptions, svg_raster_plan,
    };

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30000 20000"><rect width="30000" height="20000" fill="black"/></svg>"#;

    let svg = finalize_raster_input(svg);
    let plan = svg_raster_plan(&svg, &RasterOptions::default()).unwrap();

    assert_eq!(plan.requested_width_px, 30000.0);
    assert_eq!(plan.requested_height_px, 20000.0);
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
#[cfg(feature = "png")]
fn raster_size_limit_rejects_zero_budget_before_pixmap_allocation() {
    use merman::svg::export::{RasterOptions, RasterSizeLimit, svg_raster_plan};

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" fill="black"/></svg>"#;
    let options = RasterOptions::default().with_size_limit(RasterSizeLimit::new(
        Some(0),
        Some(128),
        Some(16_384),
    ));

    let svg = finalize_raster_input(svg);
    let err = svg_raster_plan(&svg, &options).unwrap_err();

    assert!(
        err.to_string()
            .contains("size_limit max_width and max_height must be positive"),
        "{err}"
    );
}

#[test]
#[cfg(feature = "png")]
fn custom_raster_size_limit_caps_actual_png_dimensions() {
    use merman::svg::export::{RasterOptions, RasterSizeLimit, svg_raster_plan, svg_to_png};

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30000 20000"><rect width="30000" height="20000" fill="black"/></svg>"#;
    let options = RasterOptions::default()
        .with_size_limit(RasterSizeLimit::new(Some(128), Some(128), Some(16_384)))
        .with_background("white");

    let svg = finalize_raster_input(svg);
    let plan = svg_raster_plan(&svg, &options).unwrap();
    let png = svg_to_png(&svg, &options).unwrap();

    assert!(plan.limited, "{plan:?}");
    assert!(plan.width_px <= 128, "{plan:?}");
    assert!(plan.height_px <= 128, "{plan:?}");
    assert!(u64::from(plan.width_px) * u64::from(plan.height_px) <= 16_384);
    assert_eq!(png_dimensions(&png), (plan.width_px, plan.height_px));
}

#[test]
#[cfg(feature = "pdf")]
fn default_pdf_conversion_keeps_large_vector_pages_outside_raster_pixel_limits() {
    use merman::svg::export::svg_to_pdf;

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30000 20000"><rect width="30000" height="20000" fill="black"/></svg>"#;

    let svg = finalize_raster_input(svg);
    let pdf = svg_to_pdf(&svg).unwrap();

    assert!(pdf.starts_with(b"%PDF-"));
    assert!(
        String::from_utf8_lossy(&pdf).contains("30000"),
        "the intrinsic vector page must not be downscaled by raster allocation policy"
    );
}

#[cfg(feature = "png")]
fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    let cursor = Cursor::new(bytes);
    let decoder = png::Decoder::new(cursor);
    let reader = decoder.read_info().expect("valid PNG header");
    let info = reader.info();
    (info.width, info.height)
}
