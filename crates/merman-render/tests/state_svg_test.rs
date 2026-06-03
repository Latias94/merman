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
