use base64::Engine as _;
use merman_core::{
    Engine, MermaidConfig, OperationControl, ParseOptions,
    baseline::PINNED_MERMAID_BASELINE_VERSION,
};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family::{
    self, RenderedFamilySvg, SvgSerializationBridgeReason, SvgSerializationRoute,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_family_svg(source: &str, diagram_id: &str) -> RenderedFamilySvg {
    render_family_svg_with_engine(Engine::new(), source, diagram_id)
}

fn render_family_svg_with_engine(
    engine: Engine,
    source: &str,
    diagram_id: &str,
) -> RenderedFamilySvg {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::lenient())
        .expect("source parses")
        .expect("source detects a diagram");
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(OperationControl::new())
        .expect("render session starts");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("family layout succeeds")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("canonical SVG succeeds")
}

fn render_svg(source: &str, diagram_id: &str) -> String {
    let output = render_family_svg(source, diagram_id);
    assert_eq!(
        output.serialization_route(),
        SvgSerializationRoute::CanonicalDocument,
        "{diagram_id} must not silently pass parity through the legacy bridge"
    );
    output.svg().to_owned()
}

#[test]
fn unadmitted_svg_families_report_compatibility_routes() {
    for (source, family) in [
        (
            "sequenceDiagram\nA->>B: message",
            family::RenderFamilyKind::Sequence,
        ),
        (
            "railroad-beta\nexpr = terminal(\"a\") ;\n",
            family::RenderFamilyKind::Railroad,
        ),
    ] {
        let output = render_family_svg(source, "unadmitted-route");
        assert_eq!(
            output.serialization_route(),
            SvgSerializationRoute::LegacyBridge
        );
        assert_eq!(
            output.serialization_bridge_reason(),
            Some(&SvgSerializationBridgeReason::LegacyFamily { family })
        );
        assert!(!output.svg().is_empty());
    }
}

#[test]
fn info_canonical_svg_keeps_document_root_and_version_structure() {
    let svg = render_svg("info", "info-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Info SVG is XML");
    let root = document.root_element();

    // The pinned Mermaid Info renderer does not attach a family class to the root; keep the
    // canonical-document serializer aligned with that source-backed DOM contract.
    assert_eq!(root.attribute("class"), None);
    assert_eq!(root.attribute("viewBox"), None);
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 400px; background-color: white;")
    );
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("text") && node.attribute("class") == Some("version"))
    );
    assert!(svg.contains("<style>"));
    assert_eq!(
        root.children()
            .filter(|node| node.is_element())
            .map(|node| node.tag_name().name())
            .collect::<Vec<_>>(),
        ["style", "g", "g"]
    );
    let body = root.last_element_child().unwrap();
    assert_eq!(
        body.children()
            .filter(|node| node.is_element())
            .map(|node| node.tag_name().name())
            .collect::<Vec<_>>(),
        ["text"]
    );
    assert!(!svg.contains(r#"data-merman-resource="info.background""#));
    assert!(!svg.contains("aria-labelledby="));
    assert!(!svg.contains("merman-semantic-info-"));
    assert!(svg.contains(&format!(">v{PINNED_MERMAID_BASELINE_VERSION}</text>")));

    let courier = render_family_svg_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "fontFamily": "courier"
        }))),
        "info",
        "info-courier",
    );
    assert_eq!(
        courier.serialization_route(),
        SvgSerializationRoute::CanonicalDocument
    );
    assert!(courier.svg().contains("font-family:courier;"));
}

#[test]
fn info_nonportable_css_values_use_an_explicit_effect_bridge() {
    let cases = [
        (
            "current-color",
            serde_json::json!({"themeVariables": {"textColor": "currentColor"}}),
            "textColor",
        ),
        (
            "variable-color",
            serde_json::json!({"themeVariables": {"textColor": "var(--host-text)"}}),
            "textColor",
        ),
        (
            "variable-font",
            serde_json::json!({"themeVariables": {"fontFamily": "var(--host-font), sans-serif"}}),
            "fontFamily",
        ),
    ];

    for (name, config, expected_effect) in cases {
        let output = render_family_svg_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
            "info",
            &format!("info-{name}"),
        );
        assert_eq!(
            output.serialization_route(),
            SvgSerializationRoute::LegacyBridge,
            "{name} must use an explicit bridge"
        );
        let Some(SvgSerializationBridgeReason::DrawingListUnavailable { family, reason }) =
            output.serialization_bridge_reason()
        else {
            panic!("expected an effect-specific DrawingList bridge reason for {name}");
        };
        assert_eq!(family, "info");
        assert!(
            reason.contains(expected_effect),
            "{name} bridge reason must name {expected_effect}: {reason}"
        );
    }
}

#[test]
fn error_canonical_svg_keeps_source_backed_icon_and_text_classes() {
    let svg = render_svg("flowchart TD\nA -->\n", "error-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Error SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), None);
    assert_eq!(root.attribute("aria-labelledby"), None);
    assert_eq!(root.attribute("aria-describedby"), None);
    assert_eq!(root.attribute("viewBox"), Some("0 0 2412 512"));
    assert_eq!(root.attribute("style"), Some("max-width: 512px;"));
    let root_children = root
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    assert_eq!(
        root_children
            .iter()
            .map(|node| node.tag_name().name())
            .collect::<Vec<_>>(),
        ["style", "g", "g"]
    );
    assert_eq!(
        root_children[0].tag_name().namespace(),
        Some("http://www.w3.org/2000/svg")
    );
    let body_children = root_children[2]
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    assert_eq!(body_children.len(), 8);
    assert!(body_children[..6].iter().all(|node| {
        node.has_tag_name("path")
            && node.attributes().len() == 2
            && node.attribute("class") == Some("error-icon")
            && node.attribute("d").is_some()
    }));
    assert!(body_children[6..].iter().all(|node| {
        node.has_tag_name("text")
            && node.attributes().len() == 5
            && node.attribute("class") == Some("error-text")
            && node.attribute("style") == Some("text-anchor: middle;")
    }));
    assert_eq!(body_children[6].attribute("x"), Some("1440"));
    assert_eq!(body_children[6].attribute("y"), Some("250"));
    assert_eq!(body_children[6].attribute("font-size"), Some("150px"));
    assert_eq!(body_children[6].text(), Some("Syntax error in text"));
    assert_eq!(body_children[7].attribute("x"), Some("1250"));
    assert_eq!(body_children[7].attribute("y"), Some("400"));
    assert_eq!(body_children[7].attribute("font-size"), Some("100px"));
    let expected_version = format!("mermaid version {PINNED_MERMAID_BASELINE_VERSION}");
    assert_eq!(body_children[7].text(), Some(expected_version.as_str()));
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("error-icon"))
            .count(),
        6
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("error-text"))
            .count(),
        2
    );
    assert!(svg.contains("Syntax error in text"));
    assert!(svg.contains(&format!(
        "mermaid version {PINNED_MERMAID_BASELINE_VERSION}"
    )));
    assert!(!svg.contains("data-merman-"));
    assert!(!svg.contains("<title"));
    assert!(!svg.contains("<desc"));
}

