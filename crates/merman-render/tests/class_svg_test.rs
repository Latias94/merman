mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn render_class_svg_from_text(text: &str) -> String {
    render_class_svg_from_text_with_engine(Engine::new(), text)
}

fn render_class_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    render_class_svg_from_text_with_engine_and_options(
        engine,
        text,
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    )
}

fn render_class_svg_from_text_with_engine_and_options(
    engine: Engine,
    text: &str,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    render_class_svg_from_text_with_session(engine, text, layout_options, svg_options, session)
}

fn render_class_svg_from_text_with_session(
    engine: Engine,
    text: &str,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    session: RenderSession,
) -> String {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, layout_options, session).expect("layout ok");

    artifact
        .render_svg(svg_options, &SvgDebugOptions::default())
        .expect("svg render ok")
        .svg()
        .to_owned()
}

fn render_class_fixture(
    name: &str,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> String {
    let path = workspace_root().join("fixtures").join("class").join(name);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_class_svg_from_text_with_engine_and_options(
        Engine::new(),
        &text,
        layout_options,
        svg_options,
    )
}

fn attr_f64(tag: &str, name: &str) -> f64 {
    let prefix = format!(r#"{name}=""#);
    let start = tag.find(&prefix).expect("attribute") + prefix.len();
    let end = start + tag[start..].find('"').expect("attribute end");
    tag[start..end].parse().expect("numeric attribute")
}

fn foreign_object_for_text<'a>(svg: &'a str, text: &str) -> &'a str {
    let text_start = svg.find(text).expect("text");
    let start = svg[..text_start]
        .rfind("<foreignObject ")
        .expect("foreignObject before text");
    let end = text_start
        + svg[text_start..]
            .find("</foreignObject>")
            .expect("foreignObject after text")
        + "</foreignObject>".len();
    &svg[start..end]
}

fn embedded_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid SVG");
    document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "style")
        .and_then(|node| node.text())
        .expect("embedded stylesheet")
        .to_owned()
}

fn class_model(parsed: &ParsedDiagramRender) -> &merman_core::models::class_diagram::ClassDiagram {
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class render model");
    };
    model
}

fn layout_class_with_dagre(
    parsed: &merman_core::ParsedDiagramRender,
    session: RenderSession,
) -> merman_render::model::ClassDiagramLayout {
    let artifact =
        family::prepare(parsed.clone(), &LayoutOptions::default(), session).expect("Class layout");
    serde_json::from_value(
        artifact.layout_json().expect("Class layout projection")["layout"]["ClassDiagramV2"]
            .clone(),
    )
    .expect("Class layout")
}

fn deep_class_namespace_text(depth: usize) -> String {
    let mut lines = vec!["classDiagram".to_string()];
    for i in 0..depth {
        lines.push(format!("{}namespace N{i} {{", "  ".repeat(i)));
    }
    lines.push(format!("{}class Leaf", "  ".repeat(depth)));
    for i in (0..depth).rev() {
        lines.push(format!("{}}}", "  ".repeat(i)));
    }
    lines.join("\n")
}

#[test]
fn class_svg_root_role_comes_from_the_detected_mermaid_diagram_id() {
    for (source, expected_role) in [
        ("classDiagram\nclass Animal\n", "classDiagram"),
        ("classDiagram-v2\nclass Animal\n", "classDiagram"),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-wrapper\"}}}%%\nclassDiagram\nclass Animal\n",
            "classDiagram",
        ),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\nclassDiagram\nclass Animal\n",
            "classDiagram",
        ),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\nclassDiagram\nclass Animal\nnote for Animal \"classDiagram-v2 is note text\"\n",
            "classDiagram",
        ),
    ] {
        let engine = Engine::new();
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .expect("parse ok")
            .expect("diagram detected");
        assert_eq!(
            parsed.metadata().diagram_type,
            expected_role,
            "Class detection must follow Mermaid 12's syntax-only detector contract for {source:?}"
        );
        assert_eq!(
            class_model(&parsed).diagram_type,
            parsed.metadata().diagram_type,
            "the typed Class model must preserve the detector-selected diagram id for {source:?}"
        );
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("layout ok");
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("svg render ok")
            .svg()
            .to_owned();
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");

        assert_eq!(
            document.root_element().attribute("aria-roledescription"),
            Some(expected_role),
            "Class accessibility role must preserve Mermaid's selected detector id for {source:?}"
        );
    }
}

#[test]
fn class_svg_unified_titles_inherit_root_font_size_for_both_detector_aliases() {
    for diagram_keyword in ["classDiagram", "classDiagram-v2"] {
        let source = format!(
            r##"---
title: Inherited title
---
%%{{init: {{"themeVariables": {{"fontSize": "23px"}}}} }}%%
{diagram_keyword}
class Animal
"##
        );
        let svg = render_class_svg_from_text_with_engine(
            legacy_init_theme_compat_engine(),
            source.as_str(),
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
        let root = document.root_element();
        let css = document
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "style")
            .and_then(|node| node.text())
            .expect("embedded Class stylesheet");
        let title = root
            .children()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "text"
                    && node.attribute("class") == Some("classDiagramTitleText")
            })
            .expect("unified Class diagram title");

        let root_rule_start = css.find("#merman{").expect("root font rule");
        let root_rule_end = root_rule_start
            + css[root_rule_start..]
                .find('}')
                .expect("root font rule end");
        let root_rule = &css[root_rule_start..=root_rule_end];

        assert!(
            root_rule.contains("font-size:23px;"),
            "{diagram_keyword} title should inherit the configured root font size: {root_rule}"
        );
        assert!(
            css.contains(".classTitleText{text-anchor:middle;font-size:18px;"),
            "Class CSS should preserve Mermaid's legacy title selector"
        );
        assert!(
            !css.contains(".classDiagramTitleText"),
            "the unified title class must not be captured by the legacy 18px selector"
        );
        assert_eq!(title.text(), Some("Inherited title"));
        assert_eq!(title.attribute("font-size"), None);
        assert_eq!(title.attribute("style"), None);
    }
}

