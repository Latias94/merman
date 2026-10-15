#![cfg(feature = "svg")]

use merman::svg::{
    DeterministicTextMeasurer, SvgPipeline, SvgRenderOptions,
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

fn fallback_content<'a, 'input>(
    instance: roxmltree::Node<'a, 'input>,
) -> roxmltree::Node<'a, 'input> {
    let reference = instance
        .children()
        .find(|node| node.has_tag_name("use"))
        .and_then(|node| node.attribute("href"))
        .expect("fallback instance references its definition");
    let id = reference
        .strip_prefix('#')
        .expect("same-document fallback reference");
    let definition = instance
        .document()
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .expect("fallback definition exists");
    assert!(
        definition
            .parent()
            .is_some_and(|node| node.has_tag_name("defs"))
    );
    definition
}

fn fallback_text_style(svg: &str, label: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("resvg-safe output should be XML");
    for instance in document
        .descendants()
        .filter(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
    {
        let content = fallback_content(instance);
        if let Some(text) = content.descendants().find(|node| {
            node.has_tag_name("text") && node.text().is_some_and(|text| text.trim() == label)
        }) {
            assert_eq!(
                text.attribute("class"),
                Some("merman-foreignobject-fallback-text")
            );
            assert_eq!(
                text.attribute("data-merman-source-classes"),
                instance.attribute("data-merman-source-classes"),
                "definition typography is associated with its original source instance"
            );
            assert!(instance.attribute("data-merman-source-classes").is_some());
            return text
                .attribute("style")
                .expect("explicit resolved typography")
                .to_string();
        }
    }
    panic!("expected fallback text {label:?}: {svg}")
}

fn assert_usvg_parseable(svg: &str) {
    let options = usvg_options_with_system_sans_serif();
    usvg::Tree::from_str(svg, &options).expect("resvg-safe SVG should remain usvg-parseable");
}

fn usvg_options_with_system_sans_serif() -> usvg::Options<'static> {
    const PREFERRED_FAMILIES: &[&str] = &[
        "DejaVu Sans",
        "Liberation Sans",
        "Noto Sans",
        "Arial",
        "Helvetica",
    ];

    let mut options = usvg::Options::default();
    let fallback_family = {
        let fontdb = options.fontdb_mut();
        fontdb.load_system_fonts();
        PREFERRED_FAMILIES
            .iter()
            .find_map(|preferred| {
                fontdb
                    .faces()
                    .flat_map(|face| face.families.iter())
                    .find(|(family, _)| family.eq_ignore_ascii_case(preferred))
                    .map(|(family, _)| family.clone())
            })
            .or_else(|| {
                fontdb
                    .faces()
                    .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
            })
    }
    .expect("usvg typography assertions require at least one system font");
    options.font_family = fallback_family.clone();
    options.fontdb_mut().set_sans_serif_family(fallback_family);
    options
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
                    let text_content = text
                        .chunks()
                        .iter()
                        .map(|chunk| chunk.text())
                        .collect::<String>();
                    if text_content.trim() == label {
                        return text
                            .chunks()
                            .iter()
                            .find_map(|chunk| chunk.spans().first())
                            .map(|span| span.font_size().get());
                    }
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        None
    }

    let options = usvg_options_with_system_sans_serif();
    let tree = usvg::Tree::from_str(svg, &options).expect("resvg-safe output should parse in usvg");
    find_font_size(tree.root(), label)
        .unwrap_or_else(|| panic!("expected usvg text span for {label:?}: {svg}"))
}

