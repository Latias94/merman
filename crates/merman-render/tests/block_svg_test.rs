mod common;

use base64::Engine as _;
use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use regex::Regex;

#[test]
fn native_theme_v2_block_preserves_authored_fill_and_marker_references() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            concat!(
                "---\nconfig:\n  themeVariables:\n    radius: 20\n---\n",
                "block\nA(\"Alpha\") --> B[\"Beta\"]\nstyle A fill:#123456\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.node":"#DDEEFF","surface.container":"#EAEDF0",
                         "stroke.border":"#405060"},
                "form":{"radius":6,"edge_style":"straight","connector_width":3,
                        "edge_label_bg":"pill","marker_style":"open"}
            }"##,
            None,
        )
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap();
    let svg = rendered.svg();
    let css = block_stylesheet_text(svg);
    assert!(css.contains("fill:#DDEEFF;stroke:#405060;"));
    assert!(css.contains(".flowchart-link{stroke-width:3px;}"));
    assert!(css.contains(".edgeLabel{background-color:#EAEDF0;"));

    let document = roxmltree::Document::parse(svg).unwrap();
    let authored = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .and_then(|node| node.children().find(|child| child.has_tag_name("rect")))
        .unwrap();
    assert_eq!(authored.attribute("rx"), Some("6"));
    assert!(
        authored
            .attribute("style")
            .unwrap()
            .contains("fill:#123456;")
    );
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    assert_eq!(
        edge.attribute("marker-end"),
        Some("url(#merman_block-pointEnd)")
    );
    let arrow = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman_block-pointEnd"))
        .and_then(|node| node.children().find(|child| child.has_tag_name("path")))
        .unwrap();
    assert!(arrow.attribute("style").unwrap().contains("fill:none;"));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_block_honors_native_styles_on_routed_edges() {
    let source = "---\nconfig:\n  layout: shoal\n---\nblock\ncolumns 3\nA[\"Alpha\"] X[\"Obstacle\"] B[\"Beta\"]\nA --> B\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    for (style, has_curves) in [("orthogonal", false), ("curved", true), ("sketch", true)] {
        let theme = ThemeCompiler::new()
            .compile_json(
                format!(
                    r#"{{"schema_version":2,"extends":"paper","form":{{"edge_style":"{style}"}}}}"#
                )
                .as_bytes(),
                None,
            )
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let rendered = family::prepare(
            parsed.presentation(),
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let edge = doc
            .descendants()
            .find(|node| node.attribute("data-et") == Some("edge"))
            .unwrap();
        assert_eq!(
            edge.attribute("d").unwrap().contains('C'),
            has_curves,
            "{style}: {edge:?}"
        );
    }
}

#[test]
fn block_digit_prefixed_classes_are_css_identifiers() {
    let svg = render_block_svg_from_text(
        "block\nA[\"Alpha\"]\nclassDef 2done fill:#123456\nclass A 2done\n",
    );
    assert!(block_stylesheet_text(&svg).contains(r#".\32 done"#));
    assert!(svg.contains(" 2done"));
}

fn independent_block_presentation(
    model: merman_model::families::block::BlockDiagramModel,
) -> merman_model::presentation::PresentationDiagram {
    use merman_model::OperationControl;
    use merman_model::families::block::BlockDiagramRenderModel;
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    use merman_model::resources::InputResourcePolicy;

    let config = merman_model::MermaidConfig::empty_object();
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "block".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
        },
        RenderSemanticModel::Block(BlockDiagramRenderModel::from_semantic_model(model)),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("independent Block construction is active")
    .expect("small Block model passes construction resource admission")
}

fn independent_block_tree(
    children: &[&[usize]],
) -> merman_model::families::block::BlockDiagramModel {
    use merman_model::families::block::{BlockDiagramModel, BlockNode};

    BlockDiagramModel {
        root: Some(0),
        blocks_flat: children
            .iter()
            .enumerate()
            .map(|(index, children)| BlockNode {
                id: if index == 0 {
                    "root".to_string()
                } else {
                    format!("node{index}")
                },
                label: "Repeated label".to_string(),
                block_type: if children.is_empty() {
                    "square".to_string()
                } else {
                    "composite".to_string()
                },
                children: children.to_vec(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn block_direct_input_rejects_repeated_child_indices_before_layout_expansion() {
    for model in [
        independent_block_tree(&[&[1, 1], &[]]),
        independent_block_tree(&[&[1, 2], &[3], &[3], &[]]),
    ] {
        let parsed = independent_block_presentation(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&parsed, &LayoutOptions::default(), session)
            .err()
            .expect("a flat child record cannot own two layout occurrences");
        assert!(matches!(
            error,
            merman_render::Error::InvalidModel { ref message }
                if message.contains("repeated Block child index")
        ));
    }
}

#[test]
fn block_direct_input_rejects_invalid_column_spans() {
    for second_span in [-1.0, 1.5, f64::NAN, f64::NEG_INFINITY] {
        let mut model = independent_block_tree(&[&[1, 2], &[], &[]]);
        model.blocks_flat[1].width_in_columns = Some(1.0);
        model.blocks_flat[2].width_in_columns = Some(second_span);
        let parsed = independent_block_presentation(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&parsed, &LayoutOptions::default(), session)
            .err()
            .expect("invalid direct spans must fail before grid arithmetic");
        assert!(matches!(
            error,
            merman_render::Error::InvalidModel { ref message }
                if message.contains("Block column span")
        ));
    }
}

#[test]
fn block_direct_input_preserves_distinct_records_and_supported_column_spans() {
    for (first_span, second_span) in [(1.0, 3.0), (9223372036854775808.0, 1.0)] {
        let mut model = independent_block_tree(&[&[1, 2], &[], &[]]);
        model.blocks_flat[1].id = "Repeated".to_string();
        model.blocks_flat[2].id = "Repeated".to_string();
        model.blocks_flat[1].width_in_columns = Some(first_span);
        model.blocks_flat[2].width_in_columns = Some(second_span);
        let parsed = independent_block_presentation(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("distinct flat records and JavaScript Number spans remain valid");
        let projection = artifact.layout_json().expect("project finite Block layout");
        let layout: merman_render::model::BlockDiagramLayout =
            serde_json::from_value(projection["layout"]["BlockDiagram"].clone()).unwrap();
        assert_eq!(layout.nodes.len(), 2);
        for node in &layout.nodes {
            assert_eq!(node.id, "Repeated");
            assert!(
                [node.x, node.y, node.width, node.height]
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }
}

#[test]
fn block_direct_input_validation_preserves_operation_error_priority() {
    use merman_model::OperationControl;

    let parsed = independent_block_presentation(independent_block_tree(&[&[1, 1], &[]]));
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_))
    ));

    let control = OperationControl::new();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let error = family::plan_render(&parsed, &session).err().unwrap();
    let merman_render::Error::ResourceLimitExceeded(first) = error else {
        panic!("expected model resource admission before Block validation, got {error:?}");
    };
    assert_eq!(first.limit, ResourceLimitId::MaxModelItems.as_str());
    control.cancel();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        merman_render::Error::ResourceLimitExceeded(replayed) if replayed == first
    ));
}

#[test]
fn block_large_columns_preserve_the_auto_grid_without_integer_rejection() {
    let automatic = render_block_svg_from_text("block\ncolumns auto\nA\nB\n");
    for columns in [
        "9007199254740993",
        "9223372036854775808",
        "18446744073709551615",
    ] {
        let svg = render_block_svg_from_text(&format!("block\ncolumns {columns}\nA\nB\n"));
        assert_eq!(svg, automatic, "{columns}");
    }
}

#[test]
fn block_zero_and_overflowing_counts_keep_the_upstream_render_stage() {
    let engine = MermaidEngine::new();
    for columns in ["0".to_string(), "9".repeat(309)] {
        let error = try_render_block_svg_from_text_with_engine(
            &engine,
            &format!("block\ncolumns {columns}\nA\n"),
        )
        .expect_err("the selected layout rejects zero and nonfinite columns");
        assert!(
            matches!(error, merman_render::Error::InvalidModel { ref message } if message == "Columns must be an integer !== 0.")
        );
        try_render_block_svg_from_text_with_engine(&engine, &format!("block\ncolumns {columns}\n"))
            .expect("an empty composite never calculates a column position");
    }
    let token = "9".repeat(309);
    let error =
        try_render_block_svg_from_text_with_engine(&engine, &format!("block\nA:{token}\nB\n"))
            .expect_err("a subsequent position becomes nonfinite");
    assert!(
        matches!(error, merman_render::Error::InvalidModel { ref message } if message == "Position must be a non-negative integer.Infinity")
    );
    for source in [
        "block\nA:0\nB\n".to_string(),
        "block\nA{{\"A\"}}:0\nB\n".to_string(),
        "block\nA[(\"A\")]:0\nB\n".to_string(),
        format!("block\nA:{token}\n"),
        format!("block\ncolumns 2\nA:{token}\nB\n"),
    ] {
        let error = try_render_block_svg_from_text_with_engine(&engine, &source)
            .expect_err("nonfinite layout must retain the shared finite-root SVG contract");
        assert!(
            matches!(error, merman_render::Error::InvalidModel { ref message }
            if message.starts_with("root SVG viewBox ") && message.contains("must be finite")),
            "{source}"
        );
    }
}

fn render_block_svg_from_text(text: &str) -> String {
    let engine = MermaidEngine::new();
    render_block_svg_from_text_with_engine(&engine, text)
}

fn render_block_svg_from_text_with_engine(engine: &MermaidEngine, text: &str) -> String {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
    .expect("svg render ok")
}

fn try_render_block_svg_from_text_with_engine(
    engine: &MermaidEngine,
    text: &str,
) -> merman_render::Result<String> {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
}

fn try_render_block_svg_from_text_with_engine_and_policy(
    engine: &MermaidEngine,
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

fn translated_center(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let transform = node.attribute("transform").expect("node transform");
    let captures = Regex::new(
        r"^translate\(\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*\)$",
    )
    .expect("valid transform regex")
    .captures(transform)
    .expect("translate(x, y) transform");
    (
        captures[1].parse().expect("numeric translate x"),
        captures[2].parse().expect("numeric translate y"),
    )
}

fn path_start(path: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let d = path.attribute("d").expect("path data");
    let captures =
        Regex::new(r"^M\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))")
            .expect("valid path regex")
            .captures(d)
            .expect("path starts with an absolute move");
    (
        captures[1].parse().expect("numeric path x"),
        captures[2].parse().expect("numeric path y"),
    )
}

fn root_view_box(document: &roxmltree::Document<'_>) -> (f64, f64, f64, f64) {
    let root = document.root_element();
    let values = root
        .attribute("viewBox")
        .expect("root viewBox")
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "viewBox must contain four components");
    (values[0], values[1], values[2], values[3])
}

fn render_block_geometry(config: serde_json::Value, source: &str) -> (String, serde_json::Value) {
    let engine = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Block geometry")
        .expect("detect Block geometry")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Block geometry");
    let layout =
        artifact.layout_json().expect("project Block geometry")["layout"]["BlockDiagram"].clone();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Block geometry")
        .svg()
        .to_owned();
    (svg, layout)
}

fn block_node_rect<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    node_id: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .expect("Block node")
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("Block rectangle terminal")
}

fn block_shape_geometry<'a>(layout: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    layout["shapeGeometries"]
        .as_array()
        .expect("Block shape geometries")
        .iter()
        .find(|geometry| geometry["id"] == id)
        .expect("Block node geometry")
}

fn block_stylesheet_text(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("Block stylesheet XML");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block logical stylesheet")
        .to_owned()
}

// Check only Block's generated class selectors against the actual label DOM. These fixtures
// have one source color, so no general stylesheet parser or inferred ancestor cascade is needed.
fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .unwrap_or_else(|| panic!("missing {name} attribute"))
        .parse()
        .unwrap_or_else(|_| panic!("numeric {name} attribute"))
}

