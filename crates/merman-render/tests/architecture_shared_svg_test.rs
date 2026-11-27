#![cfg(all(feature = "diagram-architecture", feature = "layout-shoal"))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};

#[test]
fn shoal_architecture_explicit_transparent_canvas_omits_cyberpunk_textures() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: shoal\n---\narchitecture-beta\nservice app(server)[API]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Cyberpunk, None)
        .unwrap();
    for transparent in [false, true] {
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
            .unwrap();
        let session = if transparent {
            session.with_canvas_background(Color::TRANSPARENT)
        } else {
            session
        };
        let rendered = family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for texture in ["pattern", "radialGradient"] {
            assert_eq!(
                document.descendants().any(|node| {
                    node.has_tag_name(texture)
                        && node.ancestors().any(|ancestor| {
                            ancestor.attribute("class") == Some("merman-theme-canvas")
                        })
                }),
                !transparent,
                "transparent={transparent}, texture={texture}"
            );
        }
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("data-id") == Some("app"))
        );
    }
}
