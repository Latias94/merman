#![cfg(feature = "layout-cytoscape")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::LayoutOptions;
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::model::MindmapDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use serde_json::json;

fn render_mindmap_svg_from_text(text: &str, diagram_id: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();

    let layout_options = LayoutOptions::headless_svg_defaults();
    let artifact = family::prepare(&parsed, &layout_options, session).expect("layout ok");

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

#[test]
fn mindmap_root_viewport_consumes_effective_padding_and_use_max_width() {
    for padding in [0.0, 24.5] {
        for use_max_width in [false, true] {
            let source = format!(
                "---\nconfig:\n  layout: tidy-tree\n  look: classic\n  htmlLabels: false\n  mindmap:\n    padding: {padding}\n    useMaxWidth: {use_max_width}\n---\nmindmap\n  root[Root]\n"
            );
            let parsed = MermaidEngine::new()
                .with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(json!({
                        "mindmap": {"padding": 3, "useMaxWidth": !use_max_width}
                    })),
                )
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let config = parsed.metadata().effective_config.as_value();
            assert_eq!(
                config.pointer("/mindmap/padding").and_then(|v| v.as_f64()),
                Some(padding)
            );
            assert_eq!(
                config
                    .pointer("/mindmap/useMaxWidth")
                    .and_then(|v| v.as_bool()),
                Some(use_max_width)
            );

            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let projection = artifact.layout_json().unwrap();
            let layout: MindmapDiagramLayout =
                serde_json::from_value(projection["layout"]["MindmapDiagram"].clone()).unwrap();
            assert_eq!(layout.nodes.len(), 1);
            // Mermaid fixes rectangle node padding independently of the root viewBox margin.
            let node = &layout.nodes[0];
            let expected = [
                node.x - node.width / 2.0 - padding,
                node.y - node.height / 2.0 - padding,
                node.width + 2.0 * padding,
                node.height + 2.0 * padding,
            ];
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let root = document.root_element();
            let view_box = root
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|part| part.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(view_box.len(), 4);
            for (actual, expected) in view_box.iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 0.001,
                    "viewBox={view_box:?}, expected={expected}, padding={padding}"
                );
            }
            if use_max_width {
                assert_eq!(root.attribute("width"), Some("100%"));
                assert_eq!(root.attribute("height"), None);
                let max_width = root
                    .attribute("style")
                    .unwrap()
                    .strip_prefix("max-width: ")
                    .unwrap()
                    .strip_suffix("px;")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                assert!((max_width - expected[2]).abs() < 0.001);
            } else {
                assert_eq!(root.attribute("style"), None);
                let width = root.attribute("width").unwrap().parse::<f64>().unwrap();
                let height = root.attribute("height").unwrap().parse::<f64>().unwrap();
                assert!((width - expected[2]).abs() < 0.001);
                assert!((height - expected[3]).abs() < 0.001);
            }
        }
    }
}

#[test]
fn mindmap_root_viewport_defaults_match_explicit_mermaid_defaults() {
    let source = "---\nconfig:\n  layout: tidy-tree\n  look: classic\n  htmlLabels: false\n---\nmindmap\n  root[Root]\n";
    let explicit = "---\nconfig:\n  layout: tidy-tree\n  look: classic\n  htmlLabels: false\n  mindmap:\n    padding: 10\n    useMaxWidth: true\n---\nmindmap\n  root[Root]\n";
    let default_svg = render_mindmap_svg_from_text(source, "mindmap-root-defaults");
    assert_eq!(
        default_svg,
        render_mindmap_svg_from_text(explicit, "mindmap-root-defaults")
    );
    let document = roxmltree::Document::parse(&default_svg).unwrap();
    let root = document.root_element();
    assert_eq!(root.attribute("width"), Some("100%"));
    assert_eq!(root.attribute("height"), None);
    assert!(root.attribute("style").unwrap().starts_with("max-width: "));
}

