#![cfg(all(feature = "native-fonts", feature = "diagram-flowchart"))]

use merman::svg::SvgPipeline;
use merman::svg::export::{NativeFontContext, NativeFontOptions};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use merman_render::environment::{
    HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer, TextMeasurementOperation,
    TextMeasurementPhase,
};
use merman_render::text::{TextStyle, WrapMode};

const LABEL: &str = "iiii iiii iiii iiii iiii iiii";

fn context() -> NativeFontContext {
    NativeFontOptions::default()
        .with_font_file(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../merman-export/tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&OperationControl::new())
        .unwrap()
}

fn style() -> TextStyle {
    TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 16.0,
        font_weight: Some("400".to_owned()),
        font_style: Some("normal".to_owned()),
    }
}

fn length(context: &NativeFontContext, text: &str) -> f64 {
    match context
        .measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::ComputedLength,
            phase: TextMeasurementPhase::ComputedLength,
            text,
            style: &style(),
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        })
        .unwrap()
        .unwrap()
    {
        HostTextMeasurement::Length(value) => value,
        other => panic!("unexpected width: {other:?}"),
    }
}

fn render(context: &NativeFontContext, width: f64, html: bool, safe: bool) -> String {
    render_with_font_size(context, width, html, safe, Some(16.0))
}

fn render_with_font_size(
    context: &NativeFontContext,
    width: f64,
    html: bool,
    safe: bool,
    font_size: Option<f64>,
) -> String {
    let html_setting = if html {
        String::new()
    } else {
        "  htmlLabels: false\n".to_owned()
    };
    let theme_setting = font_size.map_or_else(String::new, |size| {
        format!("  themeVariables:\n    fontSize: {size}px\n")
    });
    let source = format!(
        "---\nconfig:\n  fontFamily: Cousine\n{theme_setting}{html_setting}  flowchart:\n    minNodeWidth: 0\n    wrappingWidth: {width}\n---\nflowchart LR\nA[{LABEL}]\n"
    );
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context.clone())
            .unwrap(),
        pipeline: safe.then(SvgPipeline::resvg_safe),
        ..Default::default()
    };
    match Renderer::new()
        .render(RenderRequest::svg(
            &source,
            OperationControl::new(),
            request,
        ))
        .unwrap()
    {
        RenderOutput::Svg(Some(output)) => output.into_parts().0,
        other => panic!("expected flowchart SVG: {other:?}"),
    }
}

fn class(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_whitespace().any(|part| part == name))
}

fn content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn shape<'doc, 'input>(
    document: &'doc roxmltree::Document<'input>,
) -> roxmltree::Node<'doc, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name("rect") && class(*node, "label-container"))
        .expect("flowchart node rectangle")
}

fn dimension(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name).unwrap().parse().unwrap()
}

fn texts(group: &usvg::Group, boxes: &mut Vec<usvg::Rect>) {
    for node in group.children() {
        match node {
            usvg::Node::Text(text) => boxes.push(text.abs_bounding_box()),
            usvg::Node::Group(group) => texts(group, boxes),
            _ => {}
        }
    }
}

fn assert_drawn_text_inside_node(svg: &str, context: &NativeFontContext) {
    let document = roxmltree::Document::parse(svg).unwrap();
    // Annotate the existing rectangle only to identify its unchanged geometry in usvg.
    let position = shape(&document).range().start + "<rect".len();
    let mut annotated = svg.to_owned();
    annotated.insert_str(position, " id=\"native-wrapping-shape\"");
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let tree = usvg::Tree::from_str(&annotated, &options).unwrap();
    let node = tree.node_by_id("native-wrapping-shape").unwrap();
    let bounds = node.abs_bounding_box();
    let mut boxes = Vec::new();
    texts(tree.root(), &mut boxes);
    assert!(!boxes.is_empty(), "the emitted label must actually shape");
    for text in boxes {
        let epsilon = 0.01;
        assert!(
            text.x() + epsilon >= bounds.x()
                && text.y() + epsilon >= bounds.y()
                && text.right() <= bounds.right() + epsilon
                && text.bottom() <= bounds.bottom() + epsilon,
            "drawn text {text:?} must fit inside node {bounds:?}"
        );
    }
}

#[test]
fn native_svg_rows_are_wrapped_using_the_font_that_is_actually_drawn() {
    let context = context();
    let width = length(&context, "iiii iiii") + length(&context, "i") * 0.5;
    let svg = render(&context, width, false, false);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let lines: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("tspan") && class(*node, "text-outer-tspan"))
        .map(content)
        .collect();
    assert_eq!(lines, ["iiii iiii", "iiii iiii", "iiii iiii"]);
    for line in &lines {
        assert!(length(&context, line) <= width);
        assert!(length(&context, &format!("{line} iiii")) > width);
    }
    assert_drawn_text_inside_node(&svg, &context);
}