fn matched_block_node_stroke_width(document: &roxmltree::Document<'_>, id: &str) -> String {
    let shell = block_node_rect(document, id);
    assert!(shell.ancestors().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "node"))
    }));
    let selector = format!("#merman .node {}", shell.tag_name().name());
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block stylesheet");
    css.split('}')
        .filter_map(|rule| rule.split_once('{'))
        .find(|(selectors, _)| selectors.split(',').any(|part| part.trim() == selector))
        .expect("CSS rule matching the actual Block shell")
        .1
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .find(|(property, _)| property.trim() == "stroke-width")
        .expect("matched node stroke-width declaration")
        .1
        .trim()
        .to_owned()
}

fn parse_translate(raw: &str) -> (f64, f64) {
    let captures = Regex::new(
        r"^translate\(\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*\)$",
    )
    .expect("valid transform regex")
    .captures(raw)
    .expect("translate(x, y) transform");
    (
        captures[1].parse().expect("numeric translate x"),
        captures[2].parse().expect("numeric translate y"),
    )
}

fn global_foreign_object_bounds(foreign_object: roxmltree::Node<'_, '_>) -> (f64, f64, f64, f64) {
    let width = foreign_object
        .attribute("width")
        .expect("foreignObject width")
        .parse::<f64>()
        .expect("numeric foreignObject width");
    let height = foreign_object
        .attribute("height")
        .expect("foreignObject height")
        .parse::<f64>()
        .expect("numeric foreignObject height");
    let x = foreign_object
        .attribute("x")
        .unwrap_or("0")
        .parse::<f64>()
        .expect("numeric foreignObject x");
    let y = foreign_object
        .attribute("y")
        .unwrap_or("0")
        .parse::<f64>()
        .expect("numeric foreignObject y");

    let (mut tx, mut ty) = (x, y);
    for ancestor in foreign_object.ancestors().filter(|node| node.is_element()) {
        if let Some(transform) = ancestor.attribute("transform") {
            let (ancestor_x, ancestor_y) = parse_translate(transform);
            tx += ancestor_x;
            ty += ancestor_y;
        }
    }
    (tx, ty, tx + width, ty + height)
}

fn global_path_geometry_bounds(path: roxmltree::Node<'_, '_>) -> (f64, f64, f64, f64) {
    let d = path.attribute("d").expect("path data");
    let fragment = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{d}"/></svg>"#);
    let bounds = merman_render::svg::debug_svg_emitted_bounds(&fragment)
        .expect("path geometry bounds")
        .bounds;
    let (mut tx, mut ty) = (0.0, 0.0);
    for ancestor in path.ancestors().filter(|node| node.is_element()) {
        if let Some(transform) = ancestor.attribute("transform") {
            let (ancestor_x, ancestor_y) = parse_translate(transform);
            tx += ancestor_x;
            ty += ancestor_y;
        }
    }
    (
        bounds.min_x + tx,
        bounds.min_y + ty,
        bounds.max_x + tx,
        bounds.max_y + ty,
    )
}

fn deep_block_chain(depth: usize) -> String {
    let mut input = String::from("block\n");
    for level in 0..depth {
        input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
    }
    input.push_str("leaf[\"leaf\"]\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

fn terminal_shell_styles(svg: &str, node_id: &str) -> Vec<(String, String)> {
    let document = roxmltree::Document::parse(svg).expect("valid Block SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Block node {node_id}: {svg}"));

    node.descendants()
        .filter_map(|shell| {
            if !shell.is_element() {
                return None;
            }
            let tag = shell.tag_name().name();
            matches!(tag, "rect" | "circle" | "path" | "polygon")
                .then(|| shell.attribute("style").map(|style| (tag, style)))
                .flatten()
        })
        .map(|(tag, style)| (tag.to_string(), style.to_string()))
        .collect()
}

#[test]
fn block_link_tokens_reach_svg_markers_and_line_classes() {
    for (token, start, end, thickness, pattern) in [
        ("-->", false, true, "normal", "solid"),
        ("<-->", true, true, "normal", "solid"),
        ("<==>", true, true, "thick", "solid"),
        ("==>", false, true, "thick", "solid"),
        ("-.->", false, true, "normal", "dotted"),
        ("===", false, false, "thick", "solid"),
        ("-.-", false, false, "normal", "dotted"),
    ] {
        let svg = render_block_svg_from_text(&format!("block\nA{token}B\n"));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let path = document
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        let classes = path
            .attribute("class")
            .unwrap()
            .split_ascii_whitespace()
            .collect::<Vec<_>>();
        assert!(
            classes.contains(&format!("edge-thickness-{thickness}").as_str()),
            "{token}"
        );
        assert!(
            classes.contains(&format!("edge-pattern-{pattern}").as_str()),
            "{token}"
        );
        for (attribute, expected) in [("marker-start", start), ("marker-end", end)] {
            let reference = path.attribute(attribute);
            assert_eq!(reference.is_some(), expected, "{token} {attribute}");
            if let Some(reference) = reference {
                let id = reference
                    .strip_prefix("url(#")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                assert!(document.descendants().any(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id)));
            }
        }
    }
}