#[test]
fn mindmap_html_entity_depth_and_attributes_share_terminal_geometry() {
    // Mermaid 12.2 createText restores references before innerHTML consumes the fragment.
    for (label, visible, reference, title) in [
        ("A#60;B", "A<B", "A&lt;B", None),
        (
            "A#60;b#62;B#60;/b#62;C",
            "A<b>B</b>C",
            "A&lt;b&gt;B&lt;/b&gt;C",
            None,
        ),
        ("A#60;br/#62;B", "A<br/>B", "A&lt;br/&gt;B", None),
        (
            "A#amp;lt;br/#amp;gt;B",
            "A&lt;br/&gt;B",
            "A&amp;lt;br/&amp;gt;B",
            None,
        ),
        (
            "A<span title='x#quot;y'>B</span>C",
            "ABC",
            "ABC",
            Some("x\"y"),
        ),
    ] {
        let source = |label| {
            format!(
                "---\nconfig:\n  layout: tidy-tree\n  htmlLabels: true\n  securityLevel: loose\n---\nmindmap\n  root[\"{label}\"]\n"
            )
        };
        let svg = render_mindmap_svg_from_text(&source(label), "mindmap-html-entities");
        let reference_svg =
            render_mindmap_svg_from_text(&source(reference), "mindmap-html-entities");
        let document = roxmltree::Document::parse(&svg).expect("final XHTML must remain XML");
        let reference_document = roxmltree::Document::parse(&reference_svg).unwrap();
        let terminal = document
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let reference_terminal = reference_document
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let text: String = terminal
            .descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(text, visible, "{label}");
        assert!(
            !terminal
                .descendants()
                .any(|node| { node.has_tag_name("b") || node.has_tag_name("br") })
        );
        assert_eq!(
            terminal
                .descendants()
                .find_map(|node| node.attribute("title")),
            title,
            "{label}"
        );
        for attribute in ["width", "height"] {
            assert_eq!(
                terminal.attribute(attribute),
                reference_terminal.attribute(attribute),
                "{label}: terminal {attribute}"
            );
        }
        assert_eq!(
            document.root_element().attribute("viewBox"),
            reference_document.root_element().attribute("viewBox"),
            "{label}: layout must measure the same literal content as emission"
        );
    }
}

#[test]
fn mindmap_configured_root_font_sizes_html_measurement_and_terminal_geometry() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::DeterministicTextMeasurer;

    let measured_sizes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed_sizes = measured_sizes.clone();
    let measurer = DeterministicTextMeasurer::default().with_width_callback(move |text, style| {
        if text == "MMMM" {
            observed_sizes.lock().unwrap().push(style.font_size);
        }
        text.chars().count() as f64 * style.font_size / 2.0
    });
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.mindmap-configured-root-size").unwrap(),
        "1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity, measurer,
        )))
        .begin_session()
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: tidy-tree\n  look: classic\n  htmlLabels: true\n  themeVariables:\n    fontSize: 32px\n---\nmindmap\n  root[MMMM]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap().into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_json().unwrap();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("mindmap-configured-root-size".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).expect("Mindmap SVG");
    let stylesheet = mindmap_stylesheet(rendered.svg());
    let root_rule = stylesheet
        .strip_prefix("#mindmap-configured-root-size{")
        .and_then(|rule| rule.split_once('}'))
        .expect("Mindmap root typography rule")
        .0;
    assert!(root_rule.contains("font-size:32px;"), "{root_rule}");
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    assert!(
        label
            .descendants()
            .any(|node| node.is_text() && node.text() == Some("MMMM"))
    );
    assert!(
        label.descendants().all(|node| {
            node.attribute("font-size").is_none()
                && !node
                    .attribute("style")
                    .is_some_and(|style| style.contains("font-size"))
        }),
        "the HTML label must inherit root typography"
    );
    let sizes = measured_sizes.lock().unwrap();
    assert!(!sizes.is_empty(), "the HTML label must be measured");
    assert!(
        sizes.iter().all(|size| *size == 32.0),
        "terminal label measurements: {sizes:?}"
    );
    let node = &layout["layout"]["MindmapDiagram"]["nodes"][0];
    assert_eq!(node["width"].as_f64(), Some(104.0));
    assert_eq!(node["height"].as_f64(), Some(68.0));
    assert_eq!(label.attribute("width"), Some("64"));
    assert_eq!(label.attribute("height"), Some("48"));
}

