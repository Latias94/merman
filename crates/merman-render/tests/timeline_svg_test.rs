mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_timeline_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_timeline_svg_from_text(text: &str) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::TimelineDiagram(layout) = &out.layout else {
        panic!("expected TimelineDiagram layout");
    };

    render_timeline_diagram_svg(
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
fn timeline_svg_honors_mermaid_11_15_disabled_theme_colors() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"themeVariables": {"tertiaryColor": "#123456", "clusterBorder": "#abcdef"}}}%%
timeline
    section Release
        2026 : Ship
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .disabled,#merman .disabled circle,#merman .disabled text{fill:#123456;}"#
        ),
        "expected Timeline disabled node CSS to use themeVariables.tertiaryColor: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .disabled text{fill:#abcdef;}"#),
        "expected Timeline disabled text CSS to use themeVariables.clusterBorder: {svg}"
    );
    assert!(
        !svg.contains(
            r#"#merman .disabled,#merman .disabled circle,#merman .disabled text{fill:lightgray;}"#
        ),
        "Timeline disabled node CSS should not ignore theme variables"
    );
    assert!(
        !svg.contains(r#"#merman .disabled text{fill:#efefef;}"#),
        "Timeline disabled text CSS should not ignore theme variables"
    );
}

#[test]
fn timeline_svg_uses_redux_theme_on_visible_nodes_and_lines() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"theme": "redux", "themeVariables": {"THEME_COLOR_LIMIT": 2, "mainBkg": "#111827", "nodeBorder": "#38bdf8", "strokeWidth": 5, "cScale0": "#ef4444", "cScaleLabel0": "#e879f9", "cScaleInv0": "#334155", "cScale1": "#172554", "cScaleLabel1": "#f8fafc", "cScaleInv1": "#334155"}}}%%
timeline
    section Release
        Plan : Build
        Ship : Done
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .section--1 rect,#merman .section--1 path,#merman .section--1 circle{fill:#111827;stroke:#38bdf8;stroke-width:5;filter:url(#merman-drop-shadow);}"#
        ),
        "expected redux Timeline nodes to consume mainBkg/nodeBorder/strokeWidth on visible path DOM: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .section--1 text{fill:#38bdf8;font-weight:600;}"#),
        "expected redux Timeline labels to consume nodeBorder/fontWeight like Mermaid 11.15: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .lineWrapper line{stroke:#38bdf8;stroke-width:5;}"#),
        "expected redux Timeline lineWrapper CSS to consume nodeBorder/strokeWidth: {svg}"
    );
    assert!(
        svg.contains(r#"stroke-width="2" stroke="black" marker-end="url(#merman-arrowhead)""#),
        "expected current visible line DOM to keep Mermaid's presentational attributes while CSS overrides them: {svg}"
    );
    assert!(
        !svg.contains(r#"class="node-line--1""#),
        "redux Timeline nodes should not emit the classic bottom divider line DOM: {svg}"
    );
    assert!(
        !svg.contains(r#"q0,-5 5,-5"#),
        "redux Timeline node geometry should use sharp-corner paths instead of classic rounded corners: {svg}"
    );
    assert!(
        svg.contains(r#"transform="translate(195, 20)""#),
        "redux Timeline non-event labels should use the Mermaid 11.15 vertical offset: {svg}"
    );
    assert!(
        svg.contains(r#"transform="translate(95, 13)""#),
        "redux Timeline event labels should use the Mermaid 11.15 event vertical offset: {svg}"
    );
}

#[test]
fn timeline_svg_honors_disabled_max_width() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"timeline": {"useMaxWidth": false, "padding": 12}}}%%
timeline
    section Release
        2026 : Ship
"##,
    );
    let root_open = svg.split_once('>').expect("root svg open tag").0;

    assert!(root_open.contains(r#"height=""#), "{root_open}");
    assert!(
        root_open.contains(r#"style="background-color: white;""#),
        "{root_open}"
    );
    assert!(!root_open.contains("max-width"), "{root_open}");
}
