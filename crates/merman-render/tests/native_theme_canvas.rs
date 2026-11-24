#![cfg(feature = "diagram-pie")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};

fn render(native: Option<&NativeTheme>) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "pie title Capacity\n  \"Used\" : 40\n  \"Free\" : 60\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let session = native
        .map_or_else(
            || environment.begin_session(),
            |theme| environment.begin_session_with_native_theme(theme.clone()),
        )
        .unwrap();
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("capacity".into()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
    .unwrap()
    .svg()
    .to_owned()
}

#[test]
fn canvas_inserts_base_and_grid_before_family_content() {
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Blueprint, None)
        .unwrap();
    let svg = render(Some(&NativeTheme::new(theme)));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let layer = doc.root_element().first_element_child().unwrap();
    assert_eq!(layer.attribute("class"), Some("merman-theme-canvas"));
    assert_eq!(
        layer.first_element_child().unwrap().attribute("fill"),
        Some("#0E3558")
    );
    assert!(layer.descendants().any(|node| node.has_tag_name("pattern")));
    assert!(layer.descendants().any(|node| {
        node.has_tag_name("rect")
            && node.attribute("fill") == Some("url(#capacity-merman-canvas-texture-0)")
    }));
    assert!(!render(None).contains("merman-theme-canvas"));
}

#[test]
fn a_host_background_replaces_the_canvas_and_keeps_its_texture() {
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Blueprint, None)
        .unwrap();
    for (color, expected) in [
        (Color::rgb(31, 47, 63), Some("#1F2F3F")),
        (Color::TRANSPARENT, None),
    ] {
        let native = NativeTheme::new(theme.clone()).with_background(color);
        let svg = render(Some(&native));
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let layer = doc.root_element().first_element_child().unwrap();
        let fill = layer.children().find_map(|node| {
            let fill = node.attribute("fill")?;
            fill.starts_with('#').then_some(fill)
        });
        assert_eq!(fill, expected);
        assert!(layer.descendants().any(|node| node.has_tag_name("pattern")));
    }
}