#[test]
fn mindmap_html_recovery_and_policy_removal_share_layout_and_emitted_geometry() {
    let mut layouts = vec!["tidy-tree", "dagre", "swimlane"];
    if cfg!(feature = "layout-elk") {
        layouts.push("elk");
    }
    for layout in layouts {
        for (label, policy) in [
            ("Alpha</div>Beta", ""),
            (
                "Alpha<strong>removed</strong>Beta",
                "  dompurifyConfig:\n    FORBID_TAGS: [strong]\n    KEEP_CONTENT: false\n",
            ),
        ] {
            let source = |label: &str| {
                format!(
                    "---\nconfig:\n  layout: {layout}\n  securityLevel: loose\n{policy}---\nmindmap\n  root[\"{label}\"]\n    child[Child]\n"
                )
            };
            let recovered = render_mindmap_svg_from_text(&source(label), "recovered-mindmap");
            let expected = render_mindmap_svg_from_text(&source("AlphaBeta"), "recovered-mindmap");
            assert_eq!(
                recovered, expected,
                "layout and emission must both consume the final recovered label for {label:?}, {layout}"
            );
            let document = roxmltree::Document::parse(&recovered).expect("valid recovered XHTML");
            assert!(document.descendants().any(|node| {
                node.attribute("class") == Some("nodeLabel markdown-node-label")
                    && node
                        .descendants()
                        .filter_map(|child| child.text().filter(|_| child.is_text()))
                        .collect::<String>()
                        == "AlphaBeta"
            }));
        }
    }
}

fn mindmap_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Mindmap SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect::<String>()
}

