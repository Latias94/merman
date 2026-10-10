use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramThemeCompiler, DiagramThemeSpec,
    GradientStop, LinearGradient, RadialGradient, ThemeCapability, ThemeColorValue, ThemeLength,
    ThemePortabilityRequirement,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn color(value: &str) -> ThemeColorValue {
    ThemeColorValue::parse(value).expect("valid test color")
}

fn stops(first: &str, second: &str) -> [GradientStop; 2] {
    [
        GradientStop::new(0.0, color(first)).unwrap(),
        GradientStop::new(1.0, color(second)).unwrap(),
    ]
}

fn render_root_canvas() -> String {
    let base = CanvasPaint::LinearGradient(
        LinearGradient::new(90.0, stops("#0f172a", "#f8fafc"))
            .unwrap()
            .with_repeating_period_px(16.0)
            .unwrap(),
    );
    let glow = CanvasPaint::RadialGradient(
        RadialGradient::new(
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            stops("#22d3ee55", "#22d3ee00"),
        )
        .unwrap()
        .with_tile_px(20.0, 20.0)
        .unwrap(),
    );
    let canvas = CanvasSpec::default()
        .with_base(base)
        .with_layer(CanvasLayer::new(glow).with_blend_mode(BlendMode::Screen))
        .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile repeating root canvas theme");
    for capability in [
        ThemeCapability::GradientPaint,
        ThemeCapability::PatternPaint,
        ThemeCapability::LayeredCanvas,
        ThemeCapability::BlendMode,
    ] {
        assert!(
            theme.report().requires_capability(capability),
            "root canvas theme must require {capability}"
        );
    }

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .expect("parse Flowchart source")
        .expect("detect Flowchart source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("admit portable root canvas theme");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Flowchart")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("root-canvas-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render portable root canvas theme")
        .svg()
        .to_string()
}

#[test]
fn repeating_and_tiled_root_canvas_is_portable_and_deterministic() {
    let first = render_root_canvas();
    let second = render_root_canvas();

    assert_eq!(first, second);
    assert!(
        first.contains(
            r#"<linearGradient id="root-canvas-theme-merman-theme-canvas-base-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="16" y2="0" spreadMethod="repeat""#
        ),
        "{first}"
    );
    let document = roxmltree::Document::parse(&first).expect("valid root canvas SVG");
    let pattern = document
        .descendants()
        .find(|node| {
            node.attribute("id") == Some("root-canvas-theme-merman-theme-canvas-layer-0-pattern")
        })
        .expect("root canvas tile pattern");
    assert!(pattern.has_tag_name("pattern"));
    for (attribute, expected) in [
        ("patternUnits", "userSpaceOnUse"),
        ("x", "0"),
        ("y", "0"),
        ("width", "20"),
        ("height", "20"),
        ("patternTransform", "translate(4 4)"),
    ] {
        assert_eq!(pattern.attribute(attribute), Some(expected), "{attribute}");
    }
    assert!(
        first.contains(r#"fill="url(#root-canvas-theme-merman-theme-canvas-layer-0-pattern)""#),
        "{first}"
    );
    assert!(
        first.contains(r#"style="mix-blend-mode:screen""#),
        "{first}"
    );
}
