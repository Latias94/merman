use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{
    SvgRenderOptions, render_state_diagram_v2_debug_svg, render_state_diagram_v2_svg,
};
use merman_render::{LayoutOptions, layout_parsed};

fn render_state_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::StateDiagramV2(layout) = &out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    render_state_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

fn render_state_svg_with_hand_drawn_seed(seed: u64) -> String {
    let init = serde_json::json!({
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
    });
    let source = format!(
        r#"%%{{init: {init}}}%%
stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : seeded note"#
    );

    render_state_svg_from_text(&source)
}

#[test]
fn state_debug_svg_includes_cluster_positioning_metadata() {
    let text = "stateDiagram-v2\n[*] --> Active\nstate Active {\n  direction TB\n  Idle --> Idle: LOG\n}\n";
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let opts = SvgRenderOptions::default();
    let svg = render_state_diagram_v2_debug_svg(&layout, &opts);

    assert!(svg.contains(r#"id="cluster-Active""#));
    assert!(svg.contains(r#"data-diff="#));
    assert!(svg.contains(r#"data-offset-y="#));
}

#[test]
fn state_svg_honors_mermaid_11_15_theme_css_options() {
    let svg = render_state_svg_from_text(
        r##"%%{init: {"themeVariables": {"transitionColor": "#202020", "lineColor": "#303030", "nodeBorder": "#404040", "stateLabelColor": "#505050", "mainBkg": "#606060", "background": "#707070", "altBackground": "#808080", "strokeWidth": 4, "noteBorderColor": "#909090", "noteBkgColor": "#a0a0a0", "noteTextColor": "#b0b0b0", "labelBackgroundColor": "#c0c0c0", "edgeLabelBackground": "#d0d0d0", "transitionLabelColor": "#e0e0e0", "specialStateColor": "#f0f0f0", "innerEndBackground": "#010101", "compositeBackground": "#020202", "stateBkg": "#030303", "stateBorder": "#040404", "compositeTitleBackground": "#050505"}}}%%
stateDiagram-v2
[*] --> Active: start
Active --> [*]: done"##,
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
        !svg.contains(r#"id="merman-gradient""#) && !svg.contains(r#"id="merman-drop-shadow""#),
        "classic state SVG should not emit neo-only theme resources: {svg}"
    );
    assert!(
        !svg.contains(r#"markerUnits="strokeWidth""#),
        "classic state SVG should keep Mermaid's classic barb marker units: {svg}"
    );
}

#[test]
fn state_svg_neo_look_emits_neo_marker_and_cluster_theme_resources() {
    let svg = render_state_svg_from_text(
        r##"%%{init: {"look": "neo", "themeVariables": {"transitionColor": "#202020", "mainBkg": "#606060", "stateBorder": "#040404", "strokeWidth": 4, "useGradient": true, "gradientStart": "#112233", "gradientStop": "#445566", "dropShadow": "url(#drop-shadow)", "radius": 3}}}%%
stateDiagram-v2
[*] --> Active: start
state Active {
  Idle --> Busy
}"##,
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
            && root_true.contains(r#"class="nodeLabel""#)
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
fn state_svg_honors_theme_options_on_visible_rough_paths() {
    let svg = render_state_svg_from_text(
        r##"%%{init: {"themeVariables": {"stateBkg": "#101827", "stateBorder": "#38bdf8", "mainBkg": "#0f172a", "strokeWidth": 4, "specialStateColor": "#f97316", "innerEndBackground": "#22c55e", "background": "#020617", "compositeBackground": "#111827", "noteBkgColor": "#fef3c7", "noteBorderColor": "#92400e"}}}%%
stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : themed note"##,
    );

    assert!(
        svg.contains(r##"fill="#101827""##),
        "ordinary State rough paths should consume stateBkg, not the default fill: {svg}"
    );
    assert!(
        svg.contains(r##"stroke="#38bdf8" stroke-width="4""##),
        "ordinary State rough paths should consume stateBorder/strokeWidth: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#0f172a""##),
        "choice rough paths should consume mainBkg like Mermaid's State polygon rule: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#f97316""##) && svg.contains(r##"stroke="#f97316""##),
        "fork/join rough paths should consume specialStateColor: {svg}"
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