#[test]
fn block_spaced_class_targets_keep_attached_node_identity() {
    let svg = render_block_svg_from_text("block\nA[\"Alpha\"] B[\"Beta\"]\nclass A, B hot\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let nodes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "node")
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    let classes = |node: roxmltree::Node<'_, '_>| {
        node.attribute("class")
            .unwrap()
            .split_ascii_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert!(classes(nodes[0]).iter().any(|class| class == "hot"));
    assert!(!classes(nodes[0]).iter().any(|class| class == "B"));
    assert!(!classes(nodes[1]).iter().any(|class| class == "hot"));
    assert!(
        nodes[1]
            .descendants()
            .any(|node| node.text() == Some("Beta"))
    );
}

#[test]
fn block_detached_database_records_do_not_require_visible_html_labels() {
    for source in ["block\nA space B\n", "block\nA\nclass missing hot\n"] {
        let svg = render_block_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(document.descendants().any(|node| node.text() == Some("A")));
    }
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("stroke"))
        .map(|(_, value)| value.trim())
        .next_back()
}

fn terminal_fill(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("fill"))
        .map(|(_, value)| value.trim())
        .next_back()
}

#[test]
fn block_svg_scopes_text_and_edge_colors_for_html_labels() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        !svg.contains("<style></style>"),
        "expected block SVG to emit scoped CSS instead of an empty style element"
    );
    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#333;color:#333;}"#),
        "expected block HTML/SVG labels to avoid inheriting host page text color"
    );
    assert!(
        svg.contains(r#"#merman .flowchart-link{stroke:#333333;fill:none;}"#),
        "expected block edges to carry their scoped stroke color"
    );
}

#[test]
fn block_public_svg_render_handles_deep_chain() {
    const DEPTH: usize = 1200;
    let svg = try_render_block_svg_from_text_with_engine_and_policy(
        &MermaidEngine::new(),
        &deep_block_chain(DEPTH),
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("deep Block SVG render");

    assert!(
        svg.contains(r#"id="merman-leaf""#),
        "expected deep Block leaf to render without stack-dependent traversal"
    );
}

#[test]
fn block_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"block
  A["Alpha"] --> B["Beta"]
  classDef branded fill:#696,stroke:#333
  class A branded
"#;
    let engine = MermaidEngine::new();
    let baseline = try_render_block_svg_from_text_with_engine_and_policy(
        &engine,
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Block baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Block fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Block SVG byte ceiling");
    let exact =
        try_render_block_svg_from_text_with_engine_and_policy(&engine, source, exact_policy)
            .expect("the exact Block family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Block SVG byte ceiling");
    let error =
        try_render_block_svg_from_text_with_engine_and_policy(&engine, source, below_policy)
            .expect_err("one byte below the Block family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Block MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn block_svg_honors_visible_edge_stroke_width_theme() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233"}}}%%
block
  A --> B
"##,
    );

    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected shared Mermaid edge thickness CSS to reach visible Block edges: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edgePaths .path{stroke:#112233;stroke-width:2.0px;}"#),
        "expected Block edge-path CSS to use Mermaid's edgePaths contract: {svg}"
    );
    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1""#),
        "expected Block edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn block_node_stroke_width_matches_its_terminal_css_for_classic_and_neo() {
    for look in ["classic", "neo"] {
        for width in [0, 1, 4] {
            let (svg, _) = render_block_geometry(
                serde_json::json!({"look": look, "themeVariables": {"strokeWidth": width}}),
                "block\n A[\"A\"]\n",
            );
            let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
            assert_eq!(
                matched_block_node_stroke_width(&document, "merman-A"),
                format!("{width}px"),
                "{look} node terminal must consume the configured stroke width"
            );
        }
    }
}