#[test]
fn error_canonical_svg_projects_active_css_from_the_document() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "errorBkgColor": "#AABBCC",
            "errorTextColor": "#DDEEFF",
            "fontFamily": "'IBM Plex Sans', Arial, sans-serif"
        }
    })));
    let output =
        render_family_svg_with_engine(engine, "flowchart TD\nA -->\n", "error-document-style");
    assert_eq!(
        output.serialization_route(),
        SvgSerializationRoute::CanonicalDocument
    );
    let svg = output.svg();
    // These are the only stylesheet declarations that can match Error DOM visual properties;
    // their active font, fill, and stroke tokens must all come from the validated document.
    assert!(svg.contains(r#"#error-document-style{font-family:"IBM Plex Sans",Arial,sans-serif;"#));
    assert!(svg.contains("#error-document-style .error-icon{fill:#aabbcc;}"));
    assert!(svg.contains("#error-document-style .error-text{fill:#ddeeff;stroke:#ddeeff;}"));
    assert!(!svg.contains("#AABBCC"));
    assert!(!svg.contains("#DDEEFF"));

    let quoted_keyword = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"themeVariables": {"fontFamily": "'inherit', sans-serif"}}),
    ));
    let quoted_keyword_output = render_family_svg_with_engine(
        quoted_keyword,
        "flowchart TD\nA -->\n",
        "error-quoted-keyword",
    );
    assert_eq!(
        quoted_keyword_output.serialization_route(),
        SvgSerializationRoute::CanonicalDocument
    );
    assert!(
        quoted_keyword_output
            .svg()
            .contains(r#"#error-quoted-keyword{font-family:"inherit",sans-serif;"#)
    );
}

#[test]
fn error_nonportable_css_effects_use_an_explicit_effect_bridge() {
    let cases = [
        (
            "theme-css",
            serde_json::json!({"themeCSS": ".error-icon { opacity: 0.5; }"}),
            "themeCSS",
        ),
        (
            "current-color",
            serde_json::json!({"themeVariables": {"errorBkgColor": "currentColor"}}),
            "errorBkgColor",
        ),
        (
            "variable-color",
            serde_json::json!({"themeVariables": {"errorTextColor": "var(--error-text)"}}),
            "errorTextColor",
        ),
        (
            "variable-font",
            serde_json::json!({"themeVariables": {"fontFamily": "var(--host-font), sans-serif"}}),
            "fontFamily",
        ),
        (
            "environment-font",
            serde_json::json!({"themeVariables": {"fontFamily": "env(--host-font), sans-serif"}}),
            "fontFamily",
        ),
        (
            "important-font",
            serde_json::json!({"themeVariables": {"fontFamily": "Arial !important"}}),
            "fontFamily",
        ),
        (
            "escaped-font",
            serde_json::json!({"themeVariables": {"fontFamily": "Arial\\ Black"}}),
            "fontFamily",
        ),
        (
            "commented-font",
            serde_json::json!({"themeVariables": {"fontFamily": "Arial/**/, sans-serif"}}),
            "fontFamily",
        ),
        (
            "font-wide-keyword",
            serde_json::json!({"themeVariables": {"fontFamily": "inherit"}}),
            "fontFamily",
        ),
        (
            "font-control-character",
            serde_json::json!({"themeVariables": {"fontFamily": "Arial\nsans-serif"}}),
            "fontFamily",
        ),
    ];

    for (name, config, expected_effect) in cases {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
        let output = render_family_svg_with_engine(
            engine,
            "flowchart TD\nA -->\n",
            &format!("error-{name}"),
        );
        assert_eq!(
            output.serialization_route(),
            SvgSerializationRoute::LegacyBridge,
            "{name} must use an explicit bridge"
        );
        let Some(SvgSerializationBridgeReason::DrawingListUnavailable { family, reason }) =
            output.serialization_bridge_reason()
        else {
            panic!("expected an effect-specific DrawingList bridge reason for {name}");
        };
        assert_eq!(family, "error");
        assert!(
            reason.contains(expected_effect),
            "{name} bridge reason must name {expected_effect}: {reason}"
        );
    }
}

