#![cfg(feature = "svg")]

#[test]
fn xychart_background_preserves_canvas_and_explicit_config_ownership() {
    check_background_ownership(
        "xychart-beta\nx-axis [A, B]\ny-axis 0 --> 10\nbar [3, 4]",
        "xyChart",
        "background",
        "white",
    );
}

#[test]
fn wardley_background_preserves_canvas_and_explicit_config_ownership() {
    check_background_ownership(
        "wardley-beta\ncomponent App [0.8, 0.2]",
        "wardley",
        "wardley-background",
        "white",
    );
}

fn check_background_ownership(
    source: &str,
    family: &str,
    background_class: &str,
    default_fill: &str,
) {
    use merman::svg::{
        CanvasLayer, CanvasPaint, CanvasSpec, DiagramThemeCompiler, DiagramThemeSpec, ThemePreset,
    };

    let compiler = DiagramThemeCompiler::new();
    let cyberpunk = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let transparent = compiler
        .compile(DiagramThemeSpec::new().with_canvas(CanvasSpec::transparent()))
        .unwrap();
    let layered = compiler
        .compile(
            DiagramThemeSpec::new().with_canvas(
                CanvasSpec::default()
                    .with_layer(CanvasLayer::new(CanvasPaint::solid("#123456").unwrap()))
                    .unwrap(),
            ),
        )
        .unwrap();
    let unspecified = compiler.compile(DiagramThemeSpec::new()).unwrap();
    for (name, theme, config, expected) in [
        ("default", None, serde_json::json!({}), default_fill),
        (
            "unspecified",
            Some(&unspecified),
            serde_json::json!({}),
            default_fill,
        ),
        ("preset", Some(&cyberpunk), serde_json::json!({}), "none"),
        (
            "transparent",
            Some(&transparent),
            serde_json::json!({}),
            "none",
        ),
        ("layered", Some(&layered), serde_json::json!({}), "none"),
        (
            "configured root background",
            Some(&cyberpunk),
            serde_json::json!({"themeVariables": {"background": "#abcdef"}}),
            "#abcdef",
        ),
        (
            "configured chart background",
            Some(&cyberpunk),
            serde_json::json!({"themeVariables": {(family): {"backgroundColor": "#123456"}}}),
            "#123456",
        ),
        (
            "explicit default",
            Some(&cyberpunk),
            serde_json::json!({"themeVariables": {(family): {"backgroundColor": "white"}}}),
            "white",
        ),
        (
            "explicit transparent",
            Some(&cyberpunk),
            serde_json::json!({"themeVariables": {(family): {"backgroundColor": "transparent"}}}),
            "transparent",
        ),
    ] {
        let renderer = merman::Renderer::new().with_engine(
            merman::Engine::new()
                .with_site_config(merman::MermaidConfig::from_value(config.clone())),
        );
        let render = |theme: Option<&merman::svg::DiagramTheme>| {
            let request = merman::RenderRequest::svg(
                source,
                merman::OperationControl::new(),
                merman::SvgRequest::default(),
            );
            let request = match theme {
                Some(theme) => request.with_theme(theme.clone()),
                None => request,
            };
            let output = renderer.render(request).unwrap();
            let merman::RenderOutput::Svg(Some(output)) = output else {
                panic!("{name}: diagram not detected");
            };
            output
        };
        let output = render(theme);
        let document = roxmltree::Document::parse(output.svg()).unwrap();
        let background = document
            .descendants()
            .find(|node| node.attribute("class") == Some(background_class))
            .expect("family keeps its background terminal");
        if name == "configured root background" {
            // Preserve the existing family resolution of an explicit root override.
            let baseline = render(None);
            let baseline = roxmltree::Document::parse(baseline.svg()).unwrap();
            let baseline = baseline
                .descendants()
                .find(|node| node.attribute("class") == Some(background_class))
                .unwrap();
            assert_eq!(
                background.attribute("fill"),
                baseline.attribute("fill"),
                "{name}"
            );
            assert_ne!(background.attribute("fill"), Some("none"), "{name}");
        } else {
            assert_eq!(background.attribute("fill"), Some(expected), "{name}");
        }
    }
}

#[test]
fn public_cyberpunk_canvas_survives_recipe_exchange() {
    use merman::svg::{DiagramThemeCompiler, ThemePreset};

    let compiler = DiagramThemeCompiler::new();
    let direct = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let saved =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&saved).unwrap())
        .unwrap();
    let renderer = merman::Renderer::new();
    for (family, source) in [
        (
            "flowchart",
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd"),
        ),
        (
            "sequence",
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"),
        ),
        (
            "xychart",
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/xychart.mmd"),
        ),
        ("class", "classDiagram\nclass Account {\n +String name\n}"),
    ] {
        let render = |theme: &merman::svg::DiagramTheme| {
            let request = merman::RenderRequest::svg(
                source,
                merman::OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(theme.clone());
            let merman::RenderOutput::Svg(Some(output)) = renderer.render(request).unwrap() else {
                panic!("{family}: diagram not detected");
            };
            output
        };
        let output = render(&direct);
        if family == "class" {
            assert!(
                output
                    .evidence()
                    .theme_diagnostics()
                    .iter()
                    .all(|diagnostic| {
                        diagnostic.code() != "unsupported-effect"
                            && diagnostic.subject() != "effect-binding"
                    }),
                "effects scoped to other families must not create Class residuals"
            );
        }
        assert_eq!(
            output.svg(),
            render(&imported).svg(),
            "{family}: saved recipe"
        );
        let document = roxmltree::Document::parse(output.svg()).unwrap();
        let base = document
            .descendants()
            .find(|node| node.attribute("class") == Some("merman-theme-canvas-base"))
            .expect("painted root canvas");
        assert_eq!(base.attribute("fill"), Some("#051423"), "{family}");
        let layers: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("merman-theme-canvas-layer"))
            .collect();
        assert_eq!(layers.len(), 3, "{family}: radial and both grid directions");
        for layer in layers {
            assert_eq!(layer.attribute("style"), Some("mix-blend-mode:screen"));
            let paint = layer
                .descendants()
                .find(|node| node.has_tag_name("rect"))
                .expect("layer must paint a real rectangle");
            let id = paint
                .attribute("fill")
                .and_then(|fill| fill.strip_prefix("url(#"))
                .and_then(|id| id.strip_suffix(')'))
                .expect("layer references its paint");
            let definition = document
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .expect("paint reference must resolve");
            if definition.has_tag_name("pattern") {
                assert_eq!(definition.attribute("patternUnits"), Some("userSpaceOnUse"));
                assert_eq!(definition.attribute("width"), Some("40"));
                assert_eq!(definition.attribute("height"), Some("40"));
            } else {
                assert!(
                    definition.has_tag_name("radialGradient"),
                    "{family}: canvas paint"
                );
            }
        }
    }
}