fn usvg_fallback_text_fill(svg: &str, label: &str) -> Option<(u8, u8, u8)> {
    fn find_flattened_fill(group: &usvg::Group) -> Option<(u8, u8, u8)> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(fill) = find_flattened_fill(group) {
                        return Some(fill);
                    }
                }
                usvg::Node::Path(path) => {
                    let Some(fill) = path.fill() else {
                        continue;
                    };
                    if fill.opacity().get() == 0.0 {
                        continue;
                    }
                    if let usvg::Paint::Color(color) = fill.paint() {
                        return Some((color.red, color.green, color.blue));
                    }
                }
                usvg::Node::Text(text) => {
                    if let Some(fill) = find_flattened_fill(text.flattened()) {
                        return Some(fill);
                    }
                }
                usvg::Node::Image(_) => {}
            }
        }
        None
    }

    fn find_fill(group: &usvg::Group, label: &str) -> Option<(u8, u8, u8)> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(fill) = find_fill(group, label) {
                        return Some(fill);
                    }
                }
                usvg::Node::Text(text) => {
                    let text_content = text
                        .chunks()
                        .iter()
                        .map(|chunk| chunk.text())
                        .collect::<String>();
                    if text_content.trim() != label {
                        continue;
                    }
                    return find_flattened_fill(text.flattened());
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        None
    }

    let options = usvg_options_with_system_sans_serif();
    let tree = usvg::Tree::from_str(svg, &options).expect("resvg-safe output should parse in usvg");
    find_fill(tree.root(), label)
}

#[test]
fn fallback_text_isolated_from_svg_only_source_selectors() {
    for filter in ["", r##" filter="url(#blur)""##] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs><style>g.classGroup text {{ font-size:10px !important; fill:#ebdbb2 !important; }}</style><g class="classGroup"{filter}><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><span>Alpha</span></div></foreignObject></g></svg>"##
        );
        let svg =
            foreign_object_label_fallback_svg_text(&source, &DeterministicTextMeasurer::default());

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
            style.contains("font-size: 14px") || style.contains("font-size:14px"),
            "{label:?} should use the source 14px metric: {style}"
        );
        assert!(
            !style.contains("font-size: 10px") && !style.contains("font-size:10px"),
            "{label:?} must not receive the SVG-only class text selector: {style}"
        );
        assert_eq!(
            usvg_fallback_text_font_size(&svg, label),
            14.0,
            "{label:?} must paint at 14px after usvg style resolution"
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
fn er_fallback_keeps_entity_and_relationship_context_typography() {
    let svg = render_resvg_safe(
        "resvg-typography-er",
        "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n",
    );

    let entity_style = fallback_text_style(&svg, "CUSTOMER");
    let relationship_style = fallback_text_style(&svg, "places");
    assert!(
        entity_style.contains("font-size: 14px") || entity_style.contains("font-size:14px"),
        "entity labels should retain the root 14px metric: {entity_style}"
    );
    assert!(
        relationship_style.contains("font-size: 14px")
            || relationship_style.contains("font-size:14px"),
        "the .edgeLabel .label context should use 14px: {relationship_style}"
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

#[test]
fn filtered_html_fallback_rejects_new_svg_only_visibility_rules() {
    for selector in [
        ".classGroup text",
        ".classGroup > g",
        ".classGroup [data-merman-foreignobject]",
        "text:not(.absent)",
    ] {
        for declaration in [
            "visibility:hidden !important",
            "display:none",
            "opacity:0",
            "filter:url(#blur)",
        ] {
            let source = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs><style>{selector} {{{declaration}}}</style><g class="classGroup" filter="url(#blur)"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml">Alpha</div></foreignObject></g></svg>"##
            );
            let out = foreign_object_label_fallback_svg_text(
                &source,
                &DeterministicTextMeasurer::default(),
            );
            let xml = roxmltree::Document::parse(&out).unwrap();
            let text = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Alpha"))
                .unwrap();
            assert!(
                !text
                    .ancestors()
                    .any(|node| node.attribute("class") == Some("classGroup")),
                "new CSS must not silently alter fallback paint: {out}"
            );
        }
    }
}

#[test]
fn filtered_html_fallback_does_not_assume_unknown_css_is_absent() {
    for rules in [
        "@media all { .classGroup text {visibility:hidden!important} }",
        "@supports(display:grid) { .classGroup text {opacity:0} }",
        ".classGroup text {filter:url(#blur)}",
    ] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><style>{rules}</style><g class="classGroup" filter="url(#blur)" transform="translate(100,200)"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml">Alpha</div></foreignObject></g></svg>"##
        );
        let out =
            foreign_object_label_fallback_svg_text(&source, &DeterministicTextMeasurer::default());
        let xml = roxmltree::Document::parse(&out).unwrap();
        let instance = xml
            .descendants()
            .find(|node| {
                node.attribute("data-merman-foreignobject") == Some("fallback")
                    && node.children().any(|child| child.has_tag_name("use"))
            })
            .unwrap();
        let text = fallback_content(instance)
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        let style = text.attribute("style").unwrap();
        assert!(style.contains("opacity: 1"), "{style}");
        assert!(style.contains("visibility: inherit"), "{style}");
        assert!(style.contains("filter: none"), "{style}");
        let source_owner = instance
            .ancestors()
            .find(|node| node.attribute("class") == Some("classGroup"))
            .unwrap();
        assert_eq!(
            source_owner.attribute("transform"),
            Some("translate(100,200)"),
            "the use instance remains under its source coordinate system"
        );
    }
}