fn try_render_mindmap_svg_with_resource_policy(
    text: &str,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Mindmap resource-bound session");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Mindmap resource-bound fixture")
        .expect("detect Mindmap resource-bound fixture")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn mindmap_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = concat!(
        "---\nconfig:\n  layout: dagre\n  look: neo\n",
        "  mindmap:\n    padding: 10.5\n    maxNodeWidth: 80.5\n",
        "  flowchart:\n    wrappingWidth: 160\n---\n",
        "mindmap\n  Root %% literal\n    rect[Wrapped child label with several words]\n",
        "    cloud(Label]\n",
    );
    let diagram_id = "mindmap-bounded";
    let baseline = try_render_mindmap_svg_with_resource_policy(
        source,
        diagram_id,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Mindmap baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Mindmap fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Mindmap SVG byte ceiling");
    let exact = try_render_mindmap_svg_with_resource_policy(source, diagram_id, exact_policy)
        .expect("the exact Mindmap family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Mindmap SVG byte ceiling");
    let error = try_render_mindmap_svg_with_resource_policy(source, diagram_id, below_policy)
        .expect_err("one byte below the Mindmap family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Mindmap MaxSvgBytes rejection, got {error}");
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

fn layout_mindmap_typed(
    parsed: &PresentationDiagram,
    session: RenderSession,
) -> MindmapDiagramLayout {
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Mindmap layout");
    serde_json::from_value(projection["layout"]["MindmapDiagram"].clone())
        .expect("Mindmap layout projection")
}

fn deep_mindmap_chain(depth: usize) -> String {
    let mut input = String::from("---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n");
    for level in 0..depth {
        input.push_str(&" ".repeat(level));
        input.push_str(&format!("n{level}\n"));
    }
    input
}

#[test]
fn mindmap_svg_emits_mermaid_11_15_classic_dom_surface() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  Root
    Child
"#,
        "m15-mindmap",
    );

    assert!(
        svg.contains(r#"id="m15-mindmap-node_0" data-look="classic""#),
        "expected classic Mindmap node DOM id to be diagram-prefixed and expose data-look: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap-edge_0_1""#)
            && svg.contains(r#"data-id="edge_0_1""#)
            && svg.contains(r#"data-look="classic""#),
        "expected Mindmap edge DOM id to be diagram-prefixed while data-id keeps the raw edge id: {svg}"
    );
    assert!(
        svg.contains(r#"<span class="nodeLabel markdown-node-label"><p>Root</p></span>"#),
        "expected Mindmap XHTML labels to keep Mermaid 11.15 class ordering: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap_mindmap-pointEnd-margin""#)
            && svg.contains(r#"id="m15-mindmap_mindmap-pointStart-margin""#),
        "expected Mermaid 11.15 Mindmap margin markers: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap-drop-shadow""#)
            && svg.contains(r#"id="m15-mindmap-drop-shadow-small""#),
        "expected Mermaid 11.15 Mindmap scoped drop-shadow defs: {svg}"
    );
}

#[test]
fn mindmap_break_spaces_keeps_the_trailing_indentation_line_box() {
    // mindmapRenderer.ts clears rect width; labelHelper uses flowchart.wrappingWidth.
    let svg = render_mindmap_svg_from_text(
        "---\nconfig:\n  flowchart:\n    wrappingWidth: 200\n---\nmindmap\n  root[\n    Multi-line root\n    with three lines\n  ]\n",
        "mindmap-break-spaces",
    );

    assert!(
        svg.contains(
            r#"<rect class="basic label-container" style="" x="-120" y="-46" width="240" height="92"/>"#
        ),
        "expected the Mindmap node with a 200px wrap limit to include three 24px line boxes: {svg}"
    );
    assert!(
        svg.contains(r#"<foreignObject width="200" height="72">"#),
        "expected the label foreignObject to preserve the trailing indentation row: {svg}"
    );
    assert!(
        svg.contains(
            "<span class=\"nodeLabel markdown-node-label\">    Multi-line root\n    with three lines\n  </span>"
        ),
        "expected SVG emission to preserve the source whitespace used by break-spaces: {svg}"
    );
}

#[test]
fn mindmap_hex_entity_placeholders_remain_literal_well_formed_xml() {
    for (source, expected) in [
        ("mindmap\n  root[&#x41;]\n", "<p>&amp;&amp;x41;</p>"),
        ("mindmap\n  root[&#X41;]\n", "<p>&amp;&amp;X41;</p>"),
    ] {
        let svg = render_mindmap_svg_from_text(source, "mindmap-hex-entity");

        assert!(svg.contains(expected), "expected {expected:?}: {svg}");
        roxmltree::Document::parse(&svg).expect("Mindmap SVG must be well-formed XML");
    }
}

#[test]
fn mindmap_named_entity_placeholders_are_decoded_before_svg_emission() {
    let svg =
        render_mindmap_svg_from_text("mindmap\n  root[Root #quot;]\n", "mindmap-named-entity");

    assert!(
        svg.contains("<p>Root \"</p>"),
        "expected the Mermaid quote placeholder to become visible text: {svg}"
    );
    assert!(
        !svg.contains('ﬂ') && !svg.contains('¶'),
        "Mermaid entity placeholders must not leak into SVG: {svg}"
    );
    roxmltree::Document::parse(&svg).expect("Mindmap SVG must be well-formed XML");
}

#[test]
fn mindmap_typed_layout_handles_deep_chain() {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    const DEPTH: usize = 1200;
    let source = deep_mindmap_chain(DEPTH);

    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();

    // Exercise the public JSON projection as well as its ordinary destruction. The semantic
    // projection contains the same deeply nested rootNode tree even though the typed model and
    // layout are flat.
    let layout = layout_mindmap_typed(&parsed, session);

    assert_eq!(layout.nodes.len(), DEPTH);
    assert_eq!(layout.edges.len(), DEPTH - 1);
    let expected_last = format!("{}", DEPTH - 1);
    assert_eq!(
        layout.nodes.last().map(|node| node.id.as_str()),
        Some(expected_last.as_str())
    );
}

#[test]
fn mindmap_svg_wraps_section_classes_after_mermaid_palette_cycle() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  root((Many siblings))
    s01[Node 01]
    s02[Node 02]
    s03[Node 03]
    s04[Node 04]
    s05[Node 05]
    s06[Node 06]
    s07[Node 07]
    s08[Node 08]
    s09[Node 09]
    s10[Node 10]
    s11[Node 11]
    s12[Node 12]
"#,
        "m15-mindmap-cycle",
    );

    assert!(
        svg.contains(r#"class="node mindmap-node section-10" id="m15-mindmap-cycle-node_11""#),
        "expected eleventh sibling to use section-10 before the cycle wraps: {svg}"
    );
    assert!(
        svg.contains(r#"class="node mindmap-node section-0" id="m15-mindmap-cycle-node_12""#),
        "expected twelfth sibling to wrap back to section-0 like Mermaid 11.15: {svg}"
    );
    assert!(
        !svg.contains("section-11") && !svg.contains("section-edge-11"),
        "Mindmap section classes should wrap instead of emitting stale section-11 tokens: {svg}"
    );
}

#[test]
fn mindmap_svg_uses_direct_classic_shapes_for_rounded_and_hexagon_nodes() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  root((Root))
    rounded(Rounded)
    hex{{Hexagon}}
"#,
        "m15-mindmap-shapes",
    );

    assert!(
        svg.contains(r#"<rect class="basic label-container" style="" rx="5" ry="5""#),
        "expected classic rounded Mindmap nodes to render as direct rect DOM: {svg}"
    );
    assert!(
        svg.contains(r#"<polygon points=""#) && svg.contains(r#"class="label-container""#),
        "expected classic hexagon Mindmap nodes to render as direct polygon DOM: {svg}"
    );
    assert!(
        !svg.contains(r#"class="basic label-container outer-path""#),
        "classic Mindmap rounded/hexagon nodes should not use the old rough outer-path wrapper: {svg}"
    );
}

#[test]
fn mindmap_tidy_tree_config_dispatches_bidirectional_layout() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  layout: tidy-tree
---
mindmap
  root((Root))
    Left
      Left child
    Right
      Right child
    Also left
"#,
            ParseOptions::strict(),
        )
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();

    let layout = layout_mindmap_typed(&parsed, session);
    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing node {id}"))
    };

    let root = node("0");
    let left = node("1");
    let left_child = node("2");
    let right = node("3");
    let right_child = node("4");
    let also_left = node("5");
    assert!(root.x.is_finite() && root.y.is_finite());
    assert!(left.x < root.x && left_child.x < left.x);
    assert!(right.x > root.x && right_child.x > right.x);
    assert!(also_left.x < root.x);

    assert!(layout.edges.iter().all(|edge| edge.points.len() == 4));
    let edge_to_left = layout
        .edges
        .iter()
        .find(|edge| edge.from == "0" && edge.to == "1")
        .expect("root-to-left edge");
    let edge_to_right = layout
        .edges
        .iter()
        .find(|edge| edge.from == "0" && edge.to == "3")
        .expect("root-to-right edge");
    assert!(edge_to_left.points[1].x < root.x);
    assert!(edge_to_right.points[1].x > root.x);
}

#[test]
fn mindmap_sanitized_html_images_are_well_formed_xhtml() {
    for source in [
        include_str!("../../../fixtures/mindmap/stress_mindmap_html_sanitization_013.mmd"),
        include_str!("../../../fixtures/mindmap/stress_mindmap_html_sanitization_links_034.mmd"),
    ] {
        let svg = render_mindmap_svg_from_text(source, "sanitized-mindmap");
        let doc = roxmltree::Document::parse(&svg).expect("sanitized labels must form valid XML");
        let images: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("img"))
            .collect();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].attribute("src"), Some("x"));
        assert_eq!(
            images[0].tag_name().namespace(),
            Some("http://www.w3.org/1999/xhtml")
        );
        assert!(doc.descendants().all(|n| n.attribute("onerror").is_none()));
        assert!(
            doc.descendants()
                .filter_map(|n| n.attribute("href"))
                .all(|href| !href.starts_with("javascript:"))
        );
        assert!(
            doc.descendants()
                .any(|n| n.attribute("href") == Some("https://mermaid.js.org/"))
        );
    }
}

#[test]
fn mindmap_long_id_css_rejects_before_the_complete_identifier_expansion() {
    let diagram_id = "m".repeat(4096);
    let max = 65_536;
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, max)
        .unwrap();
    let error = try_render_mindmap_svg_with_resource_policy(
        "mindmap\n  Root\n    Child\n",
        &diagram_id,
        policy,
    )
    .expect_err("expanded selectors must obey the output ceiling");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected byte ceiling, got {error}");
    };
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert!(limit.actual > max);
    assert!(
        limit.actual <= max + diagram_id.len(),
        "rejection must occur on one identifier append, before whole CSS expansion: {}",
        limit.actual
    );
}