#[test]
fn class_stylesheet_matches_signed_mermaid_12_css_contract() {
    const FIXTURE: &str = "stress_class_many_relations_labels_020";
    let local_svg = render_class_fixture(
        &format!("{FIXTURE}.mmd"),
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );
    let upstream_svg = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("upstream-svgs")
            .join("class")
            .join(format!("{FIXTURE}.svg")),
    )
    .expect("signed Mermaid Class SVG");

    let local_css = embedded_stylesheet(&local_svg).replace("#merman", "#class-contract");
    let upstream_css =
        embedded_stylesheet(&upstream_svg).replace(&format!("#{FIXTURE}"), "#class-contract");

    assert_eq!(
        local_css, upstream_css,
        "Class CSS must preserve Mermaid's common prefix, family rules, icon rules, common Neo rules, and final :root order"
    );
    assert!(
        local_css.starts_with(
            r#"#class-contract{font-family:"Recursive Variable",arial,sans-serif;font-size:14px;fill:#28253D;}"#
        ),
        "the public root fill must use textColor rather than classText"
    );
}

#[test]
fn class_parse_for_render_model_handles_deep_namespace_chain() {
    const DEPTH: usize = 128;
    let source = deep_class_namespace_text(DEPTH);
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-parse".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            let engine = Engine::new();
            engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                .expect("parse ok")
                .expect("diagram detected");
        })
        .expect("spawn deep namespace parse test");
    handle
        .join()
        .expect("deep namespace parse should finish without stack overflow");
}

#[test]
fn class_layout_handles_deep_namespace_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 128;
    let source = deep_class_namespace_text(DEPTH);
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-layout".to_string())
        // This regression verifies that depth-128 namespace layout terminates without recursive
        // stack growth. The full debug Dagre pipeline has substantial fixed phase frames, so the
        // test does not claim that production rendering supports a 128 KiB thread stack.
        .stack_size(256 * 1024)
        .spawn(move || {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                .expect("parse ok")
                .expect("diagram detected");
            let layout = layout_class_with_dagre(&parsed, session);
            assert!(
                layout.nodes.iter().any(|node| node.id == "Leaf"),
                "expected deeply nested class member to remain in the layout"
            );
        })
        .expect("spawn deep namespace layout test");
    handle
        .join()
        .expect("deep namespace layout should finish without stack overflow");
}

#[test]
fn class_svg_dotted_namespace_titles_use_hierarchical_segment_labels() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
namespace Company.Project.Module {
  class User
}
"#,
    );

    assert!(svg.contains(r#"id="merman-Company" data-look="neo""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="neo""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="neo""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected default hierarchical namespace labels to use path segments"
    );
    assert!(
        !svg.contains("<p>Company.Project.Module</p>"),
        "default hierarchical mode should not render the full dotted id as the leaf label"
    );
}

#[test]
fn class_svg_scopes_text_color_for_html_labels() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
    class Animal {
        +String name
        +int age
        +makeSound()
    }
"#,
    );

    assert!(
        svg.contains(r#"#merman p{margin:0;}"#),
        "expected class SVG to reset HTML label paragraph margins"
    );
    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#28253D;}"#),
        "expected class SVG to make HTML labels self-contained instead of inheriting host page color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#28253D;}"#),
        "expected class SVG text labels to get an explicit fill color"
    );
}

#[test]
fn class_svg_honors_configured_class_text_color() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"classText": "#123456"}}}%%
classDiagram
    class Animal
"##,
    );

    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#123456;}"#),
        "expected classText theme variable to drive HTML label color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#123456;}"#),
        "expected classText theme variable to drive SVG text fill"
    );
}

#[test]
fn class_svg_uses_configured_look_in_dom_attributes() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "neo"}}%%
classDiagram
namespace Zoo {
  class Animal
  class Keeper
}
Animal --> Keeper
"#,
    );

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected class SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured class look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_basic_node_uses_rough_wrapper_and_hachure_paths() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"layout": "dagre", "look": "handDrawn", "handDrawnSeed": 7, "themeVariables": {"mainBkg": "#f8fafc", "nodeBorder": "#ef4444", "useGradient": true, "gradientStart": "#112233", "gradientStop": "#445566"}}}%%
classDiagram
  class Class10