#[test]
fn block_root_viewbox_contains_configured_node_stroke_envelope() {
    for width in [0.0, 4.0, 24.0] {
        let (svg, _) = render_block_geometry(
            serde_json::json!({"themeVariables": {"strokeWidth": width}}),
            "block\n A[\"A\"]\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
        let rect = block_node_rect(&document, "merman-A");
        let (center_x, center_y) = translated_center(rect.parent().unwrap());
        let min_x = center_x + numeric_attribute(rect, "x");
        let min_y = center_y + numeric_attribute(rect, "y");
        let max_x = min_x + numeric_attribute(rect, "width");
        let max_y = min_y + numeric_attribute(rect, "height");
        let (view_x, view_y, view_width, view_height) = root_view_box(&document);
        let margin = width / 2.0 + 5.0;
        assert!(
            view_x <= min_x - margin + 1e-6
                && view_y <= min_y - margin + 1e-6
                && view_x + view_width >= max_x + margin - 1e-6
                && view_y + view_height >= max_y + margin - 1e-6,
            "stroke {width} envelope plus Mermaid viewport padding must fit: {svg}"
        );
    }
}

#[test]
fn block_square_padding_uses_look_while_grid_spacing_uses_block_padding() {
    for look in ["classic", "neo"] {
        for padding in [8.0, 20.0] {
            for source in [
                "block\n A[\"A\"]\n",
                "block\n columns 2\n A[\"A\"] B[\"Longer\"]\n",
            ] {
                let (svg, layout) = render_block_geometry(
                    serde_json::json!({"look": look, "block": {"padding": padding}}),
                    source,
                );
                let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
                let nodes = layout["nodes"].as_array().expect("layout nodes");
                let max_label_width = nodes
                    .iter()
                    .map(|node| {
                        let id = node["id"].as_str().unwrap();
                        let rect = block_node_rect(&document, &format!("merman-{id}"));
                        let label = rect
                            .parent()
                            .unwrap()
                            .descendants()
                            .find(|node| node.has_tag_name("foreignObject"))
                            .expect("Block HTML label");
                        numeric_attribute(label, "width")
                    })
                    .fold(0.0, f64::max);
                for node in nodes {
                    let id = node["id"].as_str().unwrap();
                    let rect = block_node_rect(&document, &format!("merman-{id}"));
                    let geometry = block_shape_geometry(&layout, id);
                    let shell_x_padding = if look == "neo" { 32.0 } else { 4.0 * padding };
                    let shell_y_padding = if look == "neo" { 24.0 } else { 2.0 * padding };
                    let expected_width = max_label_width + shell_x_padding;
                    let label = rect
                        .parent()
                        .unwrap()
                        .descendants()
                        .find(|node| node.has_tag_name("foreignObject"))
                        .expect("Block HTML label");
                    let expected_height = numeric_attribute(label, "height") + shell_y_padding;
                    assert!((numeric_attribute(rect, "width") - expected_width).abs() <= 1e-6);
                    assert!((numeric_attribute(rect, "height") - expected_height).abs() <= 1e-6);
                    assert_eq!(geometry["boundary"]["width"].as_f64(), Some(expected_width));
                    assert_eq!(
                        geometry["boundary"]["height"].as_f64(),
                        Some(expected_height)
                    );
                    assert_eq!(
                        geometry["allocated"]["width"].as_f64(),
                        Some(expected_width)
                    );
                    assert_eq!(
                        geometry["allocated"]["height"].as_f64(),
                        Some(expected_height)
                    );
                }
                if nodes.len() == 2 {
                    let a = block_node_rect(&document, "merman-A");
                    let b = block_node_rect(&document, "merman-B");
                    let (ax, _) = translated_center(a.parent().unwrap());
                    let (bx, _) = translated_center(b.parent().unwrap());
                    let gap = bx
                        - ax
                        - numeric_attribute(a, "width") / 2.0
                        - numeric_attribute(b, "width") / 2.0;
                    assert!((gap - padding).abs() <= 1e-6, "{look} grid gap: {gap}");
                }
            }
        }
    }
}

#[test]
fn block_round_radius_preserves_mermaid_truthiness_in_svg_and_geometry() {
    for (radius, expected) in [
        (None, 5.0),
        (Some(serde_json::json!(12)), 12.0),
        (Some(serde_json::json!(0)), 5.0),
        (Some(serde_json::json!("0")), 0.0),
        (Some(serde_json::Value::Null), 5.0),
    ] {
        let mut config = serde_json::json!({});
        if let Some(radius) = radius {
            config["themeVariables"] = serde_json::json!({"radius": radius});
        }
        let (svg, layout) = render_block_geometry(config, "block\n A(\"A\")\n");
        let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
        let rect = block_node_rect(&document, "merman-A");
        assert_eq!(numeric_attribute(rect, "rx"), expected);
        assert_eq!(numeric_attribute(rect, "ry"), expected);
        assert_eq!(
            block_shape_geometry(&layout, "A")["boundary"]["radius"].as_f64(),
            Some(expected)
        );
    }
}

#[test]
fn block_round_diagonal_edge_uses_rectangular_routing_with_any_paint_radius() {
    for radius in [5, 12] {
        let (svg, layout) = render_block_geometry(
            serde_json::json!({"themeVariables": {"radius": radius}}),
            "block-beta\n columns 2\n A(\"A\") B[\"B\"]\n space C[\"C\"]\n A --- C\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
        let a = block_node_rect(&document, "merman-A");
        let layout_edge = layout["edges"]
            .as_array()
            .expect("Block layout edges")
            .iter()
            .find(|edge| edge["from"] == "A" && edge["to"] == "C")
            .expect("A to C semantic edge");
        let data_id = format!(
            "{}-{}",
            document.root_element().attribute("id").expect("diagram id"),
            layout_edge["id"].as_str().expect("semantic edge id")
        );
        let edge = document
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-et") == Some("edge")
                    && node.attribute("data-id") == Some(data_id.as_str())
            })
            .expect("markerless A to C edge");
        assert!(edge.attribute("marker-start").is_none());
        assert!(edge.attribute("marker-end").is_none());
        let points = base64::engine::general_purpose::STANDARD
            .decode(edge.attribute("data-points").expect("raw edge points"))
            .expect("base64 raw edge points");
        let points: Vec<merman_render::model::LayoutPoint> =
            serde_json::from_slice(&points).expect("raw edge point JSON");
        let (center_x, center_y) = translated_center(a.parent().unwrap());
        let dx = points[0].x - center_x;
        let dy = points[0].y - center_y;
        let half_width = numeric_attribute(a, "width") / 2.0;
        let half_height = numeric_attribute(a, "height") / 2.0;
        assert!(dx.abs() <= half_width + 1e-9 && dy.abs() <= half_height + 1e-9);
        assert!(
            (dx.abs() - half_width).abs() <= 1e-9 || (dy.abs() - half_height).abs() <= 1e-9,
            "radius {radius} must route to the rectangle: endpoint=({dx},{dy}), half size=({half_width},{half_height})"
        );
        assert_eq!(
            block_shape_geometry(&layout, "A")["boundary"]["radius"].as_f64(),
            Some(f64::from(radius))
        );
    }
}

#[test]
fn block_svg_uses_mermaid_11_15_dom_ids_and_html_label_shape() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        svg.contains(r#"id="merman-A""#),
        "expected Block node DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-1-A-B""#),
        "expected Block edge DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"style="display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"><p>Alpha</p></span>"#),
        "expected Block node label to use Mermaid 11.15 XHTML paragraph shape: {svg}"
    );
}

#[test]
fn block_svg_keeps_blank_placeholder_label_paragraph() {
    let svg = render_block_svg_from_text(
        r#"block
  blockArrowId6<["   "]>(down)
"#,
    );

    assert!(
        svg.contains(r#"<span class="nodeLabel"><p>   </p></span>"#),
        "expected blank Block placeholder labels to keep Mermaid's paragraph child: {svg}"
    );
}

#[test]
fn block_svg_honors_configured_node_text_color() {
    for variables in [
        serde_json::json!({"nodeTextColor": "#123456", "textColor": "#abcdef"}),
        serde_json::json!({"textColor": "#123456"}),
        serde_json::json!({"nodeTextColor": "", "textColor": "#123456"}),
        serde_json::json!({"nodeTextColor": null, "textColor": "#123456"}),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables": variables}),
            ),
        );
        let svg = render_block_svg_from_text_with_engine(
            &engine,
            r#"block
  A["Alpha"]
"#,
        );

        assert!(
            svg.contains(
                r#"#merman .label text,#merman span,#merman p{fill:#123456;color:#123456;}"#
            ),
            "expected nodeTextColor || textColor to drive Block label color: {variables}"
        );
    }
}

#[test]
fn block_svg_fades_cluster_theme_colors() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"clusterBkg": "rebeccapurple", "clusterBorder": "hsl(80, 100%, 96.2745098039%)"}}}%%
block
  block
    A["Alpha"]
  end
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node .cluster{fill:rgba(102, 51, 153, 0.5);stroke:rgba(248.6666666666, 255, 235.9999999999, 0.2);stroke-width:1px;}"#
        ),
        "expected block composite cluster CSS to follow Mermaid 11.15 fade() colors"
    );
}

#[test]
fn block_svg_rejects_unsupported_cluster_theme_color() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "clusterBkg": "not-a-css-color"
            }
        })),
    );
    let error = try_render_block_svg_from_text_with_engine(
        &engine,
        r#"block
  block
    A["Alpha"]
  end
"#,
    )
    .expect_err("unsupported khroma color must fail the render operation");

    assert!(error.to_string().contains("not-a-css-color"));
}

#[test]
fn block_svg_normalizes_khroma_colors_and_preserves_css_tokens() {
    let svg = render_block_svg_from_text(
        r#"block
  A["HSL"]
  B["Alpha"]
  C["Variable"]
  style A color:hsl(80 100% 96%)
  style B color:#8090a080
  style C color:var(--MyColor)
"#,
    );

    assert!(svg.contains("color: rgb(248, 255, 235); display: table-cell;"));
    assert!(svg.contains("color: rgba(128, 144, 160, 0.502); display: table-cell;"));
    assert!(svg.contains("color: var(--MyColor); display: table-cell;"));
}

