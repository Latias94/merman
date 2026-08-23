use merman_ascii::{AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy};
use merman_core::diagram::{ParsedDiagramRender, RenderSemanticModel};
use merman_core::{Engine, OperationControl, ParseOptions, runtime::OperationContext};

pub(crate) fn parse_model(source: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("diagram should parse")
        .expect("diagram should be detected")
        .into_parts()
        .1
}

#[allow(dead_code)]
pub(crate) fn render_model(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    render_model_with_resources(model, options, AsciiResourcePolicy::default())
}

pub(crate) fn render_model_with_resources(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic test operation context");
    render_controlled_model(
        model,
        options,
        &OperationControl::new(),
        &context,
        resources,
    )
}

#[allow(dead_code)]
pub(crate) fn render_parsed(
    parsed: &ParsedDiagramRender,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic test operation context");
    AsciiRenderer::new(*options)?.render_parsed(
        parsed,
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default(),
    )
}

#[allow(dead_code)]
pub(crate) fn render_source(source: &str) -> String {
    render_model(&parse_model(source), &AsciiRenderOptions::ascii()).expect("diagram should render")
}

pub(crate) fn render_controlled_model(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    control: &OperationControl,
    context: &OperationContext,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    AsciiRenderer::new(*options)?.render_model(model, control, context, resources)
}
