mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{
    Engine, MAX_DIAGRAM_NESTING_DEPTH, MermaidConfig, ParseOptions, ParsedDiagramRender,
};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementOperation,
    TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::TreeViewDiagramLayout;
use merman_render::resources::RenderResourcePolicy;
use merman_render::svg::{IconPack, IconRegistry, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextStyle};
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
struct TreeViewBBoxHost {
    operations: Mutex<Vec<TextMeasurementOperation>>,
}

impl HostTextMeasurer for TreeViewBBoxHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.operations.lock().unwrap().push(request.operation);
        Ok(match request.operation {
            TextMeasurementOperation::RawBBoxWidth => {
                Some(HostTextMeasurement::Length(request.text.len() as f64 * 8.0))
            }
            TextMeasurementOperation::RawBBoxHeight => Some(HostTextMeasurement::Length(31.0)),
            _ => None,
        })
    }
}

fn render_tree_view_svg_with_options(input: &str, options: SvgRenderOptions) -> String {
    render_tree_view_svg_with_environment(input, options, &RenderEnvironment::deterministic())
}

fn render_tree_view_svg_with_environment(
    input: &str,
    options: SvgRenderOptions,
    environment: &RenderEnvironment,
) -> String {
    render_tree_view_svg_with_engine_and_environment(input, options, &Engine::new(), environment)
}

fn render_tree_view_svg_with_engine_and_environment(
    input: &str,
    options: SvgRenderOptions,
    engine: &Engine,
    environment: &RenderEnvironment,
) -> String {
    let session = environment.begin_session().unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .expect("TreeView diagram");
    render_parsed_tree_view_svg(parsed, options, session)
}

fn render_parsed_tree_view_svg(
    parsed: ParsedDiagramRender,
    options: SvgRenderOptions,
    session: RenderSession,
) -> String {
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    artifact
        .render_svg(&options, &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn layout_tree_view(input: &str, environment: &RenderEnvironment) -> TreeViewDiagramLayout {
    let session = environment.begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .expect("TreeView diagram");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    serde_json::from_value(projection["layout"]["TreeViewDiagram"].clone()).unwrap()
}

fn tree_view_icon_group_for_label<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    let label_node = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some(label))
        .expect("TreeView label node");
    label_node
        .parent()
        .expect("TreeView node group")
        .children()
        .find(|node| {
            node.has_tag_name("g") && node.attribute("class") == Some("treeView-node-icon")
        })
        .expect("inline TreeView icon group")
}

fn tree_view_icon_svg_for_label<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    tree_view_icon_group_for_label(document, label)
        .children()
        .find(|node| node.has_tag_name("svg"))
        .expect("inline TreeView icon SVG")
}

#[test]
fn tree_view_typed_render_model_outputs_svg() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"---
config:
  treeView:
    rowIndent: 80
    lineThickness: 3
  themeVariables:
    treeView:
      labelFontSize: '20px'
      labelColor: '#FF0000'
      lineColor: '#00FF00'
