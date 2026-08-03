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
fn state_svg_honors_mermaid_11_16_theme_css_options() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
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
        svg.contains(r##"[id$="-dependencyStart"],#merman [id$="-dependencyEnd"]{fill:#303030;stroke:#303030;stroke-width:1;}"##),
        "expected State dependency marker CSS to use Mermaid 11.16 suffix selectors: {svg}"
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
        "classic state SVG should emit 11.16 drop-shadow defs but not gradient defs unless useGradient is set: {svg}"
    );
    assert!(
        !svg.contains(r#"markerUnits="strokeWidth""#),
        "classic state SVG should keep Mermaid's classic barb marker units: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-edge0""#)
            && svg.contains(r#"data-look="classic""#)
            && svg.contains(r#"id="merman-state-Active-1""#),
        "classic state DOM should use Mermaid 11.16 scoped ids and explicit data-look: {svg}"
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
        svg.contains(r#"marker-end="url(#merman_stateDiagram-barbEnd)""#),
        "expected neo state transitions to keep an arrowhead marker: {svg}"
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
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> B
"#,
    );

    assert!(
        svg.contains(r#"class="edgeLabel""#)
            && svg.contains(r#"<g class="label" data-id="edge0" transform="translate(0, 0)"></g>"#),
        "root htmlLabels=false should keep the State empty edge label container: {svg}"
    );
    assert_eq!(
        svg.matches("<foreignObject").count(),
        0,
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for empty State edge label DOM: {svg}"
    );
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
        r#"stateDiagram-v2
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
fn state_svg_composite_self_loop_uses_the_explicitly_updated_cluster_path_for_its_label() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
state Active {
  Idle
}
Inactive --> Idle: ACT
Active --> Active: LOG
"#,
    );

    let points = state_edge_data_points(&svg, "edge1");
    assert_eq!(points.len(), 4, "expected one compact logical self-loop");
    assert!(
        points[0].x > points[1].x && points[3].x < points[2].x,
        "data-points must retain the endpoint-clipped self-loop geometry: {points:?}"
    );

    let (label_x, label_y) = state_edge_label_position(&svg, "edge1");
    let expected_x = (points[1].x + points[2].x) / 2.0;
    let expected_y = (points[1].y + points[2].y) / 2.0;
    assert!(
        (label_x - expected_x).abs() <= 1e-5 && (label_y - expected_y).abs() <= 1e-5,
        "a composite self-loop cluster cut must move the label to the updated path midpoint: label=({label_x}, {label_y}), expected=({expected_x}, {expected_y})"
    );
}

#[test]
fn state_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
S1
click S1 href "javascript:alert(1)"
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
        loose.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected loose mode to preserve State click hrefs exactly like Mermaid's link injection path: {loose}"
    );
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
        svg.contains(r##"fill="#22c55e""##) && svg.contains(r##"stroke="#020617""##),
        "end-state inner rough path should consume innerEndBackground/background: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#fef3c7""##)
            && svg.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "note rough paths should consume noteBkgColor/noteBorderColor: {svg}"
    );
}
