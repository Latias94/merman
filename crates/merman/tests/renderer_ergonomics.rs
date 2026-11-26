#![cfg(feature = "diagram-flowchart")]

use merman::{LayoutBackend, MermaidConfig, OperationControl, Renderer};

const FLOWCHART: &str = "flowchart TD\nA[Start] --> B[Done]";

#[test]
fn renderer_builder_site_layout_is_a_default_not_a_source_override() {
    let renderer = Renderer::builder()
        .with_default_layout(LayoutBackend::Shoal)
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "layout": "elk",
            "theme": "dark"
        })))
        .build()
        .unwrap();
    let authored = format!("---\nconfig:\n  layout: dagre\n---\n{FLOWCHART}");
    for (source, layout) in [(FLOWCHART, "shoal"), (authored.as_str(), "dagre")] {
        let artifact = renderer
            .parse_semantic(source, OperationControl::new())
            .unwrap()
            .unwrap();
        let config = &artifact.metadata().semantic_config;
        assert_eq!(config.get_str("layout"), Some(layout));
        assert_eq!(config.get_str("theme"), Some("dark"));

        #[cfg(feature = "presentation")]
        {
            let artifact = renderer
                .prepare_presentation(source, OperationControl::new())
                .unwrap()
                .unwrap();
            let config = &artifact.metadata().effective_config;
            assert_eq!(config.get_str("layout"), Some(layout));
            assert_eq!(config.get_str("theme"), Some("dark"));
        }
    }
}

#[cfg(all(
    feature = "svg",
    any(feature = "layout-dagre", feature = "layout-shoal")
))]
mod graphical {
    use super::*;
    use merman::model::resources::{InputResourceLimitId, InputResourcePolicy, ResourceProfile};
    use merman::svg::{RenderCapability, RenderResourcePolicy};
    use merman::theme::{Appearance, BuiltinPreset, ThemeCompiler, ThemeMode, ThemeSelection};
    use merman::{
        MermaidEngine, OperationPhase, ParseOptions, RenderError, RenderOutput, RenderRequest,
        RendererBuilder, RendererBuilderError, SvgEnvironment, SvgRequest,
    };

    fn builder() -> RendererBuilder {
        let backend = if cfg!(feature = "layout-dagre") {
            LayoutBackend::Dagre
        } else {
            LayoutBackend::Shoal
        };
        Renderer::builder().with_default_layout(backend)
    }

