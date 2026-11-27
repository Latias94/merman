#![cfg(any(feature = "diagram-pie", feature = "diagram-info"))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};

fn render(source: &str, native: Option<&NativeTheme>) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
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

#[cfg(feature = "diagram-pie")]
const PIE_SOURCE: &str = "pie title Capacity\n  \"Used\" : 40\n  \"Free\" : 60\n";

#[cfg(feature = "diagram-pie")]
#[test]
fn canvas_inserts_base_and_grid_before_family_content() {
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Blueprint, None)
        .unwrap();
    let svg = render(PIE_SOURCE, Some(&NativeTheme::new(theme)));
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
    assert!(!render(PIE_SOURCE, None).contains("merman-theme-canvas"));
}

#[cfg(feature = "diagram-pie")]
#[test]
fn a_host_background_replaces_canvas_paint_and_preserves_transparency() {
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Blueprint, None)
        .unwrap();
    for (color, expected) in [
        (Color::rgb(31, 47, 63), Some("#1F2F3F")),
        (Color::TRANSPARENT, None),
    ] {
        let native = NativeTheme::new(theme.clone()).with_background(color);
        let svg = render(PIE_SOURCE, Some(&native));
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let layer = doc.root_element().first_element_child().unwrap();
        let fill = layer.children().find_map(|node| {
            let fill = node.attribute("fill")?;
            fill.starts_with('#').then_some(fill)
        });
        assert_eq!(fill, expected);
        assert_eq!(
            layer.descendants().any(|node| node.has_tag_name("pattern")),
            expected.is_some(),
        );
    }
}

#[cfg(feature = "diagram-pie")]
#[test]
fn canvas_padding_expands_viewport_and_paint_without_moving_content() {
    let compiler = ThemeCompiler::new();
    let baseline = NativeTheme::new(
        compiler
            .compile_preset(BuiltinPreset::Blueprint, None)
            .unwrap(),
    );
    let padded = NativeTheme::new(
        compiler
            .compile_json(
                br#"{"schema_version":2,"extends":"blueprint","appearances":{"dark":{"canvas":{"padding_px":12}}}}"#,
                None,
            )
            .unwrap(),
    );
    let source = format!("---\nconfig:\n  pie:\n    useMaxWidth: false\n---\n{PIE_SOURCE}");
    let original = render(&source, Some(&baseline));
    let expanded = render(&source, Some(&padded));
    let original = roxmltree::Document::parse(&original).unwrap();
    let expanded = roxmltree::Document::parse(&expanded).unwrap();
    let original_root = original.root_element();
    let expanded_root = expanded.root_element();
    let view_box = |root: roxmltree::Node<'_, '_>| {
        root.attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>()
    };
    let before = view_box(original_root);
    let after = view_box(expanded_root);
    let canvas = expanded_root.first_element_child().unwrap();
    let paint = canvas.first_element_child().unwrap();
    for (index, (name, offset)) in [
        ("x", -12.0),
        ("y", -12.0),
        ("width", 24.0),
        ("height", 24.0),
    ]
    .into_iter()
    .enumerate()
    {
        let coord = paint.attribute(name).unwrap().parse::<f64>().unwrap();
        assert!((after[index] - before[index] - offset).abs() < 0.001);
        assert!((coord - after[index]).abs() < 0.001);
    }
    for name in ["width", "height"] {
        let size =
            |root: roxmltree::Node<'_, '_>| root.attribute(name).unwrap().parse::<f64>().unwrap();
        assert!((size(expanded_root) - size(original_root) - 24.0).abs() < 0.001);
    }
    let slice_path = |document: &roxmltree::Document<'_>| {
        document
            .descendants()
            .find(|node| node.attribute("class") == Some("pieCircle"))
            .unwrap()
            .attribute("d")
            .unwrap()
            .to_owned()
    };
    assert_eq!(slice_path(&original), slice_path(&expanded));
}

#[cfg(feature = "diagram-info")]
#[test]
fn info_measures_and_draws_with_the_native_code_stack() {
    let selected = ThemeCompiler::new()
        .compile_json(
            br##"{
        "schema_version": 2,
        "extends": "studio",
        "overrides": [{
            "families": ["info"],
            "typography": {"mono": ["A&B Mono", "monospace"], "base_size_px": 20},
            "roles": {"text.primary": "#152737"}
        }]
    }"##,
            None,
        )
        .unwrap();
    let svg = render("info", Some(&NativeTheme::new(selected)));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let version = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("version"))
        .unwrap();
    assert_eq!(version.attribute("fill"), Some("#152737"));
    assert_eq!(version.attribute("font-size"), Some("40"));
    assert_eq!(
        version.attribute("font-family"),
        Some("\"A&B Mono\", monospace")
    );
    let parity = render("info", None);
    assert!(parity.contains(r#"class="version" font-size="32""#));
    assert!(!parity.contains("merman-theme-canvas"));
}
