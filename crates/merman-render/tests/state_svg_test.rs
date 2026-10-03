mod common;

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn state_edge_data_points(svg: &str, edge_id: &str) -> Vec<merman_render::model::LayoutPoint> {
    use base64::Engine as _;

    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let encoded = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some(edge_id))
        .and_then(|node| node.attribute("data-points"))
        .unwrap_or_else(|| panic!("missing data-points for State edge {edge_id}"));
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 State edge points");
    serde_json::from_slice(&decoded).expect("JSON State edge points")
}

fn state_edge_label_position(svg: &str, edge_id: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some(edge_id)
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .unwrap_or_else(|| panic!("missing State edge label {edge_id}"));
    let transform = label
        .parent()
        .and_then(|node| node.attribute("transform"))
        .unwrap_or_else(|| panic!("missing outer transform for State edge label {edge_id}"));
    let components = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .and_then(|value| value.split_once(','))
        .unwrap_or_else(|| panic!("invalid State edge label transform: {transform}"));
    (
        components.0.trim().parse().expect("numeric label x"),
        components.1.trim().parse().expect("numeric label y"),
    )
}

fn render_state_svg_from_text(text: &str) -> String {
    render_state_svg_from_text_with_engine(Engine::new(), text)
}

fn render_state_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare State artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State artifact")
        .svg()
        .to_string()
}

fn render_state_svg_with_hand_drawn_seed(seed: u64) -> String {
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "look": "handDrawn",
        "handDrawnSeed": seed,
        "themeVariables": {
            "stateBkg": "#101827",
            "stateBorder": "#38bdf8",
            "mainBkg": "#0f172a",
            "strokeWidth": 4,
            "specialStateColor": "#f97316",
            "innerEndBackground": "#22c55e",
            "background": "#020617",
            "noteBkgColor": "#fef3c7",
            "noteBorderColor": "#92400e"
        }
    }));
    let source = r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : seeded note"#;

    render_state_svg_from_text_with_engine(Engine::new().with_site_config(site_config), source)
}

#[test]
fn state_svg_default_min_node_width_reaches_leaf_foreign_object() {
    let svg = render_state_svg_from_text("stateDiagram-v2\nA\n");
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    let width = document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .find(|node| node.descendants().any(|child| child.text() == Some("A")))
        .and_then(|node| node.attribute("width"))
        .expect("leaf label foreignObject")
        .parse::<f64>()
        .expect("numeric leaf label width");
    assert_eq!(
        width, 120.0,
        "default state.minNodeWidth must size the label box"
    );
}

#[test]
fn state_svg_explicit_zero_min_node_width_keeps_natural_leaf_width() {
    let svg = render_state_svg_from_text(
        "%%{init: {\"state\": {\"minNodeWidth\": 0}}}%%\nstateDiagram-v2\nA\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    let width = document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .find(|node| node.descendants().any(|child| child.text() == Some("A")))
        .and_then(|node| node.attribute("width"))
        .expect("leaf label foreignObject")
        .parse::<f64>()
        .expect("numeric leaf label width");
    assert!(
        width > 0.0 && width < 120.0,
        "explicit zero must disable the default minimum: {width}"
    );
}

#[test]
fn state_svg_cross_composite_default_min_width_reaches_all_leaf_labels() {
    let source =
        include_str!("../../../fixtures/state/stress_state_cross_composite_transitions_007.mmd");
    let svg = render_state_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).expect("State SVG");

    for label in ["InnerA", "Deep", "After"] {
        let width = document
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .find(|node| node.descendants().any(|child| child.text() == Some(label)))
            .and_then(|node| node.attribute("width"))
            .expect("leaf label foreignObject")
            .parse::<f64>()
            .expect("numeric leaf label width");
        assert_eq!(width, 120.0, "default state.minNodeWidth must size {label}");
    }
}