---
treeView-beta
    "packages"
        "mermaid"
            "src"
        "parser"
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(svg.contains(r#"aria-roledescription="treeView""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"style="max-width: "#));
    assert!(svg.contains(r#"viewBox="-1.5 0 "#));
    assert!(svg.contains(r#"<g/><g class="tree-view">"#));
    assert!(svg.contains(r#"<g class="tree-view">"#));
    assert!(svg.contains(r#"<g><text dominant-baseline="middle""#));
    assert!(svg.contains(r#"class="treeView-node-label""#));
    assert!(svg.contains(r#"class="treeView-node-line""#));
    assert!(svg.contains(r#"font-size: 20px"#));
    assert!(svg.contains(r#"fill: #FF0000"#));
    assert!(svg.contains(r#"stroke: #00FF00"#));
}

#[test]
fn tree_view_typed_render_model_outputs_accessibility_nodes() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
title TreeView Diagram Title
accTitle: Accessible TreeView Title
accDescr: Accessible TreeView Description
"Root"
    "Child"
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-a11y-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(svg.contains(
        r#"aria-describedby="chart-desc-tree-view-a11y-test" aria-labelledby="chart-title-tree-view-a11y-test""#
    ));
    assert!(svg.contains(
        r#"<title id="chart-title-tree-view-a11y-test">Accessible TreeView Title</title><desc id="chart-desc-tree-view-a11y-test">Accessible TreeView Description</desc><style>"#
    ));
}

#[test]
fn tree_view_mermaid_11_16_annotations_render_svg_dom() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"---
config:
  treeView:
    showIcons: true
    defaultIconPack: logos
    extensionIcons:
      ".tsx": react
---
treeView-beta
src/ :::highlight icon(folder) ## source directory
    App.tsx ## main component
    package.json icon(none)
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-11-16-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(svg.contains(r#"class="treeView-node-label treeView-node-dir highlight""#));
    assert!(svg.contains(".treeView-node-dir { font-weight: bold; }"));
    assert!(svg.contains(r#"class="treeView-highlight-bg""#));
    assert!(svg.contains(r#"class="treeView-node-description""#));
    assert!(svg.contains("source directory"));
    assert!(svg.contains("main component"));
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    assert_eq!(
        tree_view_icon_svg_for_label(&document, "src").attribute("viewBox"),
        Some("0 0 24 24")
    );
    assert_eq!(
        tree_view_icon_svg_for_label(&document, "App.tsx").attribute("viewBox"),
        Some("0 0 80 80")
    );
    assert!(!svg.contains("<defs"));
    assert!(!svg.contains("<use"));
    assert!(!svg.contains("package.json icon"));
    assert!(svg.contains(".treeView-node-icon"));
    assert!(svg.contains(".treeView-node-description"));
    assert!(svg.contains(".treeView-highlight-bg"));
}

#[test]
fn tree_view_security_levels_keep_inline_icon_dom() {
    let input = "treeView-beta\nRoot icon(folder)\n";
    let options = SvgRenderOptions {
        diagram_id: Some("tree-view-security-level-test".to_string()),
        ..Default::default()
    };
    let strict_svg = render_tree_view_svg_with_options(input, options.clone());
    let loose_engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({ "securityLevel": "loose" }),
    ));
    let loose_svg = render_tree_view_svg_with_engine_and_environment(
        input,
        options,
        &loose_engine,
        &RenderEnvironment::deterministic(),
    );
    let strict_document = roxmltree::Document::parse(&strict_svg).expect("valid strict SVG");
    let loose_document = roxmltree::Document::parse(&loose_svg).expect("valid loose SVG");

    for document in [&strict_document, &loose_document] {
        assert!(
            document
                .descendants()
                .all(|node| !node.has_tag_name("defs"))
        );
        assert!(document.descendants().all(|node| !node.has_tag_name("use")));
        assert_eq!(
            tree_view_icon_svg_for_label(document, "Root").attribute("viewBox"),
            Some("0 0 24 24")
        );
    }

    let root_attribute = |document: &roxmltree::Document<'_>, name| {
        document
            .root_element()
            .attribute(name)
            .expect("root attribute")
            .to_string()
    };
    assert_eq!(
        root_attribute(&strict_document, "viewBox"),
        root_attribute(&loose_document, "viewBox")
    );
    let label_x = |document: &roxmltree::Document<'_>| {
        document
            .descendants()
            .find(|node| node.tag_name().name() == "text" && node.text() == Some("Root"))
            .and_then(|node| node.attribute("x"))
            .expect("label x")
            .to_string()
    };
    assert_eq!(label_x(&strict_document), label_x(&loose_document));
}

#[test]
fn tree_view_builtin_icons_render_at_fourteen_pixels_without_overlapping_labels() {
    let input = r##"treeView-beta
src/ icon(folder)
file.txt icon(file)
App.tsx icon(logos:react)
"##;

    let pack = br#"{
        "prefix":"mermaid-treeview",
        "icons":{
            "file":{"body":"<path data-icon=\"registry-override\"/>"},
            "folder":{"body":"<path data-icon=\"registry-override\"/>"}
        }
    }"#;
    let registry = IconRegistry::from_packs([IconPack::new(pack)]).unwrap();
    let environment = RenderEnvironment::deterministic().with_icon_registry(registry);
    let loose_engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({ "securityLevel": "loose" }),
    ));
    let svg = render_tree_view_svg_with_engine_and_environment(
        input,
        SvgRenderOptions {
            diagram_id: Some("tree-view-icon-size-test".to_string()),
            ..Default::default()
        },
        &loose_engine,
        &environment,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");

    for label in ["src", "file.txt"] {
        let icon_group = tree_view_icon_group_for_label(&document, label);
        let icon_svg = tree_view_icon_svg_for_label(&document, label);

        assert_eq!(icon_svg.attribute("width"), Some("14"));
        assert_eq!(icon_svg.attribute("height"), Some("14"));
        assert_eq!(icon_svg.attribute("viewBox"), Some("0 0 24 24"));

        let label_node = document
            .descendants()
            .find(|node| node.tag_name().name() == "text" && node.text() == Some(label))
            .expect("icon label node");
        let icon_x = icon_group
            .attribute("transform")
            .and_then(|transform| transform.strip_prefix("translate("))
            .and_then(|transform| transform.split(',').next())
            .expect("icon translate x")
            .parse::<f64>()
            .expect("numeric icon x");
        let icon_right = icon_x + 14.0;
        let label_x = label_node
            .attribute("x")
            .expect("label x")
            .parse::<f64>()
            .expect("numeric label x");

        assert_eq!(label_x - icon_right, 4.0);
    }
    assert!(!svg.contains("registry-override"), "{svg}");

    let fallback_svg = tree_view_icon_svg_for_label(&document, "App.tsx");
    assert_eq!(fallback_svg.attribute("width"), Some("14"));
    assert_eq!(fallback_svg.attribute("height"), Some("14"));
    assert_eq!(fallback_svg.attribute("viewBox"), Some("0 0 80 80"));
    assert!(
        fallback_svg
            .children()
            .any(|node| node.is_element() && node.tag_name().name() == "g")
    );
}

#[test]
fn tree_view_registry_icons_preserve_viewbox_and_empty_body_semantics() {
    let pack = br#"{
        "prefix":"test",
        "icons":{
            "rocket":{
                "body":"<path data-icon=\"tree-view-registry\" d=\"M2 3H34V21H2z\"/>",
                "left":2,
                "top":3,
                "width":32,
                "height":18
            },
            "empty":{"body":""}
        }
    }"#;
    let registry = IconRegistry::from_packs([IconPack::new(pack)]).unwrap();
    let environment = RenderEnvironment::deterministic().with_icon_registry(registry);
    let svg = render_tree_view_svg_with_environment(
        "treeView-beta\nRoot\n    Rocket icon(test:rocket)\n    Rocket Again icon(test:rocket)\n    Missing icon(test:missing)\n    Empty icon(test:empty)\n",
        SvgRenderOptions {
            diagram_id: Some("tree-view-registry-test".to_string()),
            ..Default::default()
        },
        &environment,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");

    let rocket_svg = tree_view_icon_svg_for_label(&document, "Rocket");
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-icon") == Some("tree-view-registry"))
            .count(),
        2
    );
    assert_eq!(rocket_svg.attribute("width"), Some("14"));
    assert_eq!(rocket_svg.attribute("height"), Some("14"));
    assert_eq!(rocket_svg.attribute("viewBox"), Some("2 3 32 18"));
    assert!(
        rocket_svg
            .descendants()
            .any(|node| node.attribute("data-icon") == Some("tree-view-registry"))
    );

    assert_unknown_tree_view_icon(&document, "Missing");

    let empty_svg = tree_view_icon_svg_for_label(&document, "Empty");
    assert_eq!(empty_svg.attribute("viewBox"), Some("0 0 16 16"));
    assert_eq!(
        empty_svg
            .children()
            .filter(|node| node.is_element())
            .count(),
        0
    );
}

#[test]
fn tree_view_missing_icon_without_registry_uses_unknown_icon() {
    let svg = render_tree_view_svg_with_options(
        "treeView-beta\nRoot icon(test:missing)\n",
        SvgRenderOptions {
            diagram_id: Some("tree-view-no-registry-test".to_string()),
            ..Default::default()
        },
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    assert_unknown_tree_view_icon(&document, "Root");
}

fn assert_unknown_tree_view_icon(document: &roxmltree::Document<'_>, label: &str) {
    let icon_svg = tree_view_icon_svg_for_label(document, label);
    assert_eq!(icon_svg.attribute("width"), Some("14"));
    assert_eq!(icon_svg.attribute("height"), Some("14"));
    assert_eq!(icon_svg.attribute("viewBox"), Some("0 0 80 80"));
    assert_eq!(
        icon_svg
            .descendants()
            .find(|node| node.tag_name().name() == "tspan")
            .and_then(|node| node.text()),
        Some("?")
    );
}

#[test]
fn tree_view_root_highlight_visual_bounds_fit_inside_viewbox() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
root/ :::highlight
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let tree_view = layout_tree_view(input, &RenderEnvironment::deterministic());
    let content_width = tree_view
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0, f64::max);

    assert_eq!(tree_view.total_width, content_width + 10.0);

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-root-highlight-test".to_string()),
            ..Default::default()
        },
        session,
    );
    assert_tree_view_highlights_fit_viewbox(&svg);
}

#[test]
fn tree_view_multiple_highlights_follow_upstream_width_growth() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
root/ :::highlight
    child/ :::highlight
        leaf.txt
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let tree_view = layout_tree_view(input, &RenderEnvironment::deterministic());
    let content_width = tree_view
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0, f64::max);
    let highlighted_nodes = tree_view
        .nodes
        .iter()
        .filter(|node| {
            node.css_class
                .as_deref()
                .is_some_and(|class| class.split_whitespace().any(|part| part == "highlight"))
        })
        .collect::<Vec<_>>();

    assert_eq!(highlighted_nodes.len(), 2);
    assert_eq!(tree_view.total_width, content_width + 20.0);

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-multiple-highlights-test".to_string()),
            ..Default::default()
        },
        session,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    let highlight_rects = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-highlight-bg"))
        .collect::<Vec<_>>();
    let mut width_before_highlight = content_width;
    for (node, rect) in highlighted_nodes.into_iter().zip(highlight_rects) {
        let actual_width = rect
            .attribute("width")
            .expect("highlight width")
            .parse::<f64>()
            .expect("numeric highlight width");
        let expected_width = width_before_highlight - node.x + 8.0;
        assert!((actual_width - expected_width).abs() < 1e-9);
        width_before_highlight += 10.0;
    }
    assert_eq!(width_before_highlight, tree_view.total_width);
    assert_tree_view_highlights_fit_viewbox(&svg);
}

fn assert_tree_view_highlights_fit_viewbox(svg: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid TreeView SVG");
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .expect("TreeView viewBox")
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    let view_box_right = view_box[0] + view_box[2];

    for rect in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-highlight-bg"))
    {
        let x = rect
            .attribute("x")
            .expect("highlight x")
            .parse::<f64>()
            .expect("numeric highlight x");
        let width = rect
            .attribute("width")
            .expect("highlight width")
            .parse::<f64>()
            .expect("numeric highlight width");
        let visual_right = x + width + 0.5;
        assert!(
            visual_right <= view_box_right,
            "highlight right edge {visual_right} exceeds viewBox right edge {view_box_right}"
        );
    }
}

#[test]
fn tree_view_trailing_slash_only_marks_directory_labels() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
src/ :::directory-probe
    main.rs :::file-probe
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-directory-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(
        svg.contains(r#"class="treeView-node-label treeView-node-dir directory-probe""#),
        "trailing-slash directory should receive the upstream directory class: {svg}"
    );
    assert!(svg.contains(r#"class="treeView-node-label file-probe""#));
    assert!(!svg.contains(r#"treeView-node-dir file-probe"#));
}

#[test]
fn tree_view_layout_measures_directory_labels_with_bold_style() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let layout = layout_tree_view(
        "treeView-beta\nverylongdirectoryname/ :::directory-probe\n",
        &RenderEnvironment::deterministic(),
    );
    let directory = layout
        .nodes
        .iter()
        .find(|node| node.css_class.as_deref() == Some("directory-probe"))
        .expect("directory node");
    let style = TextStyle {
        font_size: layout.label_font_size,
        font_weight: Some("bold".to_string()),
        ..Default::default()
    };
    let expected = session
        .text_measurer(TextMeasurementPhase::SvgBBox)
        .measure_svg_raw_text_bbox_width_px(&directory.name, &style);

    assert_eq!(directory.label_width, expected);
}

#[test]
fn tree_view_layout_measures_descriptions_with_italic_style() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let layout = layout_tree_view(
        "treeView-beta\nfile.txt ## a long slanted description\n",
        &RenderEnvironment::deterministic(),
    );
    let node = layout
        .nodes
        .iter()
        .find(|node| node.description.is_some())
        .expect("described node");
    let description = node.description.as_deref().expect("description");
    let style = TextStyle {
        font_size: layout.label_font_size,
        font_style: Some("italic".to_string()),
        ..Default::default()
    };
    let expected = session
        .text_measurer(TextMeasurementPhase::SvgBBox)
        .measure_svg_raw_text_bbox_width_px(description, &style);

    assert_eq!(node.description_width, Some(expected));
}

#[test]
fn tree_view_layout_routes_direct_text_bbox_height_through_the_host() {
    let host = Arc::new(TreeViewBBoxHost::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.tree-view-bbox-host").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            [TextMeasurementPhase::SvgBBox],
        ),
    );
    let layout = layout_tree_view("treeView-beta\nfile.txt\n", &environment);

    assert!(layout.nodes.iter().all(|node| node.label_height == 31.0));
    assert!(layout.nodes.iter().all(|node| node.height == 41.0));
    assert!(
        host.operations
            .lock()
            .unwrap()
            .contains(&TextMeasurementOperation::RawBBoxHeight)
    );
}

#[test]
fn tree_view_fixed_size_root_keeps_width_and_height() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"---
config:
  treeView:
    useMaxWidth: false
---
treeView-beta
"Root"
    "Child"
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-fixed-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(!svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"<svg id="tree-view-fixed-test" width=""#));
    assert!(svg.contains(r#"" height=""#));
    assert!(svg.contains(r#"style="background-color: white;" viewBox="-0.5 0 "#));
    assert!(!svg.contains("max-width:"));
}

#[test]
fn tree_view_public_layout_accepts_max_allowed_chain() {
    let mut input = String::from("treeView-beta\n");
    for depth in 0..MAX_DIAGRAM_NESTING_DEPTH {
        input.push_str(&" ".repeat(depth));
        input.push('"');
        input.push_str(&format!("n{depth}"));
        input.push_str("\"\n");
    }

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let trusted_environment = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input());
    let tree_view = layout_tree_view(&input, &trusted_environment);

    assert_eq!(tree_view.nodes.len(), MAX_DIAGRAM_NESTING_DEPTH + 1);
    assert_eq!(
        tree_view.nodes.first().map(|node| node.name.as_str()),
        Some("/")
    );
    let expected_last = format!("n{}", MAX_DIAGRAM_NESTING_DEPTH - 1);
    assert_eq!(
        tree_view.nodes.last().map(|node| node.name.as_str()),
        Some(expected_last.as_str())
    );
}