#[test]
#[cfg(feature = "png")]
fn class_native_label_paint_reaches_usvg_and_png() {
    use merman::svg::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };

    let renderer = Renderer::new();
    for (body, label, target) in [
        (
            "namespace Internal {\nclass A\n}\nstyle A color:#000000\n",
            "Internal",
            ThemeTarget::Title,
        ),
        (
            "class A\nstyle A color:#000000\nA --() IService\n",
            "IService",
            ThemeTarget::NodeLabel,
        ),
    ] {
        let source = format!(
            "---\nconfig:\n  htmlLabels: false\n  layout: dagre\n---\nclassDiagram\n{body}"
        );
        for (paint, expected_fill) in [
            (CanvasPaint::solid("#ff0000").unwrap(), Some((255, 0, 0))),
            (CanvasPaint::Transparent, None),
        ] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                        ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint)),
                    )),
                )
                .unwrap();
            let RenderOutput::Svg(Some(svg)) = renderer
                .render(
                    RenderRequest::svg(&source, OperationControl::new(), SvgRequest::default())
                        .with_theme(theme.clone()),
                )
                .unwrap()
            else {
                panic!("Class native label must produce SVG");
            };
            assert_eq!(
                usvg_fallback_text_fill(svg.svg(), label),
                expected_fill,
                "native {label} fill"
            );

            let RenderOutput::Png(Some(png)) = renderer
                .render(
                    RenderRequest::png(
                        &source,
                        OperationControl::new(),
                        merman::PngRequest {
                            svg: SvgRequest::default(),
                            options: merman::svg::export::RasterOptions::default(),
                        },
                    )
                    .with_theme(theme),
                )
                .unwrap()
            else {
                panic!("Class native label must produce PNG");
            };
            let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            let has_red = pixels[..info.buffer_size()]
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[0] > 128 && pixel[1] < 64 && pixel[2] < 64);
            assert_eq!(has_red, expected_fill.is_some(), "native {label} pixels");
        }
    }
}