#[test]
fn packet_canonical_svg_keeps_root_profile_and_packet_dom_roles() {
    let svg = render_svg(
        "packet\ntitle Header\n0-7: \"Version\"\n8-15: \"Length\"\n",
        "packet-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Packet SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), None);
    assert_eq!(root.attribute("viewBox"), Some("0 0 1026 94"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 1026px; background-color: white;")
    );
    assert_eq!(
        document
            .descendants()
            .filter(
                |node| node.has_tag_name("rect") && node.attribute("class") == Some("packetBlock")
            )
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetLabel"))
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetByte start"))
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetByte end"))
            .count(),
        2
    );
    assert!(!svg.contains("chart-title-packet-parity"));
    let words = root
        .children()
        .filter(|node| {
            node.has_tag_name("g") && node.children().any(|child| child.has_tag_name("rect"))
        })
        .collect::<Vec<_>>();
    assert_eq!(words.len(), 1);
    assert_eq!(
        words[0]
            .children()
            .filter(|node| node.has_tag_name("rect"))
            .count(),
        2
    );
    for label in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("packetLabel"))
    {
        assert_eq!(label.attribute("dominant-baseline"), Some("middle"));
    }
    for byte in document.descendants().filter(|node| {
        node.attribute("class")
            .is_some_and(|class| class.starts_with("packetByte"))
    }) {
        assert_eq!(byte.attribute("dominant-baseline"), Some("auto"));
    }
}

#[test]
fn packet_canonical_svg_preserves_empty_labels_words_and_authored_accessibility() {
    let svg = render_svg(
        "packet\naccTitle: Header map\naccDescr: Two words\n0: \"\"\n1-31: \"<literal>\"\n32-39: \"Tail\"\n",
        "packet-empty",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let root = document.root_element();
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-packet-empty")
    );
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-packet-empty")
    );
    let children = root
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    assert_eq!(children[0].tag_name().name(), "title");
    assert_eq!(children[0].text(), Some("Header map"));
    assert_eq!(children[1].tag_name().name(), "desc");
    assert_eq!(children[2].tag_name().name(), "style");
    assert_eq!(
        children
            .iter()
            .filter(|node| node.has_tag_name("g"))
            .count(),
        3
    );
    let first_word = children[4];
    let texts = first_word
        .children()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    assert_eq!(texts[0].attribute("class"), Some("packetLabel"));
    assert!(texts[0].text().unwrap_or_default().is_empty());
    assert_eq!(texts[1].attribute("class"), Some("packetByte start"));
    assert_eq!(texts[1].attribute("text-anchor"), Some("middle"));
    assert_eq!(texts[1].text(), Some("0"));
    assert_eq!(texts[2].text(), Some("<literal>"));
    let title = children.last().unwrap();
    assert_eq!(title.attribute("class"), Some("packetTitle"));
    assert!(title.text().unwrap_or_default().is_empty());

    let empty = render_svg("packet\n", "packet-header-only");
    let empty = roxmltree::Document::parse(&empty).unwrap();
    let title = empty
        .descendants()
        .find(|node| node.attribute("class") == Some("packetTitle"))
        .unwrap();
    assert_eq!(title.attribute("x"), Some("513"));
    assert_eq!(title.attribute("y"), Some("-8.5"));

    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "packet": { "showBits": false, "blockFillColor": "none", "blockStrokeColor": "none" }
    })));
    let hidden = render_family_svg_with_engine(engine, "packet\n0: \"\"\n", "packet-no-paint");
    assert_eq!(
        hidden.serialization_route(),
        SvgSerializationRoute::CanonicalDocument
    );
    let hidden = roxmltree::Document::parse(hidden.svg()).unwrap();
    let rect = hidden
        .descendants()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    assert_eq!(rect.attribute("class"), Some("packetBlock"));
    let css = hidden
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".packetBlock{stroke:none;stroke-width:0;fill:none;}"));
    assert_eq!(
        hidden
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .count(),
        2
    );
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn requirement_canonical_svg_keeps_nodes_relationships_and_accessibility() {
    let svg = render_svg(
        r#"requirementDiagram
  direction LR
  accTitle: Requirement parity
  accDescr: A portable requirement graph
  requirement req1 {
    id: REQ-1
    text: Login
    risk: high
    verifymethod: test
  }
  element system {
    type: service
    docref: docs
  }
  system - satisfies -> req1
"#,
        "requirement-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Requirement SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("requirementDiagram"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.starts_with("max-width: "))
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-requirement-parity")
    );
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-requirement-parity")
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("requirement.node.0")
            && node
                .attribute("class")
                .is_some_and(|class| class.split_whitespace().any(|token| token == "node"))
            && node.attribute("data-look") == Some("classic")
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("requirement.edge.0")
            && node
                .attribute("class")
                .is_some_and(|class| class.contains("edgePath"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node
                .attribute("class")
                .is_some_and(|class| class.contains("relationshipLine"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-merman-resource") == Some("requirement.node.0.shape.fill")
            && node.attribute("fill").is_some_and(|fill| fill != "none")
            && node.attribute("stroke") == Some("none")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-merman-resource") == Some("requirement.node.0.shape.stroke")
            && node.attribute("fill") == Some("none")
            && node
                .attribute("stroke")
                .is_some_and(|stroke| stroke != "none")
            && node.attribute("d").is_some_and(|path| path.contains('C'))
    }));
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("divider")
        })
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("text") && node.attribute("class") == Some("reqLabel")
        })
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("text") && node.attribute("class") == Some("edgeLabel")
        })
    );
    assert!(svg.contains(".reqBox{"));
    assert!(svg.contains(".relationshipLine{"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn state_canonical_svg_keeps_typed_shapes_transitions_and_labels() {
    let svg = render_svg(
        r#"stateDiagram-v2
  [*] --> Idle: start
  Idle --> Done: finish
"#,
        "state-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical State SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("statediagram"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.starts_with("max-width: "))
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("circle") && node.attribute("class") == Some("state-start")
    }));
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("rect") && node.attribute("class") == Some("basic") })
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node
                .attribute("class")
                .is_some_and(|class| class.contains("transition"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("path") && node.attribute("class") == Some("marker") })
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("state.edge.0.label")
            && node
                .attribute("class")
                .is_some_and(|class| class.contains("edgeLabel"))
    }));
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("text") && node.attribute("class") == Some("nodeLabel")
        })
    );
    assert!(svg.contains(".transition{"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn block_canonical_svg_keeps_nodes_routes_styles_and_html_labels() {
    let svg = render_svg(
        r#"block
  A["Alpha"] --> B(("Beta"))
  classDef hot fill:#ffe4e6,stroke:#be123c
  class A hot
"#,
        "block-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Block SVG is XML");
    let root = document.root_element();

    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.starts_with("max-width: ")
                && style.ends_with(" background-color: white;"))
    );
    assert!(
        svg.contains("#block-parity .hot&gt;*{fill:#ffe4e6!important;stroke:#be123c!important;}")
    );
    assert!(document.descendants().any(|node| {
        node.attribute("id") == Some("block-parity-A")
            && node.attribute("class") == Some("node hot flowchart-label")
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("id") == Some("block-parity-B")
            && node.attribute("class") == Some("node default flowchart-label")
    }));
    let route = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|part| part == "flowchart-link")
                })
        })
        .expect("canonical Block SVG keeps the edge route");
    assert_eq!(route.attribute("data-edge"), Some("true"));
    assert_eq!(route.attribute("data-et"), Some("edge"));
    let data_id = route.attribute("data-id").expect("edge data id");
    assert!(data_id.starts_with("block-parity-"));
    assert_eq!(
        route.attribute("id"),
        Some(format!("block-parity-{data_id}").as_str())
    );
    let encoded_points = route.attribute("data-points").expect("edge data points");
    let decoded_points = base64::engine::general_purpose::STANDARD
        .decode(encoded_points)
        .expect("edge data points are Base64");
    let points: Vec<serde_json::Value> =
        serde_json::from_slice(decoded_points.as_slice()).expect("edge data points are JSON");
    assert!(!points.is_empty());
    assert!(document.descendants().any(|node| {
        node.has_tag_name("foreignObject")
            && node
                .descendants()
                .any(|child| child.has_tag_name("p") && child.text() == Some("Alpha"))
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn treemap_canonical_svg_preserves_upstream_custom_class_tokens() {
    let svg = render_svg(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/treemap/upstream_cypress_treemap_spec_12_should_apply_classdef_fill_color_to_leaf_nodes_019.mmd"
        )),
        "treemap-class-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Treemap SVG is XML");
    let leaf_classes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|token| token == "treemapLeafGroup")
                })
        })
        .filter_map(|node| node.attribute("class"))
        .collect::<Vec<_>>();

    assert!(leaf_classes.iter().any(|class| {
        class.split_whitespace().any(|token| token == "leaf0")
            && class.split_whitespace().any(|token| token == "redClassx")
            && !class.split_whitespace().any(|token| token == "leaf0x")
    }));
    assert!(leaf_classes.iter().any(|class| {
        class.split_whitespace().any(|token| token == "leaf1")
            && class.split_whitespace().any(|token| token == "blueClassx")
            && !class.split_whitespace().any(|token| token == "leaf1x")
    }));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn radar_canonical_svg_keeps_root_profile_and_family_roles() {
    let svg = render_svg(
        "radar-beta\ntitle Radar parity\naxis A,B,C\ncurve score{1,2,3}\n",
        "radar-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Radar SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("overflow"), Some("visible"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 700 700"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 700px; background-color: white;")
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarTitle"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarAxisLabel"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarCurve-0"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarLegendBox-0"))
    );
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn xychart_canonical_svg_keeps_root_profile_theme_and_group_roles() {
    let svg = render_svg(
        "xychart\n  title Sales\n  x-axis [A, B]\n  y-axis 0 --> 100\n  bar [40, 60]\n  line [30, 70]\n",
        "xychart-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical XYChart SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 700 500"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 700px; background-color: white;")
    );
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|token| token == "main"))
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|token| token == "bar-plot-0"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("xychart.text.0.0") })
    );
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn quadrantchart_canonical_svg_keeps_root_profile_and_dom_roles() {
    let svg = render_svg(
        "quadrantChart\n  accTitle: Quadrant parity\n  title Portfolio\n  x-axis Low --> High\n  y-axis Bottom --> Top\n  quadrant-1 Invest\n  quadrant-2 Explore\n  quadrant-3 Retire\n  quadrant-4 Maintain\n  Feature: [0.7, 0.8]\n",
        "quadrantchart-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical QuadrantChart SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 500 500"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 500px; background-color: white;")
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-quadrantchart-parity")
    );
    for class in [
        "main",
        "quadrants",
        "border",
        "data-points",
        "labels",
        "title",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical QuadrantChart class {class:?}"
        );
    }
    assert!(document.descendants().any(|node| node.has_tag_name("rect")));
    assert!(document.descendants().any(|node| node.has_tag_name("line")));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("circle"))
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("quadrantchart.point.0")
    }));
}

