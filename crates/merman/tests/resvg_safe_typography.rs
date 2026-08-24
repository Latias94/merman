#![cfg(feature = "svg")]

use merman::svg::{
    SvgPipeline, SvgRenderOptions, VendoredFontMetricsTextMeasurer,
    foreign_object_label_fallback_svg_text,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn render_resvg_safe(name: &str, source: &str) -> String {
    render_with_pipeline(name, source, Some(SvgPipeline::resvg_safe()))
}

fn render_with_pipeline(name: &str, source: &str, pipeline: Option<SvgPipeline>) -> String {
    let output = Renderer::new()
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest {
                options: SvgRenderOptions {
                    diagram_id: Some(name.to_string()),
                    ..Default::default()
                },
                pipeline,
                ..Default::default()
            },
        ))
        .unwrap_or_else(|error| panic!("{name}: render failed: {error}"));
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("{name}: no diagram detected");
    };
    svg.into_parts().0
}

fn fallback_text_style(svg: &str, label: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("resvg-safe output should be XML");
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("data-merman-foreignobject") == Some("fallback")
                })
                && node.text().is_some_and(|text| text.trim() == label)
        })
        .and_then(|node| node.attribute("style"))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("expected fallback text {label:?}: {svg}"))
}

fn assert_usvg_parseable(svg: &str) {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    usvg::Tree::from_str(svg, &options).expect("resvg-safe SVG should remain usvg-parseable");
}

fn usvg_fallback_text_font_size(svg: &str, label: &str) -> f32 {
    fn find_font_size(group: &usvg::Group, label: &str) -> Option<f32> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(size) = find_font_size(group, label) {
                        return Some(size);
                    }
                }
                usvg::Node::Text(text) => {
                    for chunk in text.chunks() {
                        if chunk.text().trim() == label {
                            return chunk.spans().first().map(|span| span.font_size().get());
                        }
                    }
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        None
    }

    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(svg, &options).expect("resvg-safe output should parse in usvg");
    find_font_size(tree.root(), label)
        .unwrap_or_else(|| panic!("expected usvg text span for {label:?}: {svg}"))
}

fn usvg_fallback_text_fill(svg: &str, label: &str) -> Option<(u8, u8, u8)> {
    fn find_fill(group: &usvg::Group, label: &str) -> Option<(u8, u8, u8)> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(fill) = find_fill(group, label) {
                        return Some(fill);
                    }
                }
                usvg::Node::Text(text) => {
                    for chunk in text.chunks() {
                        if chunk.text().trim() != label {
                            continue;
                        }
                        let span = chunk.spans().first()?;
                        let fill = span.fill()?;
                        if let usvg::Paint::Color(color) = fill.paint() {
                            return Some((color.red, color.green, color.blue));
                        }
                    }
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        None
    }

    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(svg, &options).expect("resvg-safe output should parse in usvg");
    find_fill(tree.root(), label)
}

#[test]
fn fallback_text_isolated_from_svg_only_source_selectors() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>g.classGroup text { font-size:10px !important; fill:#ebdbb2 !important; }</style><g class="classGroup"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><span>Alpha</span></div></foreignObject></g></svg>"##;
    let svg =
        foreign_object_label_fallback_svg_text(source, &VendoredFontMetricsTextMeasurer::default());

    assert_eq!(
        usvg_fallback_text_font_size(&svg, "Alpha"),
        16.0,
        "an SVG-only selector must not change the fallback size after measurement: {svg}"
    );
    assert_eq!(
        usvg_fallback_text_fill(&svg, "Alpha"),
        Some((0x33, 0x33, 0x33)),
        "an SVG-only selector must not change the fallback paint after resolution: {svg}"
    );
}

#[test]
fn class_diagram_fallback_keeps_source_context_typography() {
    let svg = render_resvg_safe(
        "resvg-typography-class",
        r#"classDiagram
    class User {
        +String id
        +String name
        +signIn()
    }"#,
    );

    for label in ["User", "+String name", "+signIn()"] {
        let style = fallback_text_style(&svg, label);
        assert!(
            style.contains("font-size: 16px") || style.contains("font-size:16px"),
            "{label:?} should use the source 16px metric: {style}"
        );
        assert!(
            !style.contains("font-size: 10px") && !style.contains("font-size:10px"),
            "{label:?} must not receive the SVG-only class text selector: {style}"
        );
        assert_eq!(
            usvg_fallback_text_font_size(&svg, label),
            16.0,
            "{label:?} must paint at 16px after usvg style resolution"
        );
    }
    assert_usvg_parseable(&svg);
}

#[test]
fn parity_pipeline_remains_separate_from_the_typography_adapter() {
    let source = r#"classDiagram
    class User {
        +String name
    }"#;
    let default_svg = render_with_pipeline("parity-boundary", source, None);
    let explicit_parity =
        render_with_pipeline("parity-boundary", source, Some(SvgPipeline::parity()));
    assert_eq!(
        default_svg, explicit_parity,
        "the adapter fix must not create a second parity output contract"
    );
    assert!(default_svg.contains("<foreignObject"), "{default_svg}");

    let resvg_safe = render_resvg_safe("parity-resvg-safe", source);
    assert!(!resvg_safe.contains("<foreignObject"), "{resvg_safe}");
}

#[test]
fn er_fallback_keeps_entity_and_relationship_selector_sizes_distinct() {
    let svg = render_resvg_safe(
        "resvg-typography-er",
        "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n",
    );

    let entity_style = fallback_text_style(&svg, "CUSTOMER");
    let relationship_style = fallback_text_style(&svg, "places");
    assert!(
        entity_style.contains("font-size: 16px") || entity_style.contains("font-size:16px"),
        "entity labels should retain the root metric: {entity_style}"
    );
    assert!(
        relationship_style.contains("font-size: 14px")
            || relationship_style.contains("font-size:14px"),
        "only the matching .edgeLabel .label context should use 14px: {relationship_style}"
    );
    assert_eq!(
        usvg_fallback_text_font_size(&svg, "places"),
        14.0,
        "ER relationship text must paint at its contextual 14px size"
    );
    assert_usvg_parseable(&svg);
}

#[test]
fn venn_fallback_inherits_the_real_text_area_presentation_size() {
    let svg = render_resvg_safe(
        "resvg-typography-venn",
        r#"%%{init: {"venn": {"width": 800, "height": 426}}}%%
venn-beta
  set A["Alpha"]:20
  set B["Beta"]:12
  text A1["React"]
  union A,B["Shared"]:3
"#,
    );

    let style = fallback_text_style(&svg, "React");
    assert!(
        style.contains("font-size: 20px") || style.contains("font-size:20px"),
        "Venn text nodes should inherit the .venn-text-area presentation size (20px at width 800): {style}"
    );
    assert_eq!(
        usvg_fallback_text_font_size(&svg, "React"),
        20.0,
        "Venn text must paint at the inherited 20px size"
    );
    assert_usvg_parseable(&svg);
}