#[test]
fn block_svg_applies_class_definitions_to_assigned_nodes() {
    let svg = render_block_svg_from_text(
        r#"block
  Frontend Backend Database[("Database")]

  classDef front fill:#696,stroke:#333;
  classDef back fill:#969,stroke:#333;
  class Frontend front
  class Backend,Database back
"#,
    );

    assert!(
        svg.contains(r#"class="node front flowchart-label""#),
        "expected the Frontend node to retain its assigned class: {svg}"
    );
    let document = roxmltree::Document::parse(&svg).expect("Block class stylesheet XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block class stylesheet");
    assert!(
        css.contains(r#"#merman .front>*{fill:#696!important;stroke:#333!important;}"#),
        "expected the front class definition to style Block shapes: {svg}"
    );
    assert!(
        css.contains(r#"#merman .back>*{fill:#969!important;stroke:#333!important;}"#),
        "expected the back class definition to style Block shapes: {svg}"
    );
}

#[test]
fn block_svg_xml_escapes_class_definition_values() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"]
  classDef branded font-family:"A&B",fill:#696
  class A branded
"#,
    );

    roxmltree::Document::parse(&svg).expect("classDef CSS must remain well-formed XML");
    let document = roxmltree::Document::parse(&svg).expect("classDef CSS XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("classDef CSS");
    assert!(
        css.contains(r#"font-family:"A&B"!important;"#),
        "classDef values must be logical CSS after XML parsing: {css}"
    );
}

#[test]
fn block_svg_drops_unsafe_inline_css_without_losing_safe_siblings() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"]
  style A fill:url(https://example.test/paint.svg),stroke:#123456
"#,
    );

    assert!(
        !svg.contains("example.test") && !svg.contains("url(https://"),
        "unsafe external CSS resources must not reach the SVG: {svg}"
    );
    let shells = terminal_shell_styles(&svg, "merman-A");
    assert_eq!(shells.len(), 1, "expected one canonical Block shell");
    assert_eq!(terminal_stroke(&shells[0].1), Some("#123456"));
    assert_eq!(terminal_fill(&shells[0].1), None);
}

#[test]
fn block_circle_edge_starts_on_the_rendered_circle_boundary() {
    let svg = render_block_svg_from_text(
        r##"block-beta
  columns 3
  user(("User")):3
  space:3
  ui["Web UI"] api["API Server"] db[("Database")]

  user --> ui
  ui --> api
  api --> db

  style user fill:#ffe0b2,stroke:#fb8c00
  style db fill:#bbdefb,stroke:#1e88e5
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let user = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-user"))
        .expect("rendered user node");
    let circle = user
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .expect("rendered user circle");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "flowchart-link")
                })
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("user-ui"))
        })
        .expect("user to ui edge");

    let (center_x, center_y) = translated_center(user);
    let (edge_x, edge_y) = path_start(edge);
    let radius: f64 = circle
        .attribute("r")
        .expect("circle radius")
        .parse()
        .expect("numeric circle radius");
    let endpoint_radius = ((edge_x - center_x).powi(2) + (edge_y - center_y).powi(2)).sqrt();

    assert!(
        (endpoint_radius - radius).abs() <= 1e-3,
        "edge must start on the rendered circle: center=({center_x},{center_y}), endpoint=({edge_x},{edge_y}), endpoint_radius={endpoint_radius}, circle_radius={radius}, svg={svg}"
    );
}

#[test]
fn block_root_viewbox_uses_rendered_shape_bounds() {
    let svg = render_block_svg_from_text(
        r#"block
  id1((("This is the text in the circle")))
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-id1"))
        .expect("rendered double-circle node");
    let outer_circle = node
        .descendants()
        .find(|node| node.attribute("class") == Some("outer-circle"))
        .expect("rendered outer circle");
    let (center_x, center_y) = translated_center(node);
    let radius = outer_circle
        .attribute("r")
        .expect("outer circle radius")
        .parse::<f64>()
        .expect("numeric outer circle radius");
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let padding = 5.0;

    assert!(view_x <= center_x - radius - padding + 1e-9);
    assert!(view_y <= center_y - radius - padding + 1e-9);
    assert!(view_x + view_width >= center_x + radius + padding - 1e-9);
    assert!(view_y + view_height >= center_y + radius + padding - 1e-9);
}

#[test]
fn block_root_viewbox_ignores_non_schema_diagram_padding() {
    let default_svg = render_block_svg_from_text(
        r#"block
  A["A"] --> B["B"]
"#,
    );
    let opaque_config_svg = render_block_svg_from_text(
        r#"%%{init: {"block": {"diagramPadding": 250}}}%%
block
  A["A"] --> B["B"]
"#,
    );
    let default_document = roxmltree::Document::parse(&default_svg).expect("default Block SVG");
    let opaque_config_document =
        roxmltree::Document::parse(&opaque_config_svg).expect("configured Block SVG");

    assert_eq!(
        root_view_box(&default_document),
        root_view_box(&opaque_config_document),
        "Block viewBox padding is the fixed Mermaid 5px expansion, not an opaque config surface"
    );
}

#[test]
fn block_root_viewbox_contains_edge_label_foreign_object() {
    let svg = render_block_svg_from_text(
        r#"block
  A["A"] -- "A very long edge label" --> B["B"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let edge_label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("edgeLabel")
                && node
                    .descendants()
                    .any(|descendant| descendant.has_tag_name("foreignObject"))
        })
        .expect("rendered Block edge label group");
    let foreign_object = edge_label
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("rendered Block edge label foreignObject");
    let (label_min_x, label_min_y, label_max_x, label_max_y) =
        global_foreign_object_bounds(foreign_object);
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let view_max_x = view_x + view_width;
    let view_max_y = view_y + view_height;

    assert!(
        label_min_x >= view_x - 1e-6
            && label_min_y >= view_y - 1e-6
            && label_max_x <= view_max_x + 1e-6
            && label_max_y <= view_max_y + 1e-6,
        "edge label foreignObject must be contained by the root viewBox: label=({label_min_x},{label_min_y})..({label_max_x},{label_max_y}), viewBox=({view_x},{view_y}) {view_width}x{view_height}, svg={svg}"
    );
}

#[test]
fn block_html_labels_false_emits_svg_text_for_nodes_and_edges() {
    let svg = render_block_svg_from_text(
        r#"%%{init: {"htmlLabels": false}}%%
block
  A["Alpha"] -- "Edge label" --> B["Beta"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");

    assert!(
        document
            .descendants()
            .all(|node| !node.has_tag_name("foreignObject")),
        "htmlLabels:false must not emit foreignObject labels: {svg}"
    );

    let text_content = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .flat_map(|node| {
            node.descendants()
                .filter_map(|descendant| descendant.text())
        })
        .collect::<String>();
    assert!(
        text_content.contains("Alpha")
            && text_content.contains("Beta")
            && text_content.contains("Edge label"),
        "htmlLabels:false must preserve node and edge label text in SVG text terminals: {svg}"
    );

    for label in document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
    {
        let (x, _) = translated_center(label);
        assert_eq!(x, 0.0, "SVG text is already horizontally centered: {svg}");
    }
    let background = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .expect("edge label background");
    for attribute in ["width", "height"] {
        assert!(
            background
                .attribute(attribute)
                .unwrap()
                .parse::<f64>()
                .unwrap()
                > 0.0
        );
    }
}

#[test]
fn block_svg_labels_preserve_inline_text_color() {
    let svg = render_block_svg_from_text(
        r#"%%{init: {"htmlLabels": false}}%%
block
  A["Alpha"] B["Beta"] C["Variable"]
  style A fill:#eee,color:#123456,color:#abcdef
  style B color:hsl(80 100% 96%)
  style C color:var(--MyColor)
"#,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (id, expected) in [
        ("merman-A", "fill:#abcdef;"),
        ("merman-B", "fill:hsl(80 100% 96%);"),
        ("merman-C", "fill:var(--MyColor);"),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        let text = node
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("style"), Some(expected), "{id}: {svg}");
    }
}

#[test]
fn block_svg_edge_background_uses_operation_text_bounds() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::Arc;

    struct LabelHost(f64);
    impl HostTextMeasurer for LabelHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(match request.operation {
                TextMeasurementOperation::Wrapped => {
                    Some(HostTextMeasurement::Metrics(TextMetrics {
                        width: 80.0,
                        height: 40.0,
                        line_count: 2,
                    }))
                }
                TextMeasurementOperation::CreateTextBBoxYOffset => {
                    assert_eq!(request.text, "First\nSecond");
                    assert_eq!(request.style.font_size, 16.0);
                    Some(HostTextMeasurement::Length(self.0))
                }
                _ => None,
            })
        }
    }
    for bbox_y in [-7.5, 9.0] {
        let parsed = MermaidEngine::new().parse_diagram_for_render_model_sync(
            "%%{init: {\"htmlLabels\": false}}%%\nblock\n A[\"Alpha<br/>line\"] -- \"First<br/>Second\" --> B[\"Beta\"]\n",
            ParseOptions::default(),
        ).unwrap().unwrap().into_presentation();
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-svg-label").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(LabelHost(bbox_y)),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rect = document
            .descendants()
            .find(|node| node.attribute("class") == Some("background"))
            .unwrap();
        let number = |name| rect.attribute(name).unwrap().parse::<f64>().unwrap();
        assert_eq!((number("width"), number("height")), (84.0, 44.0));
        assert_eq!((number("x"), number("y")), (-42.0, bbox_y - 2.0));
        let label = rect.parent().unwrap();
        let (tx, ty) = translated_center(label);
        assert_eq!((tx, ty), (0.0, -bbox_y - 20.0));
        assert_eq!(tx + number("x") + number("width") / 2.0, 0.0);
        assert_eq!(ty + number("y") + number("height") / 2.0, 0.0);
        let (edge_x, edge_y) = translated_center(label.parent().unwrap());
        let (view_x, view_y, view_w, view_h) = root_view_box(&document);
        assert!(edge_x - 42.0 >= view_x && edge_x + 42.0 <= view_x + view_w);
        assert!(edge_y - 22.0 >= view_y && edge_y + 22.0 <= view_y + view_h);
        let rows = label
            .descendants()
            .filter(|node| node.attribute("class") == Some("row text-outer-tspan"))
            .count();
        assert_eq!(rows, 2);
    }
}

