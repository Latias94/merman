mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::class::layout_class_diagram_typed_with_config;
use merman_render::environment::{RenderEnvironment, RenderSession, TextMeasurementPhase};
use merman_render::family;
use merman_render::model::ClassDiagramLayout;
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

fn class_model(parsed: &ParsedDiagramRender) -> &merman_core::models::class_diagram::ClassDiagram {
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class render model");
    };
    model
}

fn layout_class_with_dagre(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> ClassDiagramLayout {
    let model = class_model(parsed);
    session
        .resource_policy()
        .check_class_complexity(model)
        .expect("class complexity within test limits");
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    layout_class_diagram_typed_with_config(model, &parsed.metadata().effective_config, &measurer)
        .expect("Dagre class layout")
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
        ("classDiagram\nclass Animal\n", "class"),
        ("classDiagram-v2\nclass Animal\n", "classDiagram"),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-wrapper\"}}}%%\nclassDiagram\nclass Animal\n",
            "classDiagram",
        ),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\nclassDiagram\nclass Animal\n",
            "class",
        ),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\nclassDiagram\nclass Animal\nnote for Animal \"classDiagram-v2 is note text\"\n",
            "class",
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
            "Class detection must follow Mermaid's renderer-aware detector contract for {source:?}"
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
        .stack_size(128 * 1024)
        .spawn(move || {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                .expect("parse ok")
                .expect("diagram detected");
            let layout = layout_class_with_dagre(&parsed, &session);
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

    assert!(svg.contains(r#"id="merman-Company" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="classic""#));
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
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#131300;}"#),
        "expected class SVG to make HTML labels self-contained instead of inheriting host page color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#131300;}"#),
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
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7, "themeVariables": {"mainBkg": "#f8fafc", "nodeBorder": "#ef4444", "useGradient": true, "gradientStart": "#112233", "gradientStop": "#445566"}}}%%
classDiagram
  class Class10
"##,
    );

    assert!(
        svg.contains(
            r#"<g class="rough-node default" id="merman-classId-Class10-0" data-look="handDrawn""#
        ),
        "hand-drawn class node should use Mermaid's rough-node wrapper class: {svg}"
    );
    assert!(
        !svg.contains(r#"<g class="node default" id="merman-classId-Class10-0""#),
        "hand-drawn class node should not keep the classic node wrapper class: {svg}"
    );
    assert!(
        svg.contains(r#"<g class="basic label-container outer-path"><path d=""#)
            && svg.contains(
                r##"stroke="#f8fafc" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d=""##
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
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let diagram_role = document
        .root_element()
        .attribute("aria-roledescription")
        .expect("Class diagram role");
    assert_eq!(diagram_role, "class");
    let marker_positions = marker_ids.map(|marker_id| {
        let marker_attr = format!(r#"id="merman_{diagram_role}-{marker_id}""#);
        svg.find(&marker_attr)
            .unwrap_or_else(|| panic!("missing Mermaid 11.16 class marker {marker_id}: {svg}"))
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
            && svg.contains(r##"stroke="#f9f" stroke-width="4" fill="none""##)
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
                r##"stroke="#fff5ad" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d=""##
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

    assert!(
        svg.contains(r#"id="merman-Platform" data-look="classic""#)
            && svg.contains(r#"data-look="classic" xlink:href="https://example.com/service""#)
            && svg.contains(r#"id="merman-classId-Service-0""#)
            && svg.contains(r#"id="merman-classId-Client-"#),
        "Class ELK layout should still render namespaces and class nodes through the Class SVG renderer: {svg}"
    );
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
        svg.contains(r#"#merman .cluster rect{fill:#ffffde;stroke:#aaaa33;stroke-width:1px;}"#),
        "expected class namespace cluster CSS to provide the upstream yellow fill: {svg}"
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
            r#"#merman .node rect,#merman .node circle,#merman .node ellipse,#merman .node polygon,#merman .node path{fill:#112233;stroke:#445566;stroke-width:7}"#
        ),
        "expected numeric strokeWidth to drive Class node shape CSS: {svg}"
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
fn class_svg_namespaces_use_11_15_hierarchical_labels_and_keep_relation_label() {
    let svg = render_class_fixture(
        "upstream_namespaces_and_generics.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(svg.contains(r#"id="merman-Company" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="classic""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected dotted namespace labels to use Mermaid 11.15 path segments"
    );
    assert!(
        svg.contains("<p>manages</p>"),
        "expected relation label text to survive hierarchical namespace rendering"
    );
}

#[test]
fn class_svg_nested_namespace_subgraphs_keep_mermaid_wrapper_structure() {
    let svg = render_class_fixture(
        "stress_class_comments_inside_namespaces_024.mmd",
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
        r#"classDiagram
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
        r#"classDiagram
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

    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.A" data-look="classic""#
    ));
    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.B.B1" data-look="classic""#
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
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-svg".to_string())
        .stack_size(128 * 1024)
        .spawn(move || render_class_svg_from_text(&source))
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
fn class_svg_edge_labels_precede_terminals_in_edge_labels_group() {
    let svg = render_class_fixture(
        "stress_class_parallel_edges_and_cardinality_004.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
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

    assert!(
        svg.contains(
            r#"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24;fill:"#
        ),
        "numeric themeVariables.fontSize should be emitted like Mermaid's raw CSS value"
    );
    assert!(
        !svg.contains(
            r#"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;fill:"#
        ),
        "numeric themeVariables.fontSize must not be rewritten as a px string"
    );
}

#[test]
fn class_svg_px_string_theme_font_size_uses_mermaid_svg_label_wrapping() {
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

    assert_eq!(
        svg.matches("text-outer-tspan").count(),
        10,
        "the title plus three three-line members should emit ten outer tspans: {svg}"
    );
    assert_eq!(
        svg.matches("text-inner-tspan").count(),
        10,
        "each outer tspan should contain one text run after wrapping: {svg}"
    );
    assert!(
        svg.contains(r#">String</tspan></tspan>"#),
        "Mermaid 11.16 should wrap each type suffix onto a standalone third row: {svg}"
    );
}