    #[test]
    fn renderer_builder_svg_matches_the_expert_request_without_decorating_explicit_targets() {
        let renderer = builder()
            .with_theme_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
            .build()
            .unwrap();
        let typed = renderer.render_svg(FLOWCHART).unwrap();
        let RenderOutput::Svg(Some(expert)) = renderer
            .render(RenderRequest::svg(
                FLOWCHART,
                OperationControl::new(),
                SvgRequest::default().with_native_theme(ThemeSelection::preset(
                    BuiltinPreset::Studio,
                    Some(Appearance::Dark),
                )),
            ))
            .unwrap()
        else {
            panic!("expected an SVG artifact");
        };
        assert_eq!(typed.svg(), expert.svg());
        assert_eq!(typed.theme_outcome(), expert.theme_outcome());
        assert_eq!(typed.theme_outcome().preset_id, Some(BuiltinPreset::Studio));
        let mut request = renderer.default_svg_request();
        request.options.diagram_id = Some("explicit-diagram".to_owned());
        let inherited = renderer
            .render_svg_with(FLOWCHART, request, OperationControl::new())
            .unwrap();
        assert!(inherited.svg().contains(r#"id="explicit-diagram""#));
        assert_eq!(typed.theme_outcome(), inherited.theme_outcome());
        let capability = if cfg!(feature = "layout-dagre") {
            RenderCapability::LayoutDagre
        } else {
            RenderCapability::LayoutShoal
        };
        assert!(
            typed
                .evidence()
                .required_capabilities()
                .contains(&capability)
        );

        let RenderOutput::Svg(Some(unthemed)) = renderer
            .render(RenderRequest::svg(
                FLOWCHART,
                OperationControl::new(),
                SvgRequest::default(),
            ))
            .unwrap()
        else {
            panic!("expected an unthemed SVG artifact");
        };
        let explicit = renderer
            .render_svg_with(FLOWCHART, SvgRequest::default(), OperationControl::new())
            .unwrap();
        assert_eq!(explicit.svg(), unthemed.svg());
        assert_eq!(explicit.theme_outcome().mode, ThemeMode::MermaidParity);
    }

    #[test]
    fn renderer_builder_accepts_a_compiled_theme() {
        let compiled = ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
            .unwrap();
        let renderer = builder()
            .with_native_theme(ThemeSelection::compiled(compiled))
            .build()
            .unwrap();
        let svg = renderer.render_svg(FLOWCHART).unwrap();
        assert_eq!(svg.theme_outcome().mode, ThemeMode::MermanNative);
        assert_eq!(svg.theme_outcome().appearance, Some(Appearance::Dark));
        assert_eq!(svg.theme_outcome().preset_id, None);
    }

    #[test]
    fn renderer_builder_limits_only_default_targets_and_explicit_control_keeps_precedence() {
        let render_policy = RenderResourcePolicy::constrained()
            .with_override("max_source_bytes", 1)
            .unwrap()
            .with_override("max_model_items", 1)
            .unwrap()
            .with_override("max_svg_bytes", 1)
            .unwrap();
        let input = InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxSourceBytes, FLOWCHART.len())
            .unwrap()
            .with_limit(InputResourceLimitId::MaxModelItems, 3)
            .unwrap();
        let renderer = builder()
            .with_render_resource_policy(render_policy)
            .with_svg_environment(SvgEnvironment::deterministic())
            .with_input_resource_policy(input)
            .build()
            .unwrap();
        assert!(matches!(
            renderer.render_svg(&format!("{FLOWCHART}\n")),
            Err(RenderError::ResourceLimitExceeded(error))
                if error.id == "max_source_bytes" && error.maximum == FLOWCHART.len() as u64
        ));
        assert!(matches!(
            renderer.render_svg(FLOWCHART),
            Err(RenderError::ResourceLimitExceeded(error))
                if error.id == "max_svg_bytes" && error.maximum == 1
                    && error.provenance.as_ref().and_then(|receipt| receipt.profile)
                        == Some(ResourceProfile::Constrained)
        ));

        let request = SvgRequest {
            environment: SvgEnvironment::deterministic().with_resource_policy(render_policy),
            ..SvgRequest::default()
        };
        let control = OperationControl::new();
        control.cancel();
        assert!(matches!(
            renderer.render_svg_with(FLOWCHART, request, control),
            Err(RenderError::Cancelled(error)) if error.phase == OperationPhase::Admission
        ));
        assert!(
            renderer
                .render_svg_with(FLOWCHART, SvgRequest::default(), OperationControl::new())
                .is_ok()
        );
    }

    #[test]
    fn renderer_can_still_be_borrowed_across_an_unwind_boundary() {
        let renderer = Renderer::new();
        assert_eq!(
            std::panic::catch_unwind(|| *renderer.resource_policy()).unwrap(),
            InputResourcePolicy::default()
        );
    }

    #[test]
    fn renderer_builder_rejects_unbounded_defaults_and_keeps_trusted_expert_policy_explicit() {
        let unbounded = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        assert!(matches!(
            builder().with_input_resource_policy(unbounded).build(),
            Err(RendererBuilderError::UnboundedInputPolicy)
        ));
        let environment = SvgEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input());
        assert!(matches!(
            builder().with_svg_environment(environment).build(),
            Err(RendererBuilderError::UnboundedRenderPolicy)
        ));
        let unbounded_render = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_input_policy(InputResourcePolicy::default());
        assert!(matches!(
            builder()
                .with_render_resource_policy(unbounded_render)
                .build(),
            Err(RendererBuilderError::UnboundedRenderPolicy)
        ));
        let unbounded_input = RenderResourcePolicy::default().with_input_policy(unbounded);
        assert!(matches!(
            builder()
                .with_render_resource_policy(unbounded_input)
                .build(),
            Err(RendererBuilderError::UnboundedInputPolicy)
        ));

        let maximum = InputResourcePolicy::default()
            .value(InputResourceLimitId::MaxSourceBytes)
            .unwrap();
        let source = " ".repeat(maximum + 1);
        let renderer = builder()
            .build()
            .unwrap()
            .with_parse_options(ParseOptions::lenient())
            .with_resource_policy(unbounded);
        assert!(matches!(
            renderer.render_svg(&source),
            Err(RenderError::ResourceLimitExceeded(error))
                if error.id == "max_source_bytes" && error.maximum == maximum as u64
        ));
        assert!(matches!(
            renderer.render(RenderRequest::svg(
                &source,
                OperationControl::new(),
                SvgRequest::default()
            )),
            Ok(RenderOutput::Svg(None))
        ));
    }