#[test]
fn block_root_viewbox_contains_rough_stroke_bounds() {
    let svg = render_block_svg_from_text(
        r#"block
  A(["A"])
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let stadium = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("rendered stadium node");
    let outer_path = stadium
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class == "basic label-container outer-path")
        })
        .expect("rendered stadium outer-path group");
    let stroke_path = outer_path
        .children()
        .find(|node| node.has_tag_name("path") && node.attribute("fill") == Some("none"))
        .expect("RoughJS stadium stroke path");
    let (min_x, min_y, max_x, max_y) = global_path_geometry_bounds(stroke_path);
    let stroke_width = stroke_path
        .attribute("stroke-width")
        .expect("RoughJS stroke width")
        .parse::<f64>()
        .expect("numeric RoughJS stroke width");
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let view_max_x = view_x + view_width;
    let view_max_y = view_y + view_height;
    let outset = stroke_width / 2.0;

    assert!(
        min_x - outset >= view_x - 1e-6
            && min_y - outset >= view_y - 1e-6
            && max_x + outset <= view_max_x + 1e-6
            && max_y + outset <= view_max_y + 1e-6,
        "root viewBox must contain the emitted RoughJS stroke envelope: path=({min_x},{min_y})..({max_x},{max_y}), stroke={stroke_width}, viewBox=({view_x},{view_y}) {view_width}x{view_height}, svg={svg}"
    );
}

#[test]
fn block_stadium_renders_rough_outer_path_and_clips_edge() {
    let svg = render_block_svg_from_text(
        r#"block
  A(["A"]) --> B["B"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let stadium = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("rendered stadium node");
    let outer_path = stadium
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class == "basic label-container outer-path")
        })
        .expect("rendered stadium outer-path group");
    let paths = outer_path
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "RoughJS stadiums emit fill and stroke paths"
    );
    assert_eq!(paths[0].attribute("stroke"), Some("none"));
    assert_eq!(paths[1].attribute("fill"), Some("none"));
    let stroke_width: f64 = paths[1]
        .attribute("stroke-width")
        .expect("RoughJS stroke width")
        .parse()
        .expect("numeric RoughJS stroke width");
    assert!((stroke_width - 1.3).abs() <= 1e-6);
    assert!(!outer_path.children().any(|node| node.has_tag_name("rect")));
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("id").is_some_and(|id| id.contains("A-B"))
        })
        .expect("stadium edge");

    let (center_x, center_y) = translated_center(stadium);
    let (edge_x, edge_y) = path_start(edge);
    assert!(
        edge_x > center_x,
        "edge must leave the stadium on its right side"
    );
    assert!((edge_y - center_y).abs() <= 1e-3);
}

#[test]
fn block_edge_invisible_self_loop_keeps_rendering() {
    for mixed in [false, true] {
        let source = if mixed {
            "block\n A --> A\n A --> B\n"
        } else {
            "block\n A --> A\n"
        };
        let svg = render_block_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
                .count(),
            if mixed { 2 } else { 1 }
        );
    }
}

#[test]
fn block_look_resources_precede_markers_and_follow_theme_configuration() {
    for (theme, gradient, flood) in [("redux", false, "#000000"), ("redux-dark", true, "#FFFFFF")] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "theme": theme,
                "look": "neo",
                "themeVariables": {
                    "useGradient": gradient,
                    "gradientStart": "#123456",
                    "gradientStop": "#abcdef"
                }
            })),
        );
        let svg = render_block_svg_from_text_with_engine(&engine, "block-beta\n  A[\"Alpha\"]\n");
        let document = roxmltree::Document::parse(&svg).expect("Block SVG");
        let root = document.root_element();
        let diagram_id = root.attribute("id").expect("diagram id");
        let children = root
            .children()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        let marker_index = children
            .iter()
            .position(|node| node.has_tag_name("marker"))
            .expect("marker");
        for (suffix, size, offset) in [
            ("drop-shadow", "130%", "4"),
            ("drop-shadow-small", "150%", "2"),
        ] {
            let id = format!("{diagram_id}-{suffix}");
            let matches = root
                .descendants()
                .filter(|node| node.attribute("id") == Some(id.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "look resources must have unique ids");
            let filter = matches[0];
            assert!(filter.has_tag_name("filter"));
            assert_eq!(filter.attribute("height"), Some(size));
            assert_eq!(filter.attribute("width"), Some(size));
            let defs = filter.parent().unwrap();
            assert!(defs.has_tag_name("defs"));
            assert!(children.iter().position(|node| *node == defs).unwrap() < marker_index);
            let shadow = filter
                .children()
                .find(|node| node.has_tag_name("feDropShadow"))
                .unwrap();
            assert_eq!(shadow.attribute("dx"), Some(offset));
            assert_eq!(shadow.attribute("dy"), Some(offset));
            assert_eq!(shadow.attribute("flood-color"), Some(flood));
        }
        let gradient_node = children
            .iter()
            .find(|node| node.has_tag_name("linearGradient"));
        assert_eq!(gradient_node.is_some(), gradient);
        if let Some(node) = gradient_node {
            assert_eq!(
                node.attribute("id"),
                Some(format!("{diagram_id}-gradient").as_str())
            );
            assert!(children.iter().position(|child| child == node).unwrap() < marker_index);
            let stops = node
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            assert_eq!(stops.len(), 2);
            assert_eq!(stops[0].attribute("stop-color"), Some("#123456"));
            assert_eq!(stops[1].attribute("stop-color"), Some("#abcdef"));
        }
    }
}

#[test]
fn block_composite_palette_arrays_own_the_actual_shell_paint() {
    const SOURCE: &str = "block-beta\nblock:g\nA\nend\n";
    for source_owned in [true, false] {
        let source = if source_owned {
            format!("{SOURCE}style g fill:#112233,stroke:#334455\n")
        } else {
            SOURCE.to_owned()
        };
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "theme":"redux-color","look":"classic", "themeVariables":{
                    "bkgColorArray":["#aabbcc"],"borderColorArray":["#445566"]
                }
            })),
        );
        let svg = render_block_svg_from_text_with_engine(&engine, &source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-g"))
            .expect("visible Block composite");
        assert_eq!(cluster.attribute("data-look"), Some("classic"));
        let shell = cluster
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("visible Block composite shell");
        let style = shell.attribute("style").unwrap_or_default();
        if source_owned {
            assert_eq!(terminal_fill(style), Some("#112233"));
            assert_eq!(terminal_stroke(style), Some("#334455"));
        } else {
            let slot = cluster
                .attribute("data-color-id")
                .expect("Block palette slot");
            let selector = format!(
                "#merman [data-look=\"classic\"][data-color-id=\"{slot}\"].node rect.composite{{"
            );
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("Block stylesheet");
            let palette_rule = css
                .split_once(&selector)
                .expect("materialized composite palette selector")
                .1
                .split_once('}')
                .unwrap()
                .0;
            assert!(palette_rule.contains("fill:#aabbcc;"), "{palette_rule}");
            assert!(palette_rule.contains("stroke:#445566;"), "{palette_rule}");
            assert_eq!(terminal_fill(style), None);
            assert_eq!(terminal_stroke(style), None);
        }
    }
}

