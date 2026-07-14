use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::VennDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::venn::layout_venn_diagram_typed;

fn render_typed_venn(input: &str) -> (VennDiagramLayout, String) {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.diagram_type, "venn");

    let layout = {
        let RenderSemanticModel::Venn(model) = &parsed.model else {
            panic!("expected Venn render model");
        };
        layout_venn_diagram_typed(
            model,
            parsed.meta.title.as_deref(),
            parsed.meta.effective_config.as_value(),
        )
        .expect("layout ok")
    };
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("venn-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render SVG")
        .svg()
        .to_owned();

    (layout, svg)
}

#[test]
fn venn_typed_render_model_outputs_classic_svg_structure() {
    let input = r##"venn-beta
title Product Surface
set A["Core"]:20
set B["Editor"]:14
union A,B["Shared"]:4
"##;

    let (layout, svg) = render_typed_venn(input);
    assert_eq!(layout.areas.len(), 3);
    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"viewBox="0 0 800 450""#));
    assert!(svg.contains(r#"<text class="venn-title""#));
    assert!(svg.contains(">Product Surface</text>"));
    assert!(svg.contains(r#"<g transform="translate(0, 24)">"#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-0""#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-1""#));
    assert!(svg.contains(r#"class="venn-area venn-intersection""#));
    assert!(svg.contains(r#"data-venn-sets="A_B""#));
    assert!(svg.contains(">Core</tspan></text>"));
    assert!(svg.contains(">Shared</tspan></text>"));
}

#[test]
fn venn_styles_and_text_nodes_render_inline_overrides() {
    let input = r##"%%{init: {"venn": {"useDebugLayout": true}, "themeVariables": {"vennSetTextColor": "#222222"}}}%%
venn-beta
set A["Frontend"]:20
  text A1["React"]
set B["Backend"]:16
union A,B["API"]:5
  text AB1["OpenAPI"]
style A fill:#ff6b6b, color:#101010, stroke:#202020, stroke-width:7, fill-opacity:0.42
style A,B fill:#00ffcc, color:#003333
style A1 color:#123456
"##;

    let (_layout, svg) = render_typed_venn(input);

    assert!(svg.contains(r#"style="fill: #ff6b6b; fill-opacity: 0.42; stroke: #202020; stroke-width: 7; stroke-opacity: 0.95;""#));
    assert!(svg.contains(r#"style="font-size: 24px; fill: #101010;""#));
    assert!(svg.contains(r#"style="fill-opacity: 1; fill: #00ffcc;""#));
    assert!(svg.contains(r#"style="font-size: 24px; fill: #003333;""#));
    assert!(svg.contains(r#"<g class="venn-text-nodes">"#));
    assert!(svg.contains(r#"<g class="venn-text-area" font-size="20px">"#));
    assert!(svg.contains(r#"class="venn-text-debug-circle""#));
    assert!(svg.contains(r#"class="venn-text-debug-cell""#));
    assert!(svg.contains(r#"<foreignObject class="venn-text-node-fo""#));
    assert!(svg.contains(r#"<span xmlns="http://www.w3.org/1999/xhtml" class="venn-text-node""#));
    assert!(svg.contains("color: #123456;\">React</span>"));
}

#[test]
fn venn_canonical_typed_path_renders_svg() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r##"%%{init: {"venn": {"useMaxWidth": false, "width": 640, "height": 360}}}%%
venn-beta
set A
set B
union A,B
"##,
            ParseOptions::strict(),
        )
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.diagram_type, "venn");

    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    assert_eq!(
        artifact.family_kind(),
        merman_render::family::RenderFamilyKind::Venn
    );
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned();

    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"viewBox="0 0 640 360""#));
    assert!(!svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"height="360""#));
    assert!(svg.contains(r#"class="venn-area venn-intersection""#));
}

#[test]
fn venn_typed_layout_and_canonical_renderer_agree() {
    let input = r##"venn-beta
set A
set B
union A,B
"##;

    let (layout, svg) = render_typed_venn(input);

    assert_eq!(layout.areas.len(), 3);
    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-0""#));
}
