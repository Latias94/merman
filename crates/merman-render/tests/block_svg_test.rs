use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_block_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_block_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::headless_svg_defaults()).expect("layout ok");
    let LayoutDiagram::BlockDiagram(layout) = &out.layout else {
        panic!("expected BlockDiagram layout");
    };

    render_block_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok")
}

#[test]
fn block_svg_scopes_text_and_edge_colors_for_html_labels() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        !svg.contains("<style></style>"),
        "expected block SVG to emit scoped CSS instead of an empty style element"
    );
    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#333;color:#333;}"#),
        "expected block HTML/SVG labels to avoid inheriting host page text color"
    );
    assert!(
        svg.contains(r#"#merman .flowchart-link{stroke:#333333;fill:none;}"#),
        "expected block edges to carry their scoped stroke color"
    );
}

#[test]
fn block_svg_honors_configured_node_text_color() {
    let svg = render_block_svg_from_text(
        r##"%%{init: {"themeVariables": {"nodeTextColor": "#123456"}}}%%
block
  A["Alpha"]
"##,
    );

    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#123456;color:#123456;}"#),
        "expected nodeTextColor theme variable to drive block label color"
    );
}