"##,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-classId-Class10-0"))
        .expect("hand-drawn Class10 node");
    assert_eq!(node.attribute("class"), Some("rough-node default"));
    assert_eq!(node.attribute("data-look"), Some("handDrawn"));
    assert!(
        svg.contains(r#"<g class="basic label-container outer-path"><path d=""#)
            && svg.contains(
                r##"stroke="#f8fafc" stroke-width="1.5" fill="none" stroke-dasharray="0 0"/><path d=""##
            )
            && svg.contains(
                r##"stroke="#ef4444" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##
            ),
        "hand-drawn class node should render RoughJS hachure fill and outline paths: {svg}"
    );
    let marker_ids = [
        "aggregationStart",
        "aggregationEnd",
        "aggregationStart-margin",
        "aggregationEnd-margin",
        "extensionStart",
        "extensionEnd",
        "extensionStart-margin",
        "extensionEnd-margin",
        "compositionStart",
        "compositionEnd",
        "compositionStart-margin",
        "compositionEnd-margin",
        "dependencyStart",
        "dependencyEnd",
        "dependencyStart-margin",
        "dependencyEnd-margin",
        "lollipopStart",
        "lollipopEnd",
        "lollipopStart-margin",
        "lollipopEnd-margin",
    ];
    let diagram_role = document
        .root_element()
        .attribute("aria-roledescription")
        .expect("Class diagram role");
    assert_eq!(diagram_role, "classDiagram");
    for marker_id in marker_ids.iter().filter(|id| !id.ends_with("-margin")) {
        let marker = document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node.attribute("id")
                        == Some(format!("merman_{diagram_role}-{marker_id}").as_str())
            })
            .unwrap_or_else(|| panic!("missing marker element {marker_id}: {svg}"));
        assert_eq!(
            marker.attribute("markerUnits"),
            Some("userSpaceOnUse"),
            "plain Class marker {marker_id} must not scale with relation stroke width"
        );
    }
    let marker_positions = marker_ids.map(|marker_id| {
        let marker_attr = format!(r#"id="merman_{diagram_role}-{marker_id}""#);
        svg.find(&marker_attr)
            .unwrap_or_else(|| panic!("missing Dagre class marker {marker_id}: {svg}"))
    });
    assert!(
        marker_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "hand-drawn class marker variants should preserve Mermaid's insertion order: {svg}"
    );
    let graph_end = svg
        .find(r#"</g><defs><filter id="merman-drop-shadow""#)
        .expect("hand-drawn class SVG should append shared resources after the graph wrapper");
    let small_shadow = svg
        .find(r#"<defs><filter id="merman-drop-shadow-small""#)
        .expect("hand-drawn class SVG should include the shared small shadow filter");
    let gradient = svg
        .find(r#"<linearGradient id="merman-gradient""#)
        .expect("hand-drawn class SVG should include a configured root gradient");
    assert!(
        graph_end < small_shadow && small_shadow < gradient,
        "shared shadow filters should preserve Mermaid root resource order: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_inline_styles_reach_rough_paths_and_labels() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
classDiagram
  class Class10
  style Class10 fill:#f9f,stroke:#333,stroke-width:4px,color:white
"##,
    );

    assert!(
        svg.contains(r#"class="rough-node default""#)
            && svg.contains(r##"stroke="#f9f" stroke-width="1.5" fill="none""##)
            && svg.contains(r##"stroke="#333" stroke-width="4" fill="none" stroke-dasharray="0 0" style="fill:#f9f;stroke:#333;stroke-width:4px;color:white""##)
            && svg.contains(r##"style="fill:#f9f;stroke:#333;stroke-width:4px;color:white"><p>Class10</p>"##),
        "inline style should reach hand-drawn class rough paths and label span: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_notes_use_rough_wrapper_and_note_hachure_paths() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7, "themeVariables": {"noteBkgColor": "#fff5ad", "noteBorderColor": "#aaaa33"}}}%%
classDiagram
  note "hello"
"##,
    );

    assert!(
        svg.contains(r#"<g class="rough-node undefined" id="merman-note0" data-look="handDrawn""#),
        "hand-drawn class note should use Mermaid's rough-node wrapper class: {svg}"
    );
    assert!(
        svg.contains(r#"<g class="basic label-container outer-path"><path d=""#)
            && svg.contains(
                r##"stroke="#fff5ad" stroke-width="1.5" fill="none" stroke-dasharray="0 0"/><path d=""##
            )
            && svg.contains(
                r##"stroke="#aaaa33" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/>"##
            )
            && svg.contains(r#"<g class="label noteLabel""#)
            && svg.contains(r#"class="nodeLabel markdown-node-label""#),
        "hand-drawn class note should render note-colored hachure fill and outline paths: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_edges_use_rough_transition_class() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
classDiagram
  class A
  class B
  A --> B
  note for A "hello"
"#,
    );

    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid transition relation""#)
            && svg.contains(r##"stroke="#000" stroke-width="1" fill="none""##)
            && svg.contains(r#"id="merman-id_A_B_1""#)
            && svg.contains(r#"data-id="id_A_B_1""#)
            && svg.contains(r#"id="merman-edgeNote0""#)
            && svg.contains(r#"data-id="edgeNote0""#)
            && svg.contains(r#"data-look="handDrawn""#),
        "hand-drawn class relations should use RoughJS transition edge DOM: {svg}"
    );
}

#[test]
fn class_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_class_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
classDiagram
class Class1
click Class1 href "javascript:alert(1)" "tip" _self
"#,
    );
    assert!(
        strict.contains(r#"<a data-look=""#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared Class link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe Class click href from SVG: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="about:blank""#),
        "expected Mermaid-compatible strict Class SVG to omit sanitized about:blank href: {strict}"
    );

    let loose = render_class_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"classDiagram
class Class1
click Class1 href "notes://do-your-thing/id" "tip" _self
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="notes://do-your-thing/id""#),
        "expected loose mode to preserve Mermaid formatUrl-compatible Class custom protocols: {loose}"
    );
    assert!(
        loose.contains(r#"target="_self""#),
        "expected loose class parity to preserve the Mermaid link target: {loose}"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_missing_section_with_short_cardinality_path_reports_source_error() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: elk.box\n---\nclassDiagram\nA \"1\" --> \"many\" B\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap();
    let result = family::prepare(
        parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    );
    assert!(
        matches!(result, Err(merman_render::Error::InvalidModel { message })
        if message.contains("Could not find a suitable point for the given distance"))
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_and_er_elk_missing_sections_paint_clipped_linear_edges_and_centered_labels() {
    use base64::Engine as _;
    use merman_render::model::{LayoutEdge, LayoutNode, LayoutPoint};

    for algorithm in ["elk.box", "elk.rectpacking"] {
        for (diagram, projection) in [
            (
                "classDiagram\nclass A\nclass B\nclass C\nclass D\nclass E\nclass F\nclass G\nclass H\nA \"1\" --> \"many\" H : a deliberately wide relationship label\n",
                "ClassDiagramV2",
            ),
            (
                "erDiagram\nA ||--o{ WiderTarget : \"a deliberately wide relationship label\"\n",
                "ErDiagram",
            ),
        ] {
            let text = format!("---\nconfig:\n  layout: {algorithm}\n---\n{diagram}");
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&text, ParseOptions::default())
                .expect("parse diagram")
                .expect("diagram detected");
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact =
                family::prepare(parsed, &LayoutOptions::default(), session).expect("layout");
            let json = artifact.layout_json().expect("layout projection");
            let layout = &json["layout"][projection];
            let nodes: Vec<LayoutNode> = serde_json::from_value(layout["nodes"].clone()).unwrap();
            let edges: Vec<LayoutEdge> = serde_json::from_value(layout["edges"].clone()).unwrap();
            assert_eq!(edges.len(), 1);
            let edge = &edges[0];
            if projection == "ClassDiagramV2" {
                assert!(
                    edge.start_label_right.is_some(),
                    "{algorithm}: start cardinality missing: {nodes:?}"
                );
                assert!(
                    edge.end_label_left.is_some(),
                    "end cardinality must survive missing sections"
                );
            }
            assert!(
                edge.points.is_empty(),
                "{algorithm} must preserve absent provider sections"
            );
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("SVG")
                .svg()
                .to_owned();
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            if projection == "ClassDiagramV2" {
                let terminals: Vec<_> = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("edgeTerminals")
                    })
                    .collect();
                assert_eq!(terminals.len(), 2, "both cardinalities must be painted");
                for terminal in terminals {
                    assert!(terminal.attribute("transform").is_some());
                }
            }
            let path = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("path") && node.attribute("data-edge") == Some("true")
                })
                .expect("visible edge");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(points.len(), 2);
            for (point, id) in points.iter().zip([&edge.from, &edge.to]) {
                let node = nodes.iter().find(|node| &node.id == id).unwrap();
                let dx = (point.x - node.x).abs();
                let dy = (point.y - node.y).abs();
                assert!(dx <= node.width / 2.0 + 1e-5 && dy <= node.height / 2.0 + 1e-5);
                assert!(
                    (dx - node.width / 2.0).abs() < 1e-5 || (dy - node.height / 2.0).abs() < 1e-5
                );
            }
            let d = path.attribute("d").unwrap();
            assert!(
                d.contains('L') && !d.contains('C') && !d.contains('Q'),
                "{d}"
            );
            let label = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("edgeLabel")
                        && node.attribute("transform").is_some()
                })
                .expect("positioned label");
            let transform = label.attribute("transform").unwrap();
            let center: Vec<f64> = transform
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(',')
                .map(|part| part.trim().parse().unwrap())
                .collect();
            assert!((center[0] - (points[0].x + points[1].x) / 2.0).abs() < 1e-5);
            assert!((center[1] - (points[0].y + points[1].y) / 2.0).abs() < 1e-5);
            let viewbox: Vec<f64> = document
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_ascii_whitespace()
                .map(|part| part.parse().unwrap())
                .collect();
            let measured_label = edge.label.as_ref().expect("measured label");
            assert!(center[0] - measured_label.width / 2.0 >= viewbox[0] - 1e-5);
            assert!(center[0] + measured_label.width / 2.0 <= viewbox[0] + viewbox[2] + 1e-5);
            assert!(center[1] - measured_label.height / 2.0 >= viewbox[1] - 1e-5);
            assert!(center[1] + measured_label.height / 2.0 <= viewbox[1] + viewbox[3] + 1e-5);
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_svg_elk_layout_preserves_existing_renderer_semantics() {
    let svg = render_class_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r##"---
config:
  layout: elk
---
classDiagram
direction LR
namespace Platform {
  class Service:::critical {
    +start()
  }
}
class Client {
  +request()
}
Client "1" --> "many" Service : calls
note for Service "ELK note"
click Service href "https://example.com/service" "Open Service" _blank
classDef critical fill:#ffdddd,stroke:#aa0000,stroke-width:2px,color:#111111
style Client fill:#ddffdd,stroke:#00aa00,stroke-width:2px
"##,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let namespace = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-Platform"))
        .expect("Platform namespace");
    assert_eq!(namespace.attribute("data-look"), Some("neo"));
    let service = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-classId-Service-0"))
        .expect("Service class node");
    let service_link = service.parent_element().expect("Service link wrapper");
    assert!(service_link.has_tag_name("a"));
    assert_eq!(service_link.attribute("data-look"), Some("neo"));
    assert_eq!(
        service_link.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.com/service")
    );
    assert!(document.descendants().any(|node| {
        node.attribute("id")
            .is_some_and(|id| id.starts_with("merman-classId-Client-"))
    }));
    assert!(
        svg.contains(r#"xlink:href="https://example.com/service""#)
            && svg.contains(r#"title="Open Service""#),
        "Class ELK layout should preserve Class click/link SVG semantics: {svg}"
    );
    assert!(
        svg.contains(r#"style="fill:#ffdddd;stroke:#aa0000;stroke-width:2px;color:#111111""#)
            && svg.contains(r#"style="fill:#ddffdd;stroke:#00aa00;stroke-width:2px""#),
        "Class ELK layout should preserve classDef and inline style SVG semantics: {svg}"
    );
    assert!(
        svg.contains("<p>ELK note</p>")
            && svg.contains(r#"<span class="edgeLabel"><p>calls</p></span>"#)
            && svg.contains(r#"<span class="edgeLabel"><p>many</p></span>"#),
        "Class ELK layout should preserve notes, relation labels, and cardinality terminals: {svg}"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_svg_elk_layout_uses_common_painter_dom() {
    let svg = render_class_svg_from_text(
        r#"---
config:
  layout: elk
---
classDiagram
direction LR
class Animal
class Duck
Animal <|-- Duck
"#,
    );

    let root = svg
        .find(r#"<g class="root""#)
        .expect("Class ELK common root group");
    let nodes = svg
        .find(r#"<g class="nodes""#)
        .expect("Class ELK nodes group");
    let edges = svg
        .find(r#"<g class="edgePaths edges""#)
        .expect("Class ELK edge paths group");
    let labels = svg
        .find(r#"<g class="edgeLabels""#)
        .expect("Class ELK edge labels group");
    let clusters = svg
        .find(r#"<g class="clusters""#)
        .expect("Class ELK clusters group");
    assert!(root < edges && edges < clusters && clusters < labels && labels < nodes);
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_svg_elk_layout_uses_shared_mermaid12_markers() {
    let svg = render_class_svg_from_text(
        r#"---
config:
  layout: elk
---
classDiagram
class C1["One"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class ELK SVG");
    let marker_units = |name: &str| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node.attribute("id").is_some_and(|id| id.ends_with(name))
            })
            .and_then(|node| node.attribute("markerUnits"))
    };
    for marker in [
        "aggregationStart",
        "aggregationEnd",
        "extensionEnd",
        "compositionStart",
        "compositionEnd",
        "dependencyStart",
        "dependencyEnd",
        "lollipopStart",
        "lollipopEnd",
    ] {
        assert_eq!(
            marker_units(marker),
            Some("userSpaceOnUse"),
            "Mermaid 12 shared ordinary marker {marker}"
        );
    }
    assert_eq!(marker_units("extensionStart"), Some("userSpaceOnUse"));
    assert_eq!(
        marker_units("aggregationStart-margin"),
        Some("userSpaceOnUse")
    );
}

#[test]
fn class_svg_namespace_clusters_keep_theme_fill() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
namespace Platform {
  class Api
}
namespace Platform.FFI {
  class Bridge
}
namespace Platform.Core {
  class Engine
}
"#,
    );

    assert!(
        svg.contains(r#"#merman .cluster rect{fill:#F9F9FB;stroke:#BDBCCC;stroke-width:1px;}"#),
        "expected class namespace cluster CSS to provide the Mermaid 12 default theme fill: {svg}"
    );
    assert!(
        !svg.contains(r#"style="fill:none !important;stroke:black !important""#),
        "namespace cluster rects must not override the theme fill with transparent inline CSS: {svg}"
    );
}

#[test]
fn class_svg_honors_numeric_stroke_width_theme_css() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"mainBkg": "#112233", "nodeBorder": "#445566", "lineColor": "#778899", "strokeWidth": 7}}}%%
classDiagram
    Animal <|-- Dog
    class Animal
    class Dog
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node rect,#merman .node circle,#merman .node ellipse,#merman .node polygon,#merman .node path{fill:#112233;stroke:#445566;stroke-width:7;}"#
        ),
        "expected numeric strokeWidth to drive Class node shape CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:7px;}"#),
        "expected numeric strokeWidth to drive Mermaid's common edge CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .divider{stroke:#445566;stroke-width:1;}"#),
        "expected nodeBorder to drive Class divider CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .relation{stroke:#778899;stroke-width:7;fill:none;}"#),
        "expected numeric strokeWidth to drive Class relation CSS: {svg}"
    );
    assert!(
        !svg.contains(r#"#merman .relation{stroke:#778899;stroke-width:1;fill:none;}"#),
        "Class relation CSS must not drop numeric strokeWidth overrides: {svg}"
    );
}

#[test]
fn class_svg_honors_configured_note_theme_colors() {
    for html_labels in [true, false] {
        let svg = render_class_svg_from_text_with_engine(
            legacy_init_theme_compat_engine(),
            &format!(
                r##"%%{{init: {{"htmlLabels": {html_labels}, "themeVariables": {{"noteBkgColor": "#112233", "noteBorderColor": "#445566", "noteTextColor": "#778899"}}}}}}%%
classDiagram
    class Animal
    note for Animal "hello"
"##
            ),
        );

        assert!(
            svg.contains(
                r##"fill="#112233" style="fill:#112233 !important;stroke:#445566 !important""##
            ),
            "expected configured noteBkgColor/noteBorderColor in note body for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            svg.contains(r##"stroke="#445566" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="fill:#112233 !important;stroke:#445566 !important""##),
            "expected configured noteBorderColor in note rough stroke for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            svg.contains(
                r#"#merman .noteLabel .nodeLabel,#merman .noteLabel .edgeLabel{color:#778899;}"#
            ),
            "expected noteTextColor CSS for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            !svg.contains(
                r##"fill="#fff5ad" style="fill:#fff5ad !important;stroke:#aaaa33 !important""##
            ),
            "note shape must not ignore configured colors for htmlLabels={html_labels}: {svg}"
        );
    }
}

#[test]
fn class_svg_namespaces_use_hierarchical_labels_and_keep_relation_label() {
    let svg = render_class_fixture(
        "upstream_namespaces_and_generics.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(svg.contains(r#"id="merman-Company" data-look="neo""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="neo""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="neo""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected dotted namespace labels to use Mermaid path segments"
    );
    let company_pos = svg
        .find(r#"id="merman-Company""#)
        .expect("Company namespace cluster");
    let project_pos = svg
        .find(r#"id="merman-Company.Project""#)
        .expect("Project namespace cluster");
    let module_pos = svg
        .find(r#"id="merman-Company.Project.Module""#)
        .expect("Module namespace cluster");
    assert!(
        company_pos < project_pos && project_pos < module_pos,
        "namespace clusters must be emitted parent-first so nested frames remain visible"
    );
    assert!(
        svg.contains("<p>manages</p>"),
        "expected relation label text to survive hierarchical namespace rendering"
    );
}

#[test]
fn class_svg_nested_namespace_relation_endpoints_share_node_coordinate_frame() {
    use base64::Engine as _;

    let svg = render_class_fixture(
        "upstream_namespaces_and_generics.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("data-id") == Some("id_Admin_User_1")
        })
        .expect("Admin to User relation path");
    let encoded = edge.attribute("data-points").expect("relation data-points");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 Class edge points");
    let points: Vec<merman_render::model::LayoutPoint> =
        serde_json::from_slice(&decoded).expect("JSON Class edge points");

    let owning_root = |node: roxmltree::Node<'_, '_>| {
        node.ancestors()
            .find(|ancestor| {
                ancestor.has_tag_name("g")
                    && ancestor.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "root")
                    })
            })
            .expect("recursive Class root")
            .id()
    };
    let node_bounds = |class_id: &str| {
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(&format!("classId-{class_id}-")))
            })
            .unwrap_or_else(|| panic!("missing {class_id} node"));
        let transform = node.attribute("transform").expect("node transform");
        let center_x = transform
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, _)| x.trim().parse::<f64>().expect("node x"))
            .expect("translate(x, y)");
        let outer = node
            .descendants()
            .find(|descendant| {
                descendant.has_tag_name("g")
                    && descendant.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "outer-path")
                    })
            })
            .and_then(|group| group.children().find(|child| child.has_tag_name("path")))
            .expect("class outer path");
        let xs = outer
            .attribute("d")
            .expect("class outer path data")
            .split_whitespace()
            .filter_map(|component| {
                component
                    .strip_prefix('M')
                    .or_else(|| component.strip_prefix('L'))
            })
            .map(|x| x.parse::<f64>().expect("class outer path x"))
            .collect::<Vec<_>>();
        let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (node, center_x + min_x, center_x + max_x)
    };
    let (admin, _admin_left, admin_right) = node_bounds("Admin");
    let (user, user_left, _user_right) = node_bounds("User");

    assert_eq!(owning_root(edge), owning_root(admin));
    assert_eq!(owning_root(edge), owning_root(user));
    assert!(
        (points.first().expect("source endpoint").x - admin_right).abs() <= 0.01,
        "Admin relation endpoint must touch the rendered node boundary"
    );
    assert!(
        (points.last().expect("target endpoint").x - user_left).abs() <= 0.01,
        "User relation endpoint must touch the rendered node boundary before marker shortening"
    );
}

#[test]
fn class_svg_nested_namespace_subgraphs_keep_mermaid_wrapper_structure() {
    let text = std::fs::read_to_string(
        workspace_root().join("fixtures/class/stress_class_comments_inside_namespaces_024.mmd"),
    )
    .expect("fixture");
    let svg = render_class_svg_from_text_with_engine_and_options(
        Engine::new(),
        &format!("%%{{init: {{\"layout\": \"dagre\"}}}}%%\n{text}"),
        &LayoutOptions::default(),
        &SvgRenderOptions {
            diagram_id: Some("stress_class_comments_inside_namespaces_024".to_string()),
            ..Default::default()
        },
    );

    assert!(
        svg.contains(r#"<g class="root" transform="translate("#)
            && svg.contains(r#"><g class="clusters">"#),
        "expected nested namespace wrapper around the cluster group"
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let wrapper = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
                && node.attribute("transform").is_some()
        })
        .expect("nested namespace root wrapper");
    let child_classes = wrapper
        .children()
        .filter(|node| node.is_element())
        .filter_map(|node| node.attribute("class"))
        .collect::<Vec<_>>();
    assert_eq!(
        child_classes.get(..4),
        Some(["clusters", "edgePaths", "edgeLabels", "nodes"].as_slice()),
        "nested namespace wrapper must keep Mermaid's direct-child order"
    );
    assert!(svg.contains("<p>Outer.Foo</p>"));
}

#[test]
fn class_svg_namespace_extraction_depends_on_cross_boundary_edges() {
    let extracted = render_class_svg_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
classDiagram
namespace Internal {
  class A
  note for A "inside"
}
class X
class Y
X --> Y
"#,
    );

    let document = roxmltree::Document::parse(&extracted).expect("valid extracted Class SVG");
    let outer_edge = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "path"
                && node
                    .attribute("data-id")
                    .is_some_and(|id| id.starts_with("id_X_Y_"))
        })
        .expect("outer X-to-Y edge path");
    let outer_edge_roots = outer_edge
        .ancestors()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        outer_edge_roots.len(),
        1,
        "an unrelated outer relation belongs only to the top-level Dagre render root"
    );

    let note_edge = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "path"
                && node.attribute("data-id") == Some("edgeNote0")
        })
        .expect("internal note edge path");
    let note_edge_roots = note_edge
        .ancestors()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        note_edge_roots.len(),
        2,
        "the internal note edge belongs to the extracted namespace root nested in the top-level root"
    );

    for root in [note_edge_roots[1], note_edge_roots[0]] {
        let child_classes = root
            .children()
            .filter(|node| node.is_element())
            .filter_map(|node| node.attribute("class"))
            .collect::<Vec<_>>();
        assert_eq!(
            child_classes,
            ["clusters", "edgePaths", "edgeLabels", "nodes"],
            "each layout-owned render root must preserve Mermaid's direct-child group order"
        );
    }

    let retained = render_class_svg_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
classDiagram
namespace Internal {
  class A
}
class Outside
A --> Outside
"#,
    );

    let document = roxmltree::Document::parse(&retained).expect("valid retained Class SVG");
    let crossing_edge = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "path"
                && node
                    .attribute("data-id")
                    .is_some_and(|id| id.starts_with("id_A_Outside_"))
        })
        .expect("namespace-crossing edge path");
    let crossing_edge_root_count = crossing_edge
        .ancestors()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
        })
        .count();
    assert_eq!(
        crossing_edge_root_count, 1,
        "a boundary-crossing relation must remain owned by the parent Dagre render root"
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("class") == Some("root")
                    && node.attribute("transform").is_some()
            })
            .count(),
        0,
        "a boundary-crossing relation prevents extraction of its namespace cluster"
    );
}