#[test]
fn block_composite_palette_wraps_slots_and_preserves_author_styles() {
    let source = "block-beta\nblock:outer\nblock:inner\nA\nend\nend\nblock:sibling\nB\nend\nstyle inner fill:#112233,stroke:#445566\n";
    for (theme, tinted) in [
        ("redux-color", true),
        ("redux-dark-color", false),
        ("redux", false),
    ] {
        let engine = MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "theme": theme, "look": "classic",
            "themeVariables": {"borderColorArray": ["#aa0000", "#00aa00"],
                "bkgColorArray": if tinted { vec!["#eeeeee"] } else { vec![] }, "THEME_COLOR_LIMIT": 1}
        })));
        let svg = render_block_svg_from_text_with_engine(&engine, source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let id = document.root_element().attribute("id").unwrap();
        for (name, slot) in [
            ("outer", "color-0"),
            ("inner", "color-1"),
            ("sibling", "color-0"),
            ("A", ""),
            ("B", ""),
        ] {
            let node_id = format!("{id}-{name}");
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some(node_id.as_str()))
                .unwrap();
            assert_eq!(node.attribute("data-look"), Some("classic"));
            assert_eq!(
                node.attribute("data-color-id"),
                (theme != "redux" && !slot.is_empty()).then_some(slot)
            );
            if name == "inner" {
                let rect = node
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                let style = rect.attribute("style").unwrap();
                assert!(style.contains("fill:#112233") && style.contains("stroke:#445566"));
            }
        }
        let rule = format!(
            r#"#{id} [data-look="classic"][data-color-id="color-1"].node rect.composite{{stroke:#00aa00;{}}}"#,
            if tinted { "fill:#eeeeee;" } else { "" }
        );
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(css.contains(&rule), theme != "redux");
    }
}

#[test]
fn block_flat_records_preserve_nested_dom_order_and_directives() {
    let svg = render_block_svg_from_text(
        "block\ncolumns 2\nblock:outer:2\ncolumns 2\nA[\"Alpha\"]\nblock:inner\nB<[\"Route\"]>(left,down)\nend\nend\nC\nA --> C\nclassDef branded fill:#696,stroke:#333\nclass A branded\nstyle inner fill:#112233,stroke:#445566\n",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let nodes = document
        .descendants()
        .filter(|node| {
            node.attribute("class").is_some_and(|classes| {
                classes
                    .split_ascii_whitespace()
                    .any(|class| class == "node")
            })
        })
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        nodes,
        [
            "merman-outer",
            "merman-A",
            "merman-inner",
            "merman-B",
            "merman-C"
        ]
    );
    assert!(svg.contains("node branded flowchart-label"));
    assert!(svg.contains("fill:#112233;stroke:#445566"));
    assert!(svg.contains("merman-1-A-C"));
}

#[test]
fn block_uppercase_source_color_reaches_svg_text_fill() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({"htmlLabels":false}),
        ),
    );
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        "block\n A[\"Alpha\"]\n style A COLOR:#123456\n",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let text = document
        .descendants()
        .find(|n| n.has_tag_name("text") && n.descendants().any(|n| n.text() == Some("Alpha")))
        .expect("visible SVG node text");
    assert!(
        text.attribute("style")
            .unwrap_or("")
            .contains("fill:#123456;")
    );
}

#[test]
fn block_html_source_font_longhands_measure_the_emitted_terminal() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::Arc;

    struct FontHost;
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::Wrapped || request.text != "Alpha" {
                return Ok(None);
            }
            let family_width = if request.style.font_family.as_deref() == Some("monospace") {
                7.0
            } else {
                0.0
            };
            let italic_width = if request.style.font_style.as_deref() == Some("italic") {
                5.0
            } else {
                0.0
            };
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width: request.style.font_size * 2.0 + family_width + italic_width,
                height: request.style.font_size * 1.5,
                line_count: 1,
            })))
        }
    }

    for (size, expected_width, expected_height) in [
        ("48px", 108.0, 72.0),
        ("36pt", 108.0, 72.0),
        // The same declaration reaches g.label, the XHTML div, and span.nodeLabel.
        ("150%", 120.0, 81.0),
        ("1.5em", 120.0, 81.0),
    ] {
        for assigned_class in [false, true] {
            let declarations = format!("font-size:{size},font-family:monospace,font-style:italic");
            let source = if assigned_class {
                format!("block\n A[\"Alpha\"]\n classDef face {declarations}\n class A face\n")
            } else {
                format!("block\n A[\"Alpha\"]\n style A {declarations}\n")
            };
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": true, "themeVariables": {"fontSize": "16px"}}),
                ),
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.block-source-terminal-font").unwrap(),
                "1",
            )
            .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity,
                    Arc::new(FontHost),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session()
                .unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-A"))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|node| node.has_tag_name("p"))
                .unwrap();
            assert_eq!(terminal.text(), Some("Alpha"));
            let styled_ancestors: Vec<_> = terminal
                .ancestors()
                .filter(|node| {
                    node.attribute("style")
                        .is_some_and(|style| style.contains("font-size:"))
                })
                .collect();
            assert_eq!(styled_ancestors.len(), 3, "preserve the pinned style sinks");
            for ancestor in styled_ancestors {
                let style = ancestor.attribute("style").unwrap();
                let priority = " !important";
                assert!(
                    style.contains(&format!("font-size:{size}{priority};")),
                    "{style}"
                );
                assert!(
                    style.contains(&format!("font-family:monospace{priority};")),
                    "{style}"
                );
                assert!(
                    style.contains(&format!("font-style:italic{priority};")),
                    "{style}"
                );
            }
            let foreign_object = terminal
                .ancestors()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert_eq!(
                numeric_attribute(foreign_object, "width"),
                expected_width,
                "{size}"
            );
            assert_eq!(
                numeric_attribute(foreign_object, "height"),
                expected_height,
                "{size}"
            );
        }
    }
}

#[test]
fn block_source_font_weight_priority_reaches_labels_and_measurement() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::{Arc, Mutex};
    struct WeightHost(Arc<Mutex<Vec<Option<String>>>>);
    impl HostTextMeasurer for WeightHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation == TextMeasurementOperation::Wrapped && request.text == "Alpha" {
                self.0
                    .lock()
                    .unwrap()
                    .push(request.style.font_weight.clone());
                return Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                    width: if request.style.font_weight.as_deref() == Some("700") {
                        70.0
                    } else {
                        40.0
                    },
                    height: 24.0,
                    line_count: 1,
                })));
            }
            Ok(None)
        }
    }
    for html in [false, true] {
        for (later, expected_css, expected_weight) in [
            ("FONT-WEIGHT:normal", "font-weight:bold !important;", "700"),
            (
                "FONT-WEIGHT:normal !important",
                "FONT-WEIGHT:normal !important;",
                "400",
            ),
        ] {
            let source = format!(
                "%%{{init: {{\"htmlLabels\": {html}}}}}%%\nblock\n A[\"Alpha\"]\n style A font-weight:bold !important,{later}\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let weights = Arc::new(Mutex::new(Vec::new()));
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.block-source-weight-priority").unwrap(),
                "1",
            )
            .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity,
                    Arc::new(WeightHost(weights.clone())),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session()
                .unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some("merman-A"))
                .unwrap();
            let label = node
                .descendants()
                .find(|n| n.has_tag_name("g") && n.attribute("class") == Some("label"))
                .unwrap();
            assert_eq!(label.attribute("style"), Some(expected_css));
            let terminal = label
                .descendants()
                .find(|n| {
                    if html {
                        n.has_tag_name("span")
                    } else {
                        n.has_tag_name("text")
                    }
                })
                .unwrap();
            assert_eq!(terminal.attribute("style"), Some(expected_css));
            if !html {
                let word = terminal
                    .descendants()
                    .find(|node| node.attribute("class") == Some("text-inner-tspan"))
                    .unwrap();
                assert_eq!(word.attribute("font-weight"), Some("normal"));
                assert_eq!(word.attribute("font-style"), Some("normal"));
            }
            let measured = weights.lock().unwrap();
            assert!(!measured.is_empty());
            assert!(
                measured.iter().all(|weight| {
                    if html {
                        weight.as_deref() == Some(expected_weight)
                    } else {
                        matches!(weight.as_deref(), None | Some("normal" | "400"))
                    }
                }),
                "html={html}, {later}: {measured:?}"
            );
        }
    }
}