#[test]
fn state_svg_uses_label_presence_and_source_owned_end_state_paints() {
    for look in ["classic", "neo"] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "look": look,
            "layout": "elk",
            "themeVariables": {
                "mainBkg": "#112233",
                "lineColor": "#223344",
                "stateBorder": "#334455",
                "specialStateColor": "#445566",
                "innerEndBackground": "#556677",
                "background": "#667788"
            }
        })));
        let svg = render_state_svg_from_text_with_engine(
            engine,
            "stateDiagram-v2\n[*] --> Ready\nReady --> Done: finish\nDone --> [*]\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
            .collect();
        assert_eq!(
            labels.len(),
            if cfg!(feature = "layout-elk") { 1 } else { 3 },
            "{look}: label groups follow the selected provider's insertion contract"
        );
        let paths = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "edgePaths")
                    })
            })
            .expect("edge path group");
        if cfg!(feature = "layout-elk") {
            assert!(
                paths
                    .attribute("class")
                    .unwrap()
                    .split_whitespace()
                    .any(|class| class == "edges")
            );
        }
        for edge in paths.children().filter(|node| node.has_tag_name("path")) {
            let marker = edge.attribute("marker-end").expect("transition marker");
            assert_eq!(
                marker.ends_with("-margin)"),
                look == "neo",
                "{look}: {marker}"
            );
        }
        let end_outer = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("outer-path")
                    && node.children().any(|child| child.has_tag_name("g"))
            })
            .expect("end-state double circle");
        let outer_paints: Vec<_> = end_outer
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(outer_paints[0].attribute("fill"), Some("#112233"));
        assert_eq!(outer_paints[1].attribute("stroke"), Some("#223344"));
        let inner = end_outer
            .children()
            .find(|node| node.has_tag_name("g"))
            .unwrap();
        let inner_paints: Vec<_> = inner
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(inner_paints[0].attribute("fill"), Some("#334455"));
        assert_eq!(inner_paints[1].attribute("stroke"), Some("#334455"));
    }
}