#[test]
fn class_svg_multiple_dotted_namespace_subgraphs_use_segment_labels() {
    let svg = render_class_fixture(
        "stress_class_nested_namespaces_many_levels_021.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions {
            diagram_id: Some("stress_class_nested_namespaces_many_levels_021".to_string()),
            ..Default::default()
        },
    );

    assert!(
        svg.contains(
            r#"id="stress_class_nested_namespaces_many_levels_021-Root.A" data-look="neo""#
        )
    );
    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.B.B1" data-look="neo""#
    ));
    assert!(
        svg.contains("<p>A</p>") && svg.contains("<p>B1</p>"),
        "expected rendered dotted namespace clusters to use path-segment labels"
    );
    assert!(
        svg.contains("<p>Root.A.A1</p>") && svg.contains("<p>Root.B.B1.B1a</p>"),
        "expected qualified relation facade class labels to remain visible"
    );
}

#[test]
fn class_svg_handles_deep_namespace_subgraph_chain() {
    const DEPTH: usize = 128;
    let source = deep_class_namespace_text(DEPTH);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-svg".to_string())
        // Keep the SVG path under a constrained stack while leaving room for its fixed debug
        // render frames; depth remains the recursion-safety signal exercised by this test.
        .stack_size(256 * 1024)
        .spawn(move || {
            render_class_svg_from_text_with_session(
                Engine::new(),
                &source,
                &LayoutOptions::headless_svg_defaults(),
                &SvgRenderOptions::default(),
                session,
            )
        })
        .expect("spawn deep namespace SVG test");
    let svg = handle
        .join()
        .expect("deep namespace SVG render should finish without stack overflow");

    assert!(
        svg.contains("Leaf"),
        "expected deeply nested class member to remain visible"
    );
    assert!(
        svg.contains(r#"id="merman-N0""#),
        "expected outer namespace cluster to be rendered"
    );
    assert!(
        svg.contains("N127"),
        "expected deepest namespace cluster to be rendered"
    );
}

#[test]
fn class_svg_long_relation_labels_wrap_to_mermaid_html_cap() {
    let svg = render_class_fixture(
        "stress_class_long_labels_wrapping_002.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );

    let label = foreign_object_for_text(
        &svg,
        "<p>edge label with spaces, punctuation, and unicode → αβγ</p>",
    );
    assert!(attr_f64(label, "width") > 0.0 && attr_f64(label, "width") <= 200.0);
    assert!(
        attr_f64(label, "height") > 16.0,
        "long label should wrap: {label}"
    );
    assert!(label.contains("max-width: 200px"));
}

#[test]
fn class_svg_cardinality_terminals_emit_positive_measured_bounds() {
    let svg = render_class_fixture(
        "upstream_relation_types_and_cardinalities_spec.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );

    let terminal = foreign_object_for_text(&svg, "<p>many</p>");
    assert!(svg.contains(r#"<span class="edgeLabel"><p>many</p></span>"#));
    assert!(attr_f64(terminal, "width") > 0.0);
    assert!(attr_f64(terminal, "height") > 0.0);
}

#[test]
fn class_svg_authored_none_cardinalities_are_not_missing_labels() {
    for diagram_type in ["classDiagram", "classDiagram-v2"] {
        let svg = render_class_svg_from_text(&format!(
            "{diagram_type}\nclass A\nclass B\nA \"none\" --> \"NONE\" B"
        ));

        for label in ["none", "NONE"] {
            assert!(
                svg.contains(&format!("<p>{label}</p>")),
                "{diagram_type} should preserve authored endpoint label {label:?}: {svg}"
            );
        }
    }
}

#[test]
fn class_svg_hand_drawn_cardinality_terminals_keep_xhtml_and_measured_bounds() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
classDiagram
  A "1" --> "many" B
"#,
    );

    let terminal = foreign_object_for_text(&svg, "<p>1</p>");
    assert!(svg.contains(r#"<span class="edgeLabel"><p>1</p></span>"#));
    assert!(attr_f64(terminal, "width") > 0.0);
    assert!(attr_f64(terminal, "height") > 0.0);
}

#[test]
fn class_svg_dagre_edge_labels_precede_terminals_in_edge_labels_group() {
    let svg = render_class_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "layout": "dagre",
        }))),
        include_str!("../../../fixtures/class/stress_class_parallel_edges_and_cardinality_004.mmd"),
    );

    let edge_labels_start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("edgeLabels group");
    let nodes_start = svg[edge_labels_start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| edge_labels_start + idx)
        .expect("nodes group after edge labels");
    let section = &svg[edge_labels_start..nodes_start];
    let last_label = section
        .rfind(r#"<g class="edgeLabel""#)
        .expect("edgeLabel group present");
    let first_terminal = section
        .find(r#"<g class="edgeTerminals""#)
        .expect("edge terminal group present");

    assert!(
        last_label < first_terminal,
        "expected Mermaid-style edgeLabels ordering: all edgeLabel groups before edgeTerminals"
    );
}

#[test]
fn class_svg_relation_titles_decode_entities_once() {
    let svg = render_class_fixture(
        "upstream_relation_types_and_cardinalities_spec.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains(r#"<p>&lt; owns</p>"#),
        "expected relation title entities to render exactly once"
    );
    assert!(
        !svg.contains("&amp;lt; owns"),
        "expected relation title entities to avoid double escaping"
    );
}

#[test]
fn class_svg_relation_only_generic_nodes_keep_type_suffix() {
    let svg = render_class_fixture(
        "upstream_cypress_classdiagram_v3_spec_8_should_render_a_simple_class_diagram_with_generic_class_and_re_016.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains("Class01&lt;T")
            && svg.contains("Class03&lt;T")
            && svg.contains("Class04&lt;T"),
        "expected relation-only generic classes to keep Mermaid-matching type suffixes"
    );
}

#[test]
fn class_svg_preserves_numeric_theme_font_size_css_spelling() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"fontSize": 10, "themeVariables": {"fontSize": 24}, "htmlLabels": false} }%%
classDiagram
  class FontSizeSvgProbe {
    +veryLongMethodNameToForceMeasurement()
  }