    #[test]
    fn renderer_builder_typed_svg_distinguishes_no_diagram_from_parse_errors() {
        let renderer = builder().build().unwrap();
        assert!(matches!(
            renderer.render_svg(""),
            Err(RenderError::NoDiagram)
        ));
        assert!(matches!(
            renderer.render_svg("flowchart TD\nA[unterminated"),
            Err(RenderError::Parse(_))
        ));
        assert!(matches!(
            renderer.render(RenderRequest::svg(
                "",
                OperationControl::new(),
                SvgRequest::default()
            )),
            Err(RenderError::Parse(_))
        ));
        let renderer = renderer.with_parse_options(ParseOptions::lenient());
        assert!(matches!(
            renderer.render_svg(""),
            Err(RenderError::NoDiagram)
        ));
        assert!(matches!(
            renderer.render(RenderRequest::svg(
                "",
                OperationControl::new(),
                SvgRequest::default()
            )),
            Ok(RenderOutput::Svg(None))
        ));
    }

    #[test]
    fn renderer_builder_preserves_a_detected_family_parsers_detection_error() {
        let mut engine = MermaidEngine::new();
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", |_, _, _| {
                Ok(Err(merman_core::detect::DetectTypeError {
                    text: "unrecognized embedded diagram".to_owned(),
                }
                .into()))
            });
        let renderer = builder().build().unwrap().with_presentation_engine(engine);
        for result in [
            renderer.render_svg(FLOWCHART),
            renderer.render_svg_with(FLOWCHART, SvgRequest::default(), OperationControl::new()),
        ] {
            assert!(matches!(
                result,
                Err(RenderError::Parse(error))
                    if error.terminal_diagnostic_details().code == "merman.parse.no_diagram_detected"
            ));
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn renderer_builder_renders_each_enabled_binary_target() {
        let source = format!(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\n{FLOWCHART}"
        );
        let renderer = builder().build().unwrap();
        #[cfg(feature = "png")]
        assert!(
            renderer
                .render_png(&source)
                .unwrap()
                .bytes()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        #[cfg(feature = "jpeg")]
        assert!(
            renderer
                .render_jpeg(&source)
                .unwrap()
                .bytes()
                .starts_with(b"\xff\xd8")
        );
        #[cfg(feature = "pdf")]
        assert!(
            renderer
                .render_pdf(&source)
                .unwrap()
                .bytes()
                .starts_with(b"%PDF-")
        );

        let renderer = renderer.with_parse_options(ParseOptions::lenient());
        #[cfg(feature = "png")]
        assert!(matches!(
            renderer.render_png(""),
            Err(RenderError::NoDiagram)
        ));
        #[cfg(feature = "jpeg")]
        assert!(matches!(
            renderer.render_jpeg(""),
            Err(RenderError::NoDiagram)
        ));
        #[cfg(feature = "pdf")]
        assert!(matches!(
            renderer.render_pdf(""),
            Err(RenderError::NoDiagram)
        ));
    }
}

#[cfg(feature = "ascii")]
#[test]
fn renderer_builder_ascii_uses_the_same_request_and_keeps_terminal_color_opt_in() {
    use merman::ascii::{AsciiColorMode, AsciiRenderOptions};
    use merman::theme::{BuiltinPreset, ThemeSelection};
    use merman::{AsciiRequest, ParseOptions, RenderError, RenderOutput, RenderRequest};

    let options = AsciiRenderOptions::default().with_color_mode(AsciiColorMode::TrueColor);
    let selection = ThemeSelection::preset(BuiltinPreset::Studio, None);
    let renderer = Renderer::builder()
        .with_ascii_options(options)
        .with_native_theme(selection)
        .build()
        .unwrap();
    let colored = renderer.render_ascii(FLOWCHART).unwrap();
    assert!(colored.text.contains("\x1b["));
    assert_eq!(
        colored,
        renderer
            .render_ascii_with(
                FLOWCHART,
                renderer.default_ascii_request(),
                OperationControl::new(),
            )
            .unwrap()
    );
    let RenderOutput::Ascii(Some(expert)) = renderer
        .render(RenderRequest::ascii(
            FLOWCHART,
            OperationControl::new(),
            AsciiRequest::default(),
        ))
        .unwrap()
    else {
        panic!("expected terminal output");
    };
    assert!(!expert.text.contains("\x1b["));
    assert!(matches!(
        renderer
            .with_parse_options(ParseOptions::lenient())
            .render_ascii(""),
        Err(RenderError::NoDiagram)
    ));
}
