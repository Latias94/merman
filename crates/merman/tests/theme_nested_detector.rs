#![cfg(all(feature = "svg", feature = "diagram-state"))]

use merman::svg::{DiagramThemeCompiler, ThemePreset};
use merman::{
    DetectorRegistry, Engine, MermaidConfig, OperationControl, RenderError, RenderOutput,
    RenderRequest, Renderer,
};

const SOURCE: &str = "stateDiagram-v2\nReady --> Done\n";

fn replace_config_with_nested_theme(_source: &str, config: &mut MermaidConfig) -> bool {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::EditorDark)
        .unwrap();
    let RenderOutput::Semantic(Some(semantic)) = Renderer::new()
        .render(RenderRequest::semantic(SOURCE, OperationControl::new()).with_theme(theme))
        .unwrap()
    else {
        panic!("nested themed semantic operation must produce a diagram");
    };
    *config = semantic.metadata().effective_config.clone();
    true
}

fn render_after_nested_detector(preset: ThemePreset) -> Result<RenderOutput, RenderError> {
    let mut engine = Engine::new();
    *engine.registry_mut() = DetectorRegistry::new();
    engine
        .registry_mut()
        .add_fn("stateDiagram", replace_config_with_nested_theme);
    let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
    Renderer::new().with_engine(engine).render(
        RenderRequest::svg(SOURCE, OperationControl::new(), Default::default()).with_theme(theme),
    )
}

#[test]
fn nested_detector_preserves_matching_frozen_theme_binding() {
    let RenderOutput::Svg(Some(svg)) = render_after_nested_detector(ThemePreset::EditorDark)
        .expect("a nested parse with the same compiled theme must retain its binding")
    else {
        panic!("the outer themed operation must produce SVG");
    };
    assert!(svg.svg().contains("Ready"));
    assert!(svg.svg().contains("Done"));
}

#[test]
fn nested_detector_rejects_different_frozen_theme_binding() {
    assert!(matches!(
        render_after_nested_detector(ThemePreset::EditorLight),
        Err(RenderError::Svg(
            merman::svg::RenderError::ThemeParseBindingMismatch
        ))
    ));
}