#[test]
fn state_svg_classic_look_honors_theme_css_options() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "classic",
        "themeVariables": {
            "transitionColor": "#202020",
            "lineColor": "#303030",
            "nodeBorder": "#404040",
            "stateLabelColor": "#505050",
            "mainBkg": "#606060",
            "background": "#707070",
            "altBackground": "#808080",
            "strokeWidth": 4,
            "noteBorderColor": "#909090",
            "noteBkgColor": "#a0a0a0",
            "noteTextColor": "#b0b0b0",
            "labelBackgroundColor": "#c0c0c0",
            "edgeLabelBackground": "#d0d0d0",
            "transitionLabelColor": "#e0e0e0",
            "specialStateColor": "#f0f0f0",
            "innerEndBackground": "#010101",
            "compositeBackground": "#020202",
            "stateBkg": "#030303",
            "stateBorder": "#040404",
            "compositeTitleBackground": "#050505"
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Active: start
Active --> [*]: done"#,
    );

    assert!(
        svg.contains(r#".marker{fill:#303030;stroke:#303030;}"#),
        "expected State base marker CSS to follow lineColor: {svg}"
    );
    assert!(
        svg.contains(r#"defs [id$="-barbEnd"]{fill:#202020;stroke:#202020;}"#),
        "expected State barbEnd marker CSS to follow transitionColor and the prefixed marker id: {svg}"
    );
    assert!(
        svg.contains(r##"#merman [id$="-dependencyStart"],#merman [id$="-dependencyEnd"]{fill:#303030;stroke:#303030;stroke-width:4;}"##),
        "expected State dependency marker CSS to use Mermaid suffix selectors: {svg}"
    );
    assert!(
        svg.contains(r#".transition{stroke:#202020;stroke-width:4;fill:none;}"#),
        "expected State transition CSS to follow transitionColor/strokeWidth: {svg}"
    );
    assert!(
        svg.contains(r#".edgeLabel .label text{fill:#e0e0e0;}"#),
        "expected State edge label CSS to follow transitionLabelColor: {svg}"
    );
    assert!(
        svg.contains(r#".node circle.state-start{fill:#f0f0f0;stroke:#f0f0f0;}"#),
        "expected State start/fork CSS to follow specialStateColor: {svg}"
    );
    assert!(
        svg.contains(r#".node rect{fill:#030303;stroke:#040404;stroke-width:4px;}"#),
        "expected State node CSS to follow stateBkg/stateBorder/strokeWidth: {svg}"
    );
    assert!(
        !svg.contains(r#"id="merman-gradient""#) && svg.contains(r#"id="merman-drop-shadow""#),
        "classic state SVG should emit classic drop-shadow defs but not gradient defs unless useGradient is set: {svg}"
    );
    assert!(
        !svg.contains(r#"markerUnits="strokeWidth""#),
        "classic state SVG should keep Mermaid's classic barb marker units: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-edge0""#)
            && svg.contains(r#"data-look="classic""#)
            && svg.contains(r#"id="merman-state-Active-1""#),
        "classic state DOM should use Mermaid scoped ids and explicit data-look: {svg}"
    );
}

#[test]
fn state_svg_neo_look_emits_neo_marker_and_cluster_theme_resources() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo",
        "themeVariables": {
            "transitionColor": "#202020",
            "mainBkg": "#606060",
            "stateBorder": "#040404",
            "strokeWidth": 4,
            "useGradient": true,
            "gradientStart": "#112233",
            "gradientStop": "#445566",
            "dropShadow": "url(#drop-shadow)",
            "radius": 3
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Active: start
state Active {
  Idle --> Busy
}"#,
    );

    assert!(
        svg.contains(r#"<defs><linearGradient id="merman-gradient""#),
        "expected neo state SVG to emit the shared gradient resource: {svg}"
    );
    assert!(
        svg.contains(r#"<filter id="merman-drop-shadow""#),
        "expected neo state SVG to emit the shared drop-shadow resource: {svg}"
    );
    assert!(
        svg.contains(r#"markerUnits="strokeWidth""#)
            && svg.contains(r#"d="M 19,7 L11,14 L13,7 L11,0 Z""#),
        "expected neo state SVG to use Mermaid's neo barb marker geometry: {svg}"
    );
    assert!(
        svg.contains(r#"marker-end="url(#merman_stateDiagram-barbEnd-margin)""#),
        "expected neo state transitions to use the margin arrowhead marker"
    );
    assert!(
        svg.contains(
            r##"[data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:url(#merman-gradient);stroke-width:4;}"##
        ),
        "expected neo state cluster CSS to reference the scoped gradient: {svg}"
    );
    assert!(
        svg.contains(
            r##"[data-look="neo"].statediagram-cluster rect.outer{rx:3px;ry:3px;filter:url(#merman-drop-shadow);}"##
        ),
        "expected neo state cluster outer rect CSS to reference the scoped drop-shadow and radius: {svg}"
    );
}

#[test]
fn state_svg_hand_drawn_seed_controls_visible_rough_paths() {
    let seed_7 = render_state_svg_with_hand_drawn_seed(7);
    let seed_7_again = render_state_svg_with_hand_drawn_seed(7);
    let seed_8 = render_state_svg_with_hand_drawn_seed(8);

    assert_eq!(
        seed_7, seed_7_again,
        "same handDrawnSeed should keep State rough SVG deterministic"
    );
    assert_ne!(
        seed_7, seed_8,
        "different handDrawnSeed should change visible State rough paths"
    );
    assert!(
        seed_7.contains(r##"fill="#101827""##)
            && seed_7.contains(r##"stroke="#38bdf8" stroke-width="4""##),
        "seed test should exercise ordinary visible rough paths: {seed_7}"
    );
    assert!(
        seed_7.contains(r##"fill="#fef3c7""##)
            && seed_7.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "seed test should exercise note rough paths as a second visible consumer: {seed_7}"
    );
}

#[test]
fn state_svg_root_html_labels_override_deprecated_flowchart_label_dom() {
    let root_false = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );
    let root_true = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );

    assert!(
        root_false.contains(r#"<text y="-10.1""#)
            && root_false.contains(r#"class="text-outer-tspan row""#)
            && root_false.contains(r#"class="text-inner-tspan""#),
        "root htmlLabels=false should render State labels as SVG text: {root_false}"
    );
    assert!(
        !root_false.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for simple State label DOM: {root_false}"
    );
    assert!(
        root_true.contains("<foreignObject")
            && root_true.contains(r#"class="nodeLabel markdown-node-label""#)
            && root_true.contains(r#"class="edgeLabel""#),
        "root htmlLabels=true should override deprecated flowchart.htmlLabels=false and keep HTML label DOM: {root_true}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_cluster_titles() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
state Parent {
  A
}
"#,
    );

    assert!(
        svg.contains(r#"class="cluster-label""#)
            && svg.contains(r#"<text y="-10.1""#)
            && svg.contains(r#"class="text-outer-tspan row""#),
        "root htmlLabels=false should render State cluster titles as SVG text: {svg}"
    );
    assert!(
        !svg.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for simple State cluster DOM: {svg}"
    );
}

#[test]
fn state_composite_paint_uses_large_and_multiline_title_measurements() {
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html_labels in [true, false] {
            for title in ["Large title", "First<br/>Second"] {
                let engine =
                    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                        "layout": layout,
                        "look": "classic",
                        "htmlLabels": html_labels,
                        "themeVariables": { "fontSize": "32px" }
                    })));
                let source = format!("stateDiagram-v2\nstate \"{title}\" as Parent {{\n A\n}}\n");
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                    .expect("parse composite")
                    .expect("State diagram");
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
                    .expect("prepare composite");
                let projection = artifact.layout_json().expect("State layout projection");
                let clusters = projection["layout"]["StateDiagramV2"]["clusters"]
                    .as_array()
                    .expect("State clusters");
                let cluster = clusters
                    .iter()
                    .find(|cluster| cluster["id"] == "Parent")
                    .expect("Parent cluster");
                let title_height = cluster["title_label"]["height"].as_f64().unwrap();
                let rendered = artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .expect("render composite");
                let document = roxmltree::Document::parse(rendered.svg()).expect("State SVG");
                let group = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("data-id") == Some("Parent")
                    })
                    .expect("Parent SVG group");
                let rect = |class| {
                    group
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("rect") && node.attribute("class") == Some(class)
                        })
                        .expect("composite rectangle")
                };
                let number = |node: roxmltree::Node<'_, '_>, name| {
                    node.attribute(name).unwrap().parse::<f64>().unwrap()
                };
                let outer = rect("outer");
                let inner = rect("inner");
                // clusters.js reserves the full painted bbox plus the fixed border gap.
                assert!(
                    (number(inner, "y") - number(outer, "y") - title_height - 2.0).abs() < 0.001,
                    "{layout}, html={html_labels}, title={title}"
                );
                assert!(
                    (number(outer, "height") - number(inner, "height") - title_height - 6.0).abs()
                        < 0.001,
                    "{layout}, html={html_labels}, title={title}"
                );
                let label = group
                    .children()
                    .find(|node| node.attribute("class") == Some("cluster-label"))
                    .expect("composite title");
                if html_labels {
                    let foreign = label
                        .descendants()
                        .find(|node| node.has_tag_name("foreignObject"))
                        .expect("HTML title");
                    assert_eq!(number(foreign, "height"), title_height);
                    assert_eq!(
                        title_height,
                        if title.contains("<br/>") { 96.0 } else { 48.0 }
                    );
                    assert_eq!(label.descendants().filter(|node| node.has_tag_name(("http://www.w3.org/1999/xhtml", "br"))).count(),
                        usize::from(title.contains("<br/>")));
                } else {
                    assert!(
                        title_height > 24.0,
                        "32px title must exceed the old fixed height"
                    );
                    assert_eq!(
                        label
                            .descendants()
                            .filter(|node| node.attribute("class") == Some("text-outer-tspan row"))
                            .count(),
                        if title.contains("<br/>") { 2 } else { 1 }
                    );
                }
            }
        }
    }
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_notes() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A
note right of A : Note text
"#,
    );

    assert!(
        svg.contains("statediagram-note")
            && svg.contains(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">Note text</tspan>"#
            ),
        "root htmlLabels=false should render State notes as SVG text: {svg}"
    );
    assert!(
        !svg.contains(r#"<span class="nodeLabel"><p>Note text</p></span>"#),
        "root htmlLabels=false should not render State note text through HTML node labels: {svg}"
    );
}

#[test]
fn state_svg_serializes_sanitized_note_images_as_valid_xhtml() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
A
note right of A
  <a href='https://mermaid.js.org/' target='_blank'><code>note about mermaid</code></a><br/>
  <img src=x onerror=alert(1)>
end note
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let image = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "img")
        .expect("sanitized note image");
    assert_eq!(image.attribute("src"), Some("x"));
    assert_eq!(
        image.attribute("style"),
        Some("display: flex; flex-direction: column; width: 100%;")
    );
    assert!(image.attribute("onerror").is_none());
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_rect_with_title() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
Display : Ready
Display : Running
"#,
    );

    assert!(
        svg.contains(r#"title-state"#)
            && svg.contains(r#"<text y="-10.1""#)
            && svg.contains(r#"class="text-outer-tspan row""#)
            && svg.contains("Ready")
            && svg.contains("Running"),
        "root htmlLabels=false should render State rectWithTitle labels as SVG text: {svg}"
    );
    assert!(
        !svg.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for State rectWithTitle DOM: {svg}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_empty_edge_labels() {
    for backend in ["elk", "dagre"] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "layout": backend,
            "htmlLabels": false,
            "flowchart": {"htmlLabels": true}
        })));
        let svg = render_state_svg_from_text_with_engine(engine, "stateDiagram-v2\nA --> B\n");
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
            .collect();
        if backend == "elk" && cfg!(feature = "layout-elk") {
            assert!(
                labels.is_empty(),
                "ELK only inserts groups for present labels"
            );
        } else {
            assert_eq!(
                labels.len(),
                1,
                "Dagre inserts an empty wrapper for an unlabeled edge"
            );
            let label = labels[0]
                .children()
                .find(|node| node.is_element())
                .expect("Dagre label group");
            assert_eq!(label.attribute("data-id"), Some("edge0"));
            assert_eq!(label.attribute("transform"), Some("translate(0, 0)"));
            assert!(
                !label.children().any(|node| node.is_element()),
                "empty SVG labels have no HTML child"
            );
        }
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            "{backend}: root htmlLabels=false overrides the deprecated Flowchart setting"
        );
    }
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_self_loop_edge_labels() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> A: again
"#,
    );

    assert!(
        svg.contains(r#"data-id="edge0""#)
            && svg.contains(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">again</tspan>"#
            ),
        "root htmlLabels=false should render State self-loop labels on the original edge id as SVG text: {svg}"
    );
    assert!(
        !svg.contains("cyclic-special"),
        "Mermaid 11.16 keeps cyclic-special helpers out of the public self-loop edge label DOM: {svg}"
    );
    assert_eq!(
        svg.matches("<foreignObject").count(),
        0,
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for State self-loop label DOM: {svg}"
    );
}

#[test]
fn state_svg_leaf_self_loop_keeps_dagre_label_anchor_without_an_explicit_path_update() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
A --> A: again
"#,
    );

    let points = state_edge_data_points(&svg, "edge0");
    let (_, label_y) = state_edge_label_position(&svg, "edge0");
    let path_max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);

    assert!(
        label_y > path_max_y,
        "a leaf self-loop has no cluster cut, so Mermaid keeps its outside Dagre label anchor: {svg}"
    );
}

#[test]
fn state_svg_composite_self_loop_preserves_the_layout_label_offset_after_cluster_clipping() {
    let source = r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
state Active {
  Idle
}
Inactive --> Idle: ACT
Active --> Active: LOG
"#;
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout: merman_render::model::StateDiagramLayout =
        serde_json::from_value(projection["layout"]["StateDiagramV2"].clone()).unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg();

    let points = state_edge_data_points(svg, "edge1");
    assert_eq!(points.len(), 4, "expected one compact logical self-loop");
    assert!(
        points[0].x > points[1].x && points[3].x < points[2].x,
        "data-points must retain the endpoint-clipped self-loop geometry: {points:?}"
    );

    let (label_x, label_y) = state_edge_label_position(svg, "edge1");
    let edge = layout.edges.iter().find(|edge| edge.id == "edge1").unwrap();
    let anchor = edge.label.as_ref().unwrap();
    // This symmetric self-loop keeps the same route midpoint after cluster clipping.
    // Mermaid 12.1 therefore preserves the layout anchor, including its label offset.
    let expected_x = anchor.x;
    let expected_y = anchor.y;
    let midpoint_y = (points[1].y + points[2].y) / 2.0;
    assert!(
        expected_y > midpoint_y,
        "the fixture must retain a nonzero label offset"
    );
    assert!(
        (label_x - expected_x).abs() <= 1e-5 && (label_y - expected_y).abs() <= 1e-5,
        "a symmetric cluster cut must preserve the layout label offset: label=({label_x}, {label_y}), expected=({expected_x}, {expected_y})"
    );
}

#[test]
fn state_svg_direct_composite_self_loop_keeps_unclipped_dagre_endpoints() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
[*] --> Active
state Active {
  [*] --> Ready
  Ready --> Ready : ping
  Ready --> Working : start
  Working --> Working : progress
  Working --> Ready : done
}
Active --> Active : re-enter
Active --> [*] : stop
"#,
    );

    let points = state_edge_data_points(&svg, "edge6");
    assert_eq!(points.len(), 4, "expected one compact composite self-loop");
    assert!(
        (points[0].x - points[1].x).abs() <= 1e-9 && (points[2].x - points[3].x).abs() <= 1e-9,
        "a direct composite endpoint has no node intersect callback, so the Dagre endpoints must remain unclipped: {points:?}"
    );
}

#[test]
fn state_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
	S1
	S2
	S3
	S4
	click S1 href "javascript:alert(1)"
	click S2 href "jav&#x61;script:alert(2)"
	click S3 href ""
	click S4 href "javascript#colon;alert(4)"
"#,
    );
    assert!(
        strict.contains(r#"<a>"#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared State link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe State click href from SVG: {strict}"
    );
    assert!(
        strict.contains(r#"xlink:href="jav&amp;&amp;x61;script:alert(2)""#),
        "expected strict mode to run Mermaid cleanup and preserve only the browser-safe literal entity spelling: {strict}"
    );
    assert!(
        strict.contains(r#"xlink:href="""#),
        "expected strict mode to preserve an empty DOM href like DOMPurify: {strict}"
    );
    assert!(!strict.contains(r#"target="_blank""#), "{strict}");
    assert!(
        !strict.contains("ﬂ°")
            && !strict.contains("¶ß")
            && !strict.contains(r#"xlink:href="javascript&colon;alert(4)""#),
        "expected strict mode to apply Mermaid cleanup before DOMPurify admission: {strict}"
    );

    let loose = render_state_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"stateDiagram-v2
S1
click S1 href "javascript:alert(1)"
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="javascript:alert(1)" target="_blank""#),
        "expected loose mode to preserve State click hrefs exactly like Mermaid's link injection path: {loose}"
    );
}

#[test]
fn state_svg_normalizes_truthy_whitespace_tooltips_after_dompurify() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
click S1 "https://example.test" "   "
"#,
    );

    assert!(svg.contains(r#"title="""#), "{svg}");
    assert!(!svg.contains(r#"title="   ""#), "{svg}");
}

#[test]
fn state_svg_treats_explicit_and_omitted_empty_tooltips_like_mermaid() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
S2
click S1 "https://example.test/one" ""
click S2 href "https://example.test/two"
"#,
    );

    let explicit = svg
        .find(r#"xlink:href="https://example.test/one""#)
        .expect("explicit-tooltip anchor");
    let explicit_tag = &svg[explicit
        ..svg[explicit..]
            .find('>')
            .map_or(svg.len(), |end| explicit + end)];
    assert!(!explicit_tag.contains(" title="), "{explicit_tag}");

    let omitted = svg
        .find(r#"xlink:href="https://example.test/two""#)
        .expect("href-form anchor");
    let omitted_tag = &svg[omitted
        ..svg[omitted..]
            .find('>')
            .map_or(svg.len(), |end| omitted + end)];
    assert!(!omitted_tag.contains(" title="), "{omitted_tag}");
}

#[test]
fn state_svg_strict_repeated_unsafe_clicks_preserve_nested_wrappers() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
S1
click S1 "javascript:alert(1)" "JavaScript"
click S1 "data:text/html,unsafe" "Data"
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert!(inner.has_tag_name("a"), "{svg}");
    assert!(outer.has_tag_name("a"), "{svg}");
    assert_eq!(outer.attribute("title"), Some("JavaScript"));
    assert_eq!(inner.attribute("title"), Some("Data"));
    assert_eq!(node.attribute("title"), Some("Data"));
    for anchor in [outer, inner] {
        assert_eq!(
            anchor.attribute(("http://www.w3.org/1999/xlink", "href")),
            None,
            "{svg}"
        );
        assert_eq!(anchor.attribute("target"), None, "{svg}");
    }
}

#[test]
fn state_svg_loose_repeated_clicks_preserve_each_href_and_target() {
    let svg = render_state_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"stateDiagram-v2
S1
click S1 "https://example.test/first" "First"
click S1 "https://example.test/last" "Last"
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert_eq!(outer.attribute("title"), Some("First"));
    assert_eq!(inner.attribute("title"), Some("Last"));
    assert_eq!(node.attribute("title"), Some("Last"));
    assert_eq!(
        outer.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.test/first")
    );
    assert_eq!(
        inner.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.test/last")
    );
    assert_eq!(outer.attribute("target"), Some("_blank"));
    assert_eq!(inner.attribute("target"), Some("_blank"));
}

#[test]
fn state_svg_empty_later_tooltip_does_not_clear_existing_node_title() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
click S1 "https://example.test/first" "First"
click S1 "https://example.test/last" ""
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert_eq!(outer.attribute("title"), Some("First"));
    assert_eq!(inner.attribute("title"), None);
    assert_eq!(node.attribute("title"), Some("First"));
}

#[test]
fn state_svg_honors_theme_options_on_visible_rough_paths() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "stateBkg": "#101827",
            "stateBorder": "#38bdf8",
            "mainBkg": "#0f172a",
            "strokeWidth": 4,
            "specialStateColor": "#f97316",
            "innerEndBackground": "#22c55e",
            "background": "#020617",
            "compositeBackground": "#111827",
            "noteBkgColor": "#fef3c7",
            "noteBorderColor": "#92400e"
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : themed note"#,
    );

    assert!(
        svg.contains(r##".node rect{fill:#101827;stroke:#38bdf8;stroke-width:4px;}"##),
        "classic ordinary State rects should consume stateBkg/stateBorder/strokeWidth through CSS: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#0f172a""##),
        "choice rough paths should consume mainBkg like Mermaid's State polygon rule: {svg}"
    );
    assert!(
        svg.contains(r##".node circle.state-start{fill:#f97316;stroke:#f97316;}"##),
        "start-state styling should consume specialStateColor: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#38bdf8""##) && svg.contains(r##"stroke="#38bdf8""##),
        "end-state inner rough path should consume stateBorder for fill and stroke: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#fef3c7""##)
            && svg.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "note rough paths should consume noteBkgColor/noteBorderColor: {svg}"
    );
}

#[test]
fn state_svg_plain_rect_radius_uses_effective_theme_for_both_label_modes() {
    // Pinned roundedRect -> drawRect keeps State's preset radius 10 for numeric
    // zero, while the truthy string "0" explicitly draws square corners.
    // Theme Neo supplies radius 3 and Redux supplies 12, independently of look.
    for (look, theme, radius, expected) in [
        ("neo", "neo", None, 3.0),
        ("neo", "redux", None, 12.0),
        ("classic", "redux", None, 12.0),
        ("neo", "default", None, 5.0),
        ("classic", "default", None, 5.0),
        ("default", "default", None, 5.0),
        ("neo", "neo", Some(serde_json::json!(0)), 10.0),
        ("classic", "default", Some(serde_json::json!(0)), 10.0),
        ("neo", "neo", Some(serde_json::json!("0")), 0.0),
        ("classic", "default", Some(serde_json::json!(7.5)), 7.5),
        ("neo", "neo", Some(serde_json::json!(7.5)), 7.5),
    ] {
        for html_labels in [true, false] {
            let mut config = serde_json::json!({
                "look": look,
                "theme": theme,
                "htmlLabels": html_labels
            });
            if let Some(radius) = radius.clone() {
                config["themeVariables"] = serde_json::json!({"radius": radius});
            }
            let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
            let svg = render_state_svg_from_text_with_engine(engine, "stateDiagram-v2\nA\n");
            let document = roxmltree::Document::parse(&svg).expect("State SVG");
            let rect = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class") == Some("basic label-container")
                })
                .expect("ordinary State rectangle");
            for attr in ["rx", "ry"] {
                let actual: f64 = rect.attribute(attr).unwrap().parse().unwrap();
                assert_eq!(
                    actual, expected,
                    "look={look}, theme={theme}, radius={radius:?}, htmlLabels={html_labels}, {attr}"
                );
            }
        }
    }
}

#[test]
fn state_svg_small_terminal_shadow_uses_effective_theme_and_look() {
    // stateStart/stateEnd choose the small filter using nodeShadow, independently
    // of look=neo and dropShadow; a hand-drawn terminal never gets this override.
    for (look, theme, shadow, expected) in [
        ("neo", "neo", None, false),
        ("neo", "redux", None, true),
        ("classic", "redux", None, true),
        ("classic", "default", None, false),
        ("neo", "neo", Some(serde_json::json!(false)), false),
        ("classic", "default", Some(serde_json::json!(true)), true),
        ("handDrawn", "neo", Some(serde_json::json!(true)), false),
        ("neo", "neo", Some(serde_json::json!(0)), false),
        ("neo", "neo", Some(serde_json::json!("false")), true),
    ] {
        let mut config = serde_json::json!({"look": look, "theme": theme});
        if let Some(shadow) = shadow.clone() {
            config["themeVariables"] = serde_json::json!({"nodeShadow": shadow});
        }
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
        let svg = render_state_svg_from_text_with_engine(
            engine,
            "stateDiagram-v2\n[*] --> A\nA --> [*]\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let diagram_id = document.root_element().attribute("id").expect("diagram id");
        let expected_style = format!("filter:url(#{diagram_id}-drop-shadow-small)");
        for class in ["state-start", "outer-path"] {
            let terminal = document
                .descendants()
                .find(|node| node.attribute("class") == Some(class))
                .unwrap_or_else(|| panic!("missing State terminal {class}"));
            assert_eq!(
                terminal.attribute("style"),
                expected.then_some(expected_style.as_str()),
                "look={look}, theme={theme}, nodeShadow={shadow:?}, terminal={class}"
            );
        }
    }
}

#[test]
fn state_svg_min_width_updates_html_box_without_changing_wrapping() {
    for (min_width, label, wrapping_width, display, white_space, width) in [
        (120, "A", 1000, "table", "nowrap", Some("120px")),
        (0, "A", 1000, "table-cell", "nowrap", None),
        (
            120,
            "A long state label beyond the minimum width",
            1000,
            "table-cell",
            "nowrap",
            None,
        ),
        (
            120,
            "A long state label that must wrap onto multiple lines",
            120,
            "table",
            "break-spaces",
            Some("120px"),
        ),
    ] {
        for note in [false, true] {
            let config = serde_json::json!({
                "htmlLabels": true,
                "state": {"minNodeWidth": min_width, "wrappingWidth": wrapping_width}
            });
            let source = if note {
                format!("stateDiagram-v2\nN\nnote right of N : {label}\n")
            } else {
                format!("stateDiagram-v2\nstate \"{label}\" as N\n")
            };
            let svg = render_state_svg_from_text_with_engine(
                Engine::new().with_site_config(MermaidConfig::from_value(config)),
                &source,
            );
            let document = roxmltree::Document::parse(&svg).expect("State SVG");
            let div = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("div")
                        && node.descendants().any(|child| child.text() == Some(label))
                })
                .expect("leaf label div");
            let style: std::collections::BTreeMap<_, _> = div
                .attribute("style")
                .unwrap()
                .split(';')
                .filter_map(|entry| entry.split_once(':'))
                .map(|(key, value)| (key.trim(), value.trim()))
                .collect();
            assert_eq!(
                style.get("display").copied(),
                Some(display),
                "note={note}, min={min_width}, label={label}"
            );
            assert_eq!(style.get("white-space").copied(), Some(white_space));
            assert_eq!(style.get("width").copied(), width);
        }
    }
    let svg = render_state_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false, "state": {"minNodeWidth": 120}
        }))),
        "stateDiagram-v2\n[*] --> A\nA --> [*]\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("state-start"))
            .count(),
        1
    );
}