#[test]
#[cfg(feature = "png")]
fn filtered_html_fallback_reaches_native_glow_pixels_and_receipts() {
    let source = r##"<svg id="diagram" xmlns="http://www.w3.org/2000/svg" width="300" height="140" viewBox="0 0 300 140"><style>@keyframes dash {to {stroke-dashoffset:0}} #diagram :root {--mermaid-font-family:sans-serif} #diagram .label text {fill:#222;color:#222}</style><defs><filter id="label-theme-effect-glow" color-interpolation-filters="linearRGB" filterUnits="userSpaceOnUse" x="-20" y="-20" width="170" height="100"><feDropShadow in="SourceGraphic" dx="0" dy="0" stdDeviation="4" flood-color="#ff0080"/></filter></defs><g class="label" transform="translate(60,40)"><g filter="url(#label-theme-effect-glow)"><foreignObject width="120" height="48"><div xmlns="http://www.w3.org/1999/xhtml" style="font-family:sans-serif;font-size:20px;color:#222">Alpha</div></foreignObject></g></g></svg>"##;
    for (overflow, source, expected_receipt) in [
        ("hidden", source.to_string(), false),
        (
            "visible",
            source.replace(
                r#"<foreignObject width="120""#,
                r#"<foreignObject style="overflow:visible" width="120""#,
            ),
            true,
        ),
    ] {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let sealed = merman_render::svg::finalize_resvg_svg(&source, &session).unwrap();
        let document = roxmltree::Document::parse(sealed.as_str()).unwrap();
        let instance = document
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        let viewport = fallback_content(instance)
            .children()
            .find(|node| node.has_tag_name("svg"))
            .unwrap();
        assert!(
            viewport
                .attribute("style")
                .unwrap()
                .contains(&format!("overflow:{overflow}")),
            "{overflow}: source viewport clipping must survive finalization"
        );
        let (bytes, report) =
            merman::svg::export::svg_to_png_with_report(&sealed, &Default::default()).unwrap();
        let receipt = report.native_filter_receipt();
        // A clipped source viewport cannot prove the exporter's unclipped-glyph receipt.
        // Explicit visible overflow keeps the original glow and its stronger evidence.
        assert_eq!(receipt.is_some(), expected_receipt, "overflow={overflow}");
        if let Some(receipt) = receipt {
            assert_eq!(receipt.filter_count(), 1);
            assert_eq!(receipt.reference_count(), 1);
        }
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert!(
            pixels[..info.buffer_size()]
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[0] > pixel[1] && pixel[2] > pixel[1]),
            "overflow={overflow}: pink glow must reach native pixels"
        );
        #[cfg(feature = "pdf")]
        if expected_receipt {
            let (_, pdf_report) =
                merman::svg::export::svg_to_pdf_with_report(&sealed, &Default::default()).unwrap();
            assert_eq!(pdf_report.native_filter_receipt(), receipt);
        }
    }
}

#[test]
fn discretionary_hyphen_survives_usvg_glyph_conversion() {
    fn first_text_glyph_width(group: &usvg::Group) -> Option<f32> {
        group.children().iter().find_map(|node| match node {
            usvg::Node::Group(group) => first_text_glyph_width(group),
            usvg::Node::Text(text) => Some(text.flattened().bounding_box().width()),
            usvg::Node::Path(_) | usvg::Node::Image(_) => None,
        })
    }
    let measurer = DeterministicTextMeasurer::default().with_width_callback(|text, _| {
        text.chars().filter(|ch| *ch != '\u{00ad}').count() as f64 * 10.0
    });
    let source = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"120\" height=\"48\"><foreignObject width=\"60\" height=\"48\"><div style=\"white-space:break-spaces;width:60px\">alpha\u{00ad}beta</div></foreignObject></svg>";
    let svg = foreign_object_label_fallback_svg_text(source, measurer.as_ref());
    assert!(svg.contains(">alpha-</text>"), "{svg}");
    let options = usvg_options_with_system_sans_serif();
    let with_hyphen = usvg::Tree::from_str(&svg, &options).unwrap();
    let without_hyphen =
        usvg::Tree::from_str(&svg.replace(">alpha-</text>", ">alpha</text>"), &options).unwrap();
    assert!(
        first_text_glyph_width(with_hyphen.root()).unwrap()
            > first_text_glyph_width(without_hyphen.root()).unwrap(),
        "the selected hyphen must extend the actual glyph outline in the consumer"
    );
}