#[test]
fn mindmap_native_finalization_preserves_theme_v2_codepoints_and_paints() {
    let expected = "\"A&B\", monospace";
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version": 2, "extends": "paper",
                "typography": {"sans": ["A&B", "monospace"], "body_family": "sans"},
                "roles": {"text.primary": "#112233", "surface.emphasis": "#4488FF", "text.on_emphasis": "#110011"},
                "appearances": {"light": {"data": {
                    "ink": ["#90251D", "#226982", "#664226", "#86517C"],
                    "fill": ["#AFD5ED", "#F1D2CD", "#EFE8C2", "#FAD9E0"],
                    "fill_ink": "#112233"
                }}}
            }"##,
            None,
        )
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("mindmap\n  Root\n    Child\n", ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg();
    assert!(svg.contains("A&amp;B"));
    let document = roxmltree::Document::parse(svg).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-mindmap-root") == Some("true")
            && node.attribute("fill") == Some("#110011")
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-mindmap-branch") == Some("0")
            && node.attribute("fill") == Some("#112233")
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("data-edge") == Some("true") && node.attribute("stroke") == Some("#90251D")
    }));
    let finalized = rendered
        .finalize_resvg(&merman_render::svg::SvgPipeline::resvg_safe())
        .expect("native SVG finalization must preserve logical CSS");
    let document =
        roxmltree::Document::parse(finalized.svg().as_str()).expect("valid native Mindmap XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    assert!(
        css.contains(&format!("font-family:{expected};")),
        "native stylesheet must preserve the typed stack: {css}"
    );
    assert!(css.contains("fill:#112233"));
    assert!(css.contains("fill:#4488FF"));
    assert!(css.contains("fill:#AFD5ED"));
    let labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("merman-foreignobject-fallback-text")
        })
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 2);
    let actual_text = labels
        .iter()
        .map(|label| {
            label
                .descendants()
                .filter(roxmltree::Node::is_text)
                .filter_map(|node| node.text())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual_text
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        ["Root", "Child"]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );

    for label in labels {
        let style = label.attribute("style").expect("actual native label style");
        let family_value = style
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
            .find(|(property, _)| property.trim().eq_ignore_ascii_case("font-family"))
            .map(|(_, value)| value.trim())
            .unwrap_or_else(|| panic!("native label omitted font-family: {style}"));
        let mut parser = cssparser::Parser::new(family_value);
        assert_eq!(
            parser.expect_string().unwrap().as_ref(),
            "A&B",
            "native font family: {style}"
        );
        parser.expect_comma().unwrap();
        assert_eq!(
            parser.expect_ident().unwrap().as_ref(),
            "monospace",
            "native fallback family: {style}"
        );
        assert!(
            parser.is_exhausted(),
            "unexpected font-family content: {style}"
        );
    }

    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
}