#[test]
fn pie_canonical_svg_keeps_root_profile_and_chart_roles() {
    let svg = render_svg(
        "pie\n  accTitle: Pie parity\n  title Releases\n  \"Stable\" : 3\n  \"Alpha\" : 1\n",
        "pie-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Pie SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 556.2 450"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 556.2px; background-color: white;")
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-pie-parity")
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("circle") && node.attribute("class") == Some("pieOuterCircle")
    }));
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")
        })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.attribute("class") == Some("slice") })
    );
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|token| token == "legend"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("pieTitleText")
    }));
    assert_eq!(document.descendants().filter(|node| node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")).count(), 2);
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn pie_canonical_svg_retains_unpainted_geometry_and_authored_legend_text() {
    let output = render_family_svg_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeVariables": {"pie1": "none", "pieStrokeColor": "none"}
        }))),
        "pie\n  \"  A   B  \" : 1\n",
        "pie-unpainted",
    );
    assert_eq!(
        output.serialization_route(),
        SvgSerializationRoute::CanonicalDocument
    );
    let xml = roxmltree::Document::parse(output.svg()).unwrap();
    let slice = xml
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("class") == Some("pieCircle"))
        .unwrap();
    assert_eq!(slice.attribute("fill"), Some("none"));
    assert!(slice.attribute("style").unwrap().contains("stroke:none;"));
    let legend = xml
        .descendants()
        .find(|node| node.attribute("class") == Some("legend"))
        .unwrap();
    let children: Vec<_> = legend.children().filter(|node| node.is_element()).collect();
    assert_eq!(children.len(), 2);
    assert!(children[0].has_tag_name("rect"));
    assert!(
        children[0]
            .attribute("style")
            .unwrap()
            .contains("fill:none;")
    );
    assert_eq!(children[1].text(), Some("  A   B  "));
    let title = xml
        .descendants()
        .find(|node| node.attribute("class") == Some("pieTitleText"))
        .unwrap();
    assert_eq!(title.text(), None);
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn timeline_canonical_svg_keeps_node_connector_and_axis_roles() {
    let svg = render_svg(
        "timeline\n  accTitle: Timeline parity\n  section Release\n    2026 : Ship\n",
        "timeline-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Timeline SVG is XML");
    let root = document.root_element();

    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-timeline-parity")
    );
    for class in [
        "timeline-node",
        "taskWrapper",
        "eventWrapper",
        "lineWrapper",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Timeline class {class:?}"
        );
    }
    assert!(document.descendants().any(|node| node.has_tag_name("line")));
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-resource")
            .is_some_and(|id| id.ends_with(".arrowhead"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("Ship") })
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn sankey_canonical_svg_keeps_nodes_labels_links_and_gradients() {
    let svg = render_svg("sankey-beta\nA,B,10\n", "sankey-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Sankey SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 600 400"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 600px; background-color: white;")
    );
    let layers = root
        .children()
        .filter_map(|node| node.attribute("class"))
        .collect::<Vec<_>>();
    assert_eq!(layers, ["nodes", "node-labels", "links"]);
    let nodes = root
        .children()
        .find(|node| node.attribute("class") == Some("nodes"))
        .unwrap();
    let groups = nodes
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 2);
    for (index, node) in groups.iter().enumerate() {
        assert_eq!(node.attribute("class"), Some("node"));
        assert_eq!(
            node.attribute("id"),
            Some(format!("sankey-parity-node-{}", index + 1).as_str())
        );
        let rect = node.first_element_child().unwrap();
        assert!(rect.has_tag_name("rect"));
        assert!(rect.attribute("width").is_some() && rect.attribute("fill").is_some());
        assert!(rect.attribute("data-merman-resource").is_none());
    }
    let links = root
        .children()
        .find(|node| node.attribute("class") == Some("links"))
        .unwrap();
    assert_eq!(links.attribute("fill"), Some("none"));
    assert_eq!(links.attribute("stroke-opacity"), Some("0.5"));
    let link = links.first_element_child().unwrap();
    assert_eq!(link.attribute("class"), Some("link"));
    assert_eq!(link.attribute("style"), Some("mix-blend-mode: multiply;"));
    let gradient = link.first_element_child().unwrap();
    assert!(gradient.has_tag_name("linearGradient"));
    let path = gradient.next_sibling_element().unwrap();
    assert!(path.has_tag_name("path"));
    assert_eq!(
        path.attribute("stroke"),
        Some(format!("url(#{})", gradient.attribute("id").unwrap()).as_str())
    );
    assert!(path.attribute("class").is_none() && path.attribute("opacity").is_none());
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.text().is_some_and(|text| text.contains("A"))
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn venn_canonical_svg_keeps_area_roles_labels_and_theme() {
    let svg = render_svg(
        "venn-beta\n title Product Surface\n set A[\"Core\"]:20\n set B[\"Editor\"]:14\n union A,B[\"Shared\"]:4\n",
        "venn-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Venn SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 800 450"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 800px; background-color: white;")
    );
    assert!(svg.contains("#venn-parity .venn-title"));
    for (sets, class) in [
        ("A", "venn-circle"),
        ("B", "venn-circle"),
        ("A_B", "venn-intersection"),
    ] {
        assert!(document.descendants().any(|node| {
            node.has_tag_name("g")
                && node.attribute("data-venn-sets") == Some(sets)
                && node
                    .attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
        }));
    }
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-merman-resource") == Some("venn.area.0.shape")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("venn-title")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.attribute("class") == Some("label")
            && node.text() == Some("Core")
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn railroad_canonical_svg_keeps_rule_roles_connectors_and_theme() {
    let svg = render_svg(
        "railroad-beta\naccTitle: Railroad parity\nexpr = sequence(nonterminal(\"term\"), terminal(\"+\"), special(\"guard\")) ;\n",
        "railroad-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Railroad SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("railroad-diagram"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-railroad-parity")
    );
    assert!(svg.contains("#railroad-parity .railroad-terminal"));
    for class in [
        "railroad-rule",
        "railroad-terminal",
        "railroad-nonterminal",
        "railroad-special",
        "railroad-line",
        "railroad-start",
        "railroad-end",
        "railroad-rule-name",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Railroad class {class:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("railroad.rule.0") })
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-resource")
            .is_some_and(|id| id == "railroad.rule.0.connector.start.path")
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("text") && node.text() == Some("term"))
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn eventmodeling_canonical_svg_keeps_swimlanes_boxes_relations_and_text() {
    let svg = render_svg(
        "eventmodeling\ntf 01 ui Web.ShopCart\ntf 02 cmd Cart.AddItem ->> 01 { sku: \"SKU-1\" }\ntf 03 evt Cart.ItemAdded ->> 02\n",
        "eventmodeling-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical EventModeling SVG is XML");
    let root = document.root_element();

    assert_eq!(
        root.attribute("aria-roledescription"),
        Some("eventmodeling")
    );
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    for class in ["em-swimlane", "em-box", "em-relation", "em-arrowhead"] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical EventModeling class {class:?}"
        );
    }
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("eventmodeling.relation.0")
    }));
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("ShopCart") })
    );
    assert!(!svg.contains("<foreignObject"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn ishikawa_canonical_svg_keeps_fishbone_geometry_and_semantic_labels() {
    let svg = render_svg(
        "ishikawa-beta\n    Blurry Photo\n    Process\n        Out of focus\n        Shutter speed too slow\n    User\n        Shaky hands\n",
        "ishikawa-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Ishikawa SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("ishikawa"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "ishikawa",
        "ishikawa-spine",
        "ishikawa-branch",
        "ishikawa-sub-branch",
        "ishikawa-head-label",
        "ishikawa-label-box",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Ishikawa class {class:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("ishikawa.head") })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("Out of focus") })
    );
    assert!(!svg.contains("<marker") && !svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn cynefin_canonical_svg_keeps_domains_transitions_and_accessibility() {
    let svg = render_svg(
        "cynefin-beta\n  complex\n    \"Observe\"\n  complicated\n    \"Analyze\"\n  complex --> complicated : \"move\"\n",
        "cynefin-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Cynefin SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("cynefin"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "cynefin",
        "cynefinDomain",
        "cynefinBoundary",
        "cynefinCliff",
        "cynefinConfusion",
        "cynefinDomainLabel",
        "cynefinItem",
        "cynefinItemText",
        "cynefinArrowLine",
        "cynefinArrowHead",
        "cynefinArrowLabel",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Cynefin class {class:?}"
        );
    }
    assert!(
        document.descendants().any(|node| {
            node.attribute("data-merman-semantic-id") == Some("cynefin.transition.0")
        })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("Observe") })
    );
    assert!(!svg.contains("<marker") && !svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn tree_view_canonical_svg_keeps_lines_icons_labels_and_semantics() {
    let svg = render_svg(
        "treeView-beta\nsrc/ :::highlight icon(folder) ## source directory\n    main.rs icon(file) ## entry point\n",
        "tree-view-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical TreeView SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("treeView"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "tree-view",
        "treeView-node-line",
        "treeView-node-icon",
        "treeView-node-label",
        "treeView-node-dir",
        "treeView-node-description",
        "treeView-highlight-bg",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical TreeView class {class:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("treeView.node.0") })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("main.rs") })
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn gantt_canonical_svg_keeps_axes_tasks_states_ids_and_semantics() {
    let svg = render_svg(
        "gantt\n  title Release Plan\n  dateFormat YYYY-MM-DD\n  topAxis\n  todayMarker off\n  section Core\n  Build :a1, 2026-01-01, 4d\n  Ship :crit, milestone, 2026-01-05, 1d\n  section Follow-up\n  Docs :done, 2026-01-06, 2d\n",
        "gantt-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("Gantt SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("gantt"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    for class in [
        "grid",
        "tick",
        "section0",
        "section1",
        "task0",
        "done1",
        "critText0",
        "doneText1",
        "milestoneText",
        "sectionTitle",
        "titleText",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Gantt class {class:?}"
        );
    }
    for dom_id in [
        "gantt-parity-a1",
        "gantt-parity-a1-text",
        "gantt-parity-task1",
        "gantt-parity-task1-text",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(dom_id)),
            "expected scoped Gantt DOM id {dom_id:?}"
        );
    }
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node
                .attribute("class")
                .is_some_and(|value| value.split_whitespace().any(|token| token == "task"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.text().is_some_and(|text| text == "Release Plan")
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn journey_canonical_svg_keeps_faces_sections_actors_and_activity_axis() {
    let svg = render_svg(
        "journey\n  title User checkout\n  section Checkout\n    Sign Up: 5: Alice\n    Pay: 3: Bob\n    Review: 1: Alice\n",
        "journey-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Journey SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("journey"));
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-journey-parity")
    );
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert_eq!(root.attribute("preserveAspectRatio"), Some("xMinYMin meet"));
    for class in [
        "legend",
        "journey-section",
        "section-type-0",
        "task",
        "task-type-0",
        "task-line",
        "face",
        "mouth",
        "actor-0",
        "actor-1",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Journey class {class:?}"
        );
    }
    for semantic_id in [
        "journey.document",
        "journey.actor.0",
        "journey.section.0",
        "journey.task.0",
        "journey.task.1",
        "journey.activity",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| { node.attribute("data-merman-semantic-id") == Some(semantic_id) }),
            "expected canonical Journey semantic id {semantic_id:?}"
        );
    }
    for dom_id in [
        "journey-parity-task0",
        "journey-parity-task1",
        "journey-parity-task2",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(dom_id)),
            "expected scoped Journey task line id {dom_id:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("circle"))
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.text().is_some_and(|text| text == "User checkout")
    }));
    assert!(svg.contains(r#"data-merman-resource="journey.activity.arrowhead""#));
    assert!(!svg.contains("<foreignObject"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn kanban_canonical_svg_keeps_sections_cards_ticket_links_and_plain_text() {
    let svg = render_svg(
        r##"%%{init: {"kanban": {"ticketBaseUrl": "https://example.test/tickets/#TICKET#"}}}%%
kanban
  todo[Todo]
    task[Task]@{ ticket: K-1, assigned: "Ada", priority: "High" }
"##,
        "kanban-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Kanban SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("kanban"));
    assert_eq!(root.attribute("viewBox"), Some("90 -310 220 111"));
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert!(document.descendants().any(|node| {
        node.attribute("id") == Some("kanban-parity-todo")
            && node.attribute("data-look") == Some("classic")
            && node
                .attribute("class")
                .is_some_and(|value| value.split_whitespace().any(|token| token == "section-1"))
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("id") == Some("kanban-parity-task")
            && node
                .attribute("class")
                .is_some_and(|value| value.split_whitespace().any(|token| token == "node"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node
                .attribute("class")
                .is_some_and(|value| value.contains("label-container"))
    }));
    assert!(svg.contains(
        r#"<a class="kanban-ticket-link" xlink:href="https://example.test/tickets/K-1""#
    ));
    for text in ["Todo", "Task", "K-1", "Ada"] {
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some(text)),
            "expected canonical Kanban text {text:?}"
        );
    }
    assert!(!svg.contains("<foreignObject"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn kanban_canonical_svg_escapes_logical_ticket_uris_at_the_attribute_boundary() {
    let svg = render_svg(
        r##"%%{init: {"kanban": {"ticketBaseUrl": "https://example.test/tickets/#TICKET#"}}}%%
kanban
  todo[Todo]
    task[Task]@{ ticket: "A&B" }
"##,
        "kanban-uri-escaping",
    );

    assert!(svg.contains(r#"xlink:href="https://example.test/tickets/A&amp;B""#));
    assert!(!svg.contains(r#"xlink:href="https://example.test/tickets/A&B""#));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn gitgraph_canonical_svg_keeps_branch_commit_arrow_and_label_roles() {
    let svg = render_svg(
        r##"gitGraph
  commit id: "A"
  branch dev
  checkout dev
  commit id: "B"
  checkout main
  merge dev
"##,
        "gitgraph-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical GitGraph SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("gitGraph"));
    assert_eq!(root.attribute("aria-roledescription"), Some("gitGraph"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    for class in [
        "branch",
        "branchLabelBkg",
        "branch-label0",
        "commit-bullets",
        "commit-merge",
        "arrow",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical GitGraph class {class:?}"
        );
    }
    for semantic_id in [
        "gitgraph.document",
        "gitgraph.branch.0",
        "gitgraph.arrow.0",
        "gitgraph.commit.0",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("data-merman-semantic-id") == Some(semantic_id)),
            "expected canonical GitGraph semantic id {semantic_id:?}"
        );
    }
    assert!(document.descendants().any(|node| node.has_tag_name("line")));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("circle"))
    );
    assert!(document.descendants().any(|node| node.has_tag_name("rect")));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node
                .descendants()
                .any(|child| child.has_tag_name("tspan") && child.text() == Some("dev"))
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "full-family structure comparison failed; public canonical admission withdrawn"]
fn gitgraph_canonical_svg_scopes_gradient_coordinates_to_each_branch_label() {
    let svg = render_svg(
        r##"%%{init: {"theme": "neo", "themeVariables": {"useGradient": true, "gradientStart": "#112233", "gradientStop": "#445566"}}}%%
gitGraph
  commit id: "A"
  branch dev
  checkout dev
  commit id: "B"
  checkout main
  branch feature
  checkout feature
  commit id: "C"
"##,
        "gitgraph-gradient-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical GitGraph SVG is XML");
    let gradients = document
        .descendants()
        .filter(|node| node.has_tag_name("linearGradient"))
        .collect::<Vec<_>>();
    assert_eq!(
        gradients.len(),
        3,
        "each branch label needs its own gradient: {svg}"
    );
    assert!(gradients.iter().all(|gradient| {
        gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("x1").is_some()
            && gradient.attribute("x2").is_some()
            && gradient.attribute("x1") != gradient.attribute("x2")
    }));

    let label_rects = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|token| token == "branchLabelBkg")
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        label_rects.len(),
        3,
        "expected one gradient-backed label rect per branch"
    );
    let gradient_refs = label_rects
        .iter()
        .filter_map(|rect| rect.attribute("stroke"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        gradient_refs.len(),
        3,
        "branch labels must not share one document-space gradient"
    );
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn er_canonical_svg_keeps_attribute_table_columns_rows_and_relationship_roles() {
    let svg = render_svg(
        r#"erDiagram
  CAR ||--o{ NAMED-DRIVER : allows
  CAR {
    string registrationNumber
    string make
    string model
  }
  NAMED-DRIVER {
    string license
  }
"#,
        "er-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical ER SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("erDiagram"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "root",
        "edgePaths",
        "edgeLabels",
        "nodes",
        "relationshipLine",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical ER class {class:?}"
        );
    }
    for semantic_id in ["er.document", "er.entity.0", "er.edge.0", "er.edge.0.label"] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("data-merman-semantic-id") == Some(semantic_id)),
            "expected canonical ER semantic id {semantic_id:?}"
        );
    }
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("row-rect-odd"))
            .count()
            >= 2
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-resource")
            .is_some_and(|id| id == "er.entity.0.divider.column.0")
    }));
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("data-merman-resource") == Some("er.edge.0.route")
        })
        .expect("canonical ER relationship route");
    assert_eq!(edge.attribute("data-edge"), Some("true"));
    assert_eq!(edge.attribute("data-et"), Some("edge"));
    assert_eq!(edge.attribute("data-look"), Some("classic"));
    assert!(
        edge.attribute("data-id")
            .is_some_and(|id| id.starts_with("id_entity-"))
    );
    assert!(
        edge.attribute("data-points")
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(
        edge.attribute("marker-start"),
        Some("url(#er-parity_er-onlyOneStart)")
    );
    assert_eq!(
        edge.attribute("marker-end"),
        Some("url(#er-parity_er-zeroOrMoreEnd)")
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("marker") && node.attribute("id") == Some("er-parity_er-zeroOrMoreEnd")
    }));
    for text in ["CAR", "string", "registrationNumber", "make", "model"] {
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("span") && node.text() == Some(text)),
            "expected canonical ER table cell text {text:?}"
        );
    }
    assert!(!document.descendants().any(|node| {
        node.has_tag_name("span") && node.text() == Some("string registrationNumber")
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn wardley_canonical_svg_keeps_sections_expanded_markers_and_accessibility() {
    let svg = render_svg(
        r#"wardley-beta
accTitle: Platform map
accDescr: Strategic platform evolution
component API [0.70, 0.65] (buy)
component Database [0.50, 0.45] (inertia)
API +<> Database
evolve API 0.85
annotations [0.10, 0.20]
annotation 1,[0.68, 0.62] "Platform boundary"
"#,
        "wardley-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Wardley SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("wardley"));
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-wardley-parity")
    );
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-wardley-parity")
    );
    for class in [
        "wardley-map",
        "wardley-axes",
        "wardley-links",
        "wardley-trends",
        "wardley-nodes",
        "wardley-annotations",
    ] {
        assert!(document.descendants().any(|node| {
            node.attribute("class")
                .is_some_and(|value| value.split_whitespace().any(|token| token == class))
        }));
    }
    for suffix in [".start", ".end"] {
        assert!(document.descendants().any(|node| {
            node.has_tag_name("marker")
                && node.attribute("id").is_some_and(|id| {
                    id == "link-arrow-start-wardley-parity" && suffix == ".start"
                        || id == "link-arrow-end-wardley-parity" && suffix == ".end"
                })
        }));
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("wardley.node.0") })
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("line")
            && node.attribute("data-merman-resource") == Some("wardley.link.0.line")
            && node.attribute("marker-start") == Some("url(#link-arrow-start-wardley-parity)")
            && node.attribute("marker-end") == Some("url(#link-arrow-end-wardley-parity)")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("line")
            && node.attribute("data-merman-resource") == Some("wardley.trend.0.line")
            && node.attribute("marker-end") == Some("url(#arrow-wardley-parity)")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("dominant-baseline") == Some("middle")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("dominant-baseline") == Some("central")
    }));
    assert!(!document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-merman-resource").is_some_and(|id| {
                id.starts_with("wardley.link.") && (id.ends_with(".start") || id.ends_with(".end"))
            })
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn c4_canonical_svg_keeps_shapes_boundaries_relations_and_accessibility() {
    let svg = render_svg(
        r#"C4Context
accDescr: Internal platform relationships
title Platform map
Enterprise_Boundary(platform, "Platform") {
  Person(user, "User")
  System(api, "API", "Core API", $shape="component")
  System(worker, "Worker")
}
Rel(user, api, "Uses", "HTTPS")
Rel(api, worker, "Publishes", "Events")
"#,
        "c4-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical C4 SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("c4"));
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-c4-parity")
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("g")
            && node.attribute("id") == Some("c4-parity-user")
            && node
                .attribute("class")
                .is_some_and(|class| class.contains("c4-person"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("circle")
            && node.attribute("data-merman-resource") == Some("c4.shape.0.head")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.attribute("class") == Some("c4-name")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains("fill: #ffffff !important;"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("polygon")
            && node.attribute("data-merman-resource") == Some("c4.shape.1.shape")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node.attribute("data-merman-resource") == Some("c4.boundary.1.box")
            && node.attribute("stroke-dasharray") == Some("7,7")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-merman-resource") == Some("c4.relation.1.route")
            && node.attribute("d").is_some_and(|path| path.contains(" Q "))
    }));
    for text in [
        "Platform map",
        "Platform",
        "[ENTERPRISE]",
        "[HTTPS]",
        "[Events]",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("tspan") && node.text() == Some(text)),
            "expected canonical C4 text {text:?}"
        );
    }
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("marker"))
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[cfg(feature = "layout-cytoscape")]
#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn architecture_canonical_svg_keeps_services_groups_routes_and_accessibility() {
    let svg = render_svg(
        r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%
architecture-beta
accTitle: Platform architecture
accDescr: Services and mixed-direction routes
group core(cloud)[Core]
service api(server)[API] in core
service db(database)[Database] in core
service worker(disk)[Worker]
api:R -[sync]-> L:db
db:B -[events]-> R:worker
"#,
        "architecture-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Architecture SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("architecture"));
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-architecture-parity")
    );
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-architecture-parity")
    );
    for class in [
        "architecture-edges",
        "architecture-services",
        "architecture-groups",
    ] {
        assert!(document.descendants().any(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
        }));
    }
    assert!(document.descendants().any(|node| {
        node.has_tag_name("g")
            && node.attribute("id") == Some("architecture-parity-service-api")
            && node.attribute("data-merman-semantic-id") == Some("architecture.node.0")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node.attribute("id") == Some("architecture-parity-group-core")
            && node.attribute("data-merman-resource") == Some("architecture.group.0.outline")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("class") == Some("edge")
            && node.attribute("data-merman-resource") == Some("architecture.edge.0.route")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("polygon")
            && node.attribute("class") == Some("arrow")
            && node
                .attribute("data-merman-resource")
                .is_some_and(|id| id.starts_with("architecture.edge."))
    }));
    let mixed_label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("architecture-service-label")
                && node.text() == Some("events")
        })
        .expect("mixed-direction Architecture label");
    assert!(
        mixed_label
            .attribute("transform")
            .is_some_and(|transform| transform.contains("rotate(-45)")),
        "B-to-R Architecture labels must use Mermaid's -45 degree mixed-axis rotation"
    );
    assert!(svg.contains("#architecture-parity .edge{"));
    assert!(svg.contains("#architecture-parity .arrow{"));
    assert!(svg.contains("#architecture-parity .node-bkg{"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[cfg(feature = "layout-cytoscape")]
#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn mindmap_canonical_svg_keeps_dom_routes_shapes_and_html_labels() {
    let svg = render_svg(
        r#"mindmap
  root((Root))
    child[Child]
"#,
        "mindmap-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Mindmap SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("mindmapDiagram"));
    assert_eq!(root.attribute("aria-roledescription"), Some("mindmap"));
    for class in ["subgraphs", "edgePaths", "edgeLabels", "nodes"] {
        assert!(document.descendants().any(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
        }));
    }
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("id") == Some("mindmap-parity-edge_0_1")
        })
        .expect("canonical Mindmap edge");
    assert_eq!(edge.attribute("data-id"), Some("edge_0_1"));
    assert_eq!(edge.attribute("data-look"), Some("classic"));
    assert_eq!(
        edge.parent().and_then(|parent| parent.attribute("class")),
        Some("edgePaths"),
        "Mindmap edge paths must remain direct children of Mermaid's edgePaths container"
    );
    assert_eq!(
        edge.attribute("data-merman-semantic-id"),
        Some("mindmap.edge.0")
    );
    assert_eq!(
        edge.attribute("data-merman-resource"),
        Some("mindmap.edge.0.route")
    );
    assert!(
        edge.attribute("data-points")
            .is_some_and(|value| !value.is_empty())
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("g")
            && node.attribute("id") == Some("mindmap-parity-node_0")
            && node.attribute("data-look") == Some("classic")
            && node.attribute("data-merman-semantic-id") == Some("mindmap.node.0")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("circle")
            && node.attribute("class") == Some("basic label-container")
            && node.attribute("data-merman-resource") == Some("mindmap.node.0.shape.outer")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node.attribute("class") == Some("basic label-container")
            && node.attribute("data-merman-resource") == Some("mindmap.node.1.shape.outer")
    }));
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .count(),
        3,
        "two node labels and the empty edge-label shell must retain Mermaid's XHTML structure"
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("span")
            && node.attribute("class") == Some("nodeLabel markdown-node-label")
            && node.descendants().any(|child| child.text() == Some("Root"))
    }));
    assert!(svg.contains("mindmap-parity_mindmap-pointEnd-margin"));
    assert!(svg.contains("mindmap-parity-drop-shadow-small"));
    assert!(svg.contains("#mindmap-parity .edge{"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
#[ignore = "canonical SVG migration is not yet admitted by the full upstream DOM gate"]
fn zenuml_canonical_svg_keeps_typed_geometry_and_statement_roles() {
    let svg = render_svg("zenuml\nA->B: hello\n", "zenuml-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical ZenUML SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("zenuml"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.starts_with("max-width: "))
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("g") && node.attribute("class") == Some("participant")
        })
    );
    let message = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("message"))
        .expect("canonical ZenUML message semantic group");
    assert_eq!(
        message.attribute("data-statement"),
        Some("zenuml-statement-0")
    );
    assert!(message.descendants().any(|node| {
        node.has_tag_name("line") && node.attribute("class") == Some("message-line")
    }));
    assert!(message.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("message-label")
    }));
    assert!(svg.contains("data-merman-resource=\"zenuml.message.0.line\""));
    assert!(!svg.contains("zenuml-content"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}