"##,
    );

    let css = embedded_stylesheet(&svg);
    let root_rule = css.split_once('}').expect("root stylesheet rule").0;
    assert!(root_rule.starts_with("#merman{"), "{root_rule}");
    assert!(
        root_rule.contains("font-size:24;"),
        "numeric themeVariables.fontSize should be emitted like Mermaid's raw CSS value: {root_rule}"
    );
    assert!(
        !root_rule.contains("font-size:24px;"),
        "numeric themeVariables.fontSize must not be rewritten as a px string: {root_rule}"
    );
}

#[test]
fn class_svg_px_string_theme_font_size_drives_svg_label_wrapping_without_losing_text() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"theme": "base", "fontSize": 10, "themeVariables": {"fontSize": "24px"}, "htmlLabels": false} }%%
classDiagram
  class Foo {
    +veryLongMemberNameToWrapTheLayoutProbe: String
    +anotherVeryLongMemberNameToWrapTheLayoutProbe: String
    +thirdVeryLongMemberNameToWrapTheLayoutProbe: String
  }
"##,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .map(|node| {
            let rows = node
                .descendants()
                .filter(|descendant| {
                    descendant.has_tag_name("tspan")
                        && descendant.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "text-outer-tspan")
                        })
                })
                .map(|row| {
                    row.descendants()
                        .filter_map(|descendant| descendant.text().filter(|_| descendant.is_text()))
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            (rows.join(" "), rows.len())
        })
        .collect::<Vec<_>>();

    for expected in [
        "+veryLongMemberNameToWrapTheLayoutProbe: String",
        "+anotherVeryLongMemberNameToWrapTheLayoutProbe: String",
        "+thirdVeryLongMemberNameToWrapTheLayoutProbe: String",
    ] {
        let expected_compact = expected.split_whitespace().collect::<String>();
        let (_, rows) = labels
            .iter()
            .find(|(text, _)| text.split_whitespace().collect::<String>() == expected_compact)
            .unwrap_or_else(|| panic!("missing complete wrapped member {expected:?}: {labels:?}"));
        assert!(
            *rows >= 2,
            "24px theme text should wrap the long member without depending on a font-specific row boundary: {labels:?}"
        );
    }
}