#[test]
fn block_class_fonts_follow_assignment_order_and_terminal_inline_priority() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMetrics, TextStyle};
    use std::sync::{Arc, Mutex};

    struct FontHost(Arc<Mutex<Vec<TextStyle>>>);
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::Wrapped || request.text != "Alpha" {
                return Ok(None);
            }
            self.0.lock().unwrap().push(request.style.clone());
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width: request.style.font_size * 2.0,
                height: request.style.font_size * 1.5,
                line_count: 1,
            })))
        }
    }

    for (declarations, expected_size, expected_family) in [
        (
            "classDef default font-size:36pt,font-family:monospace\n",
            48.0,
            "monospace",
        ),
        (
            "classDef default font-size:150%,font-family:monospace\n",
            36.0,
            "monospace",
        ),
        // Class assignment order wins the compiled inline label, independent of definition order.
        (
            "classDef first font-size:24px,font-family:serif\nclassDef second font-size:48px,font-family:monospace\nclass A second\nclass A first\n",
            24.0,
            "serif",
        ),
        (
            "classDef first font-size:24px,font-family:serif\nclassDef second font-size:48px,font-family:monospace\nclass A first\nclass A second\n",
            48.0,
            "monospace",
        ),
        (
            "classDef face font-size:48px,font-family:monospace\nclass A face\nstyle A font-size:30px,font-family:serif\n",
            30.0,
            "serif",
        ),
        (
            "classDef face font-size:48px,font-family:monospace\nclass A face\nstyle A font-size:30px !important,font-family:serif !important\n",
            30.0,
            "serif",
        ),
        // Direct styles are compiled to inline important fonts even without an assigned class.
        (
            "classDef default font-size:48px,font-family:monospace\nstyle A font-size:30px,font-family:serif\n",
            30.0,
            "serif",
        ),
        (
            "classDef default font-size:48px,font-family:monospace\nstyle A font-size:30px !important,font-family:serif !important\n",
            30.0,
            "serif",
        ),
        (
            "classDef default font-size:48px\nclassDef plain stroke:red\nclass A plain\nstyle A font-size:30px,font-family:serif\n",
            30.0,
            "serif",
        ),
    ] {
        let source = format!("block\nA[\"Alpha\"]\n{declarations}");
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels": true, "themeVariables": {"fontSize": "16px"}}),
            ),
        );
        let parsed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-class-font-order").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(FontHost(calls.clone())),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-A"))
            .unwrap();
        let viewport = node
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            numeric_attribute(viewport, "width"),
            expected_size * 2.0,
            "{declarations}"
        );
        assert_eq!(
            numeric_attribute(viewport, "height"),
            expected_size * 1.5,
            "{declarations}"
        );
        let calls = calls.lock().unwrap();
        assert!(!calls.is_empty(), "{declarations}");
        assert!(
            calls.iter().all(|style| style.font_size == expected_size
                && style.font_family.as_deref() == Some(expected_family)),
            "{declarations}: {calls:?}"
        );
        if declarations.contains("class A face") {
            let span = node
                .descendants()
                .find(|node| node.attribute("class") == Some("nodeLabel"))
                .unwrap();
            assert!(
                span.attribute("style")
                    .unwrap()
                    .contains("font-size:30px !important;")
            );
        }
    }
}

#[test]
fn block_html_wrapper_class_fonts_reach_multiline_and_descendant_measurement() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use std::sync::{Arc, Mutex};

    struct FontHost(Arc<Mutex<Vec<(String, f64)>>>);
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if !matches!(request.text, "Alpha" | "Beta") {
                return Ok(None);
            }
            let length = match request.operation {
                TextMeasurementOperation::CanvasMeasureTextWidth => request.style.font_size * 2.0,
                TextMeasurementOperation::HtmlLineHeight => request.style.font_size * 1.5,
                _ => return Ok(None),
            };
            self.0
                .lock()
                .unwrap()
                .push((request.text.to_owned(), request.style.font_size));
            Ok(Some(HostTextMeasurement::Length(length)))
        }
    }

    for (label, expected_size, expected_height) in [
        ("Alpha<br/>Beta", 48.0, 144.0),
        ("<span style='font-size:24px'>Alpha</span>", 48.0, 72.0),
        (
            "<span style='font-size:24px !important'>Alpha</span>",
            24.0,
            72.0,
        ),
    ] {
        let source = format!("block\nA[\"{label}\"]\nclassDef nodeLabel font-size:48px\n");
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels":true,"themeVariables":{"fontSize":"16px"}}),
                ),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-wrapper-class-terminal").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(FontHost(calls.clone())),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-A"))
            .unwrap();
        let viewport = node
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            numeric_attribute(viewport, "width"),
            expected_size * 2.0,
            "{label}"
        );
        assert_eq!(
            numeric_attribute(viewport, "height"),
            expected_height,
            "{label}"
        );
        let paragraph = node
            .descendants()
            .find(|node| node.has_tag_name("p"))
            .unwrap();
        assert!(
            paragraph.attribute("style").is_none(),
            "measurement projection must not rewrite the emitted DOM"
        );
        assert!(
            rendered
                .svg()
                .contains(".nodeLabel>*{font-size:48px!important;}")
        );
        assert!(
            rendered
                .svg()
                .contains(".nodeLabel span{font-size:48px!important;}")
        );
        let calls = calls.lock().unwrap();
        assert!(
            calls.iter().any(|(text, _)| text == "Alpha"),
            "{label}: {calls:?}"
        );
        if label.contains("Beta") {
            assert!(calls.iter().any(|(text, _)| text == "Beta"), "{calls:?}");
        }
        assert!(
            calls.iter().all(|(_, size)| *size == expected_size),
            "{label}: {calls:?}"
        );
    }
}

#[test]
fn block_zero_fonts_preserve_shells_and_edge_backgrounds() {
    for html in [false, true] {
        for from_source in [false, true] {
            let config = serde_json::json!({"htmlLabels": html, "themeVariables": {"fontSize": "0px", "edgeLabelBackground": "#b316cd"}});
            let body = "block\ncolumns 2\nblock:group:2\nA[\"Alpha\"] B[\"Beta\"]\nend\nA -- \"relates\" --> B\n";
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config)
            };
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site),
            );
            let svg = render_block_svg_from_text_with_engine(&engine, &source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            assert!(css.contains("font-size:0px;"), "{css}");
            assert!(
                css.contains(".edgeLabel{background-color:#b316cd;"),
                "{css}"
            );
            for id in ["merman-group", "merman-A", "merman-B"] {
                let node = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap();
                let shell = node
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                assert!(numeric_attribute(shell, "width") > 0.0);
                assert!(numeric_attribute(shell, "height") > 0.0);
            }
            if !html {
                assert!(document.descendants().any(|node| node.has_tag_name("rect")
                    && node.attribute("class") == Some("background")
                    && numeric_attribute(node, "width") > 0.0));
            }
        }
    }
}

#[test]
fn block_zero_edge_label_class_keeps_its_background() {
    for html in [false, true] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "htmlLabels": html, "themeVariables": {"edgeLabelBackground":"#123456"}
            })),
        );
        let svg = render_block_svg_from_text_with_engine(
            &engine,
            "block\nA[\"\"] -- \"relates\" --> B[\"\"]\nclassDef edgeLabel font-size:0px\n",
        );
        assert!(svg.contains(".edgeLabel{background-color:#123456;"));
        assert!(svg.contains("relates"));
    }
}