#[test]
fn default_html_geometry_and_resvg_safe_rows_share_native_wrapping() {
    let context = context();
    let width = length(&context, "iiii iiii") + length(&context, "i") * 0.5;
    let parity = render(&context, width, true, false);
    let document = roxmltree::Document::parse(&parity).unwrap();
    let foreign = document
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("default labels preserve the HTML rendering contract");
    let div = foreign
        .descendants()
        .find(|node| node.has_tag_name("div"))
        .unwrap();
    assert!(
        parity.contains("font-size:16px"),
        "the drawn HTML font size must match the native probe"
    );
    assert_eq!(content(div), LABEL);
    let css = div.attribute("style").unwrap();
    assert!(
        css.contains("white-space: break-spaces"),
        "long HTML label must soft wrap: {css}"
    );
    let HostTextMeasurement::Metrics(metrics) = context
        .measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::Wrapped,
            phase: TextMeasurementPhase::Wrap,
            text: LABEL,
            style: &style(),
            max_width: Some(width),
            wrap_mode: WrapMode::HtmlLike,
        })
        .unwrap()
        .unwrap()
    else {
        panic!("expected native wrapped HTML metrics")
    };
    // CSS break-spaces retains each trailing space: two five-character segments cannot fit
    // this 9.5-character line, whereas the last segment has no trailing space.
    assert_eq!(metrics.line_count, 5);
    assert!(
        (dimension(foreign, "height") - metrics.height).abs() < 0.02,
        "foreign height={} native metrics={:?} foreign={:?}",
        dimension(foreign, "height"),
        metrics,
        foreign
    );
    assert!(
        (dimension(foreign, "width") - metrics.width).abs() < 0.02,
        "foreign width={} native metrics={:?} foreign={:?}",
        dimension(foreign, "width"),
        metrics,
        foreign
    );
    assert!(dimension(shape(&document), "height") >= metrics.height);
    assert!(dimension(shape(&document), "width") >= metrics.width);

    let safe = render(&context, width, true, true);
    let safe_document = roxmltree::Document::parse(&safe).unwrap();
    assert!(
        !safe_document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    let lines: Vec<_> = safe_document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text") && class(*node, "merman-foreignobject-fallback-text")
        })
        .map(content)
        .collect();
    assert_eq!(lines, ["iiii ", "iiii ", "iiii ", "iiii ", "iiii iiii"]);
    assert_eq!(lines.len(), metrics.line_count);
    assert_eq!(
        dimension(shape(&safe_document), "height"),
        dimension(shape(&document), "height")
    );
    assert_eq!(
        dimension(shape(&safe_document), "width"),
        dimension(shape(&document), "width")
    );
    assert_drawn_text_inside_node(&safe, &context);
}

#[test]
fn default_theme_html_metrics_follow_the_emitted_font_size() {
    let context = context();
    let width = length(&context, "iiii iiii") + length(&context, "i") * 0.5;
    let parity = render_with_font_size(&context, width, true, false, None);
    let document = roxmltree::Document::parse(&parity).unwrap();
    let root_id = document.root_element().attribute("id").unwrap();
    let root_selector = format!("#{root_id}");
    let declarations = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| css.split('}'))
        .find_map(|rule| {
            let (selector, declarations) = rule.split_once('{')?;
            (selector.trim() == root_selector).then_some(declarations)
        })
        .expect("the emitted SVG declares root typography");
    let font_size = declarations
        .split(';')
        .find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name.trim() == "font-size").then_some(value.trim())
        })
        .expect("root font size")
        .strip_suffix("px")
        .expect("CSS pixel font size")
        .parse::<f64>()
        .unwrap();
    assert!(font_size.is_finite() && font_size > 0.0);
    let effective_style = TextStyle {
        font_size,
        ..style()
    };
    let HostTextMeasurement::Metrics(metrics) = context
        .measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::Wrapped,
            phase: TextMeasurementPhase::Wrap,
            text: LABEL,
            style: &effective_style,
            max_width: Some(width),
            wrap_mode: WrapMode::HtmlLike,
        })
        .unwrap()
        .unwrap()
    else {
        panic!("expected effective-theme wrapped metrics")
    };
    assert!(metrics.line_count > 1);
    let foreign = document
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    assert!(
        (dimension(foreign, "height") - metrics.height).abs() < 0.02,
        "font-size={font_size}px, foreign height={}, native={metrics:?}",
        dimension(foreign, "height")
    );
    assert!(
        (dimension(foreign, "width") - metrics.width).abs() < 0.02,
        "font-size={font_size}px, foreign width={}, native={metrics:?}",
        dimension(foreign, "width")
    );

    let safe = render_with_font_size(&context, width, true, true, None);
    let safe_document = roxmltree::Document::parse(&safe).unwrap();
    let lines: Vec<_> = safe_document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text") && class(*node, "merman-foreignobject-fallback-text")
        })
        .map(content)
        .collect();
    assert_eq!(lines.len(), metrics.line_count);
    assert_eq!(
        lines.concat(),
        LABEL,
        "fallback must retain all spaces and text"
    );
    assert_eq!(
        dimension(shape(&safe_document), "height"),
        dimension(shape(&document), "height")
    );
    assert_eq!(
        dimension(shape(&safe_document), "width"),
        dimension(shape(&document), "width")
    );
    assert_drawn_text_inside_node(&safe, &context);
}
