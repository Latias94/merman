mod layout;
mod paint;
mod plan;

use crate::operation::AsciiExecution;
use crate::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiProjection, AsciiRenderOptions,
    AsciiSourceContext, AsciiViewportPolicy, Result,
};
use merman_model::OperationPhase;
use merman_model::families::zenuml::ZenumlDiagramModel;

pub(crate) fn render(
    model: &ZenumlDiagramModel,
    options: &AsciiRenderOptions,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    crate::output::preflight_semantic_model(&AsciiDiagram::Zenuml(model), source, execution)?;
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    let topology = if let Some(plan) = plan::prepare(model, &resources)? {
        let scene_resources = resources.scoped();
        let scene_execution = execution
            .with_render_ledger(&scene_resources)
            .with_trimmed_canvas()
            .with_viewport(AsciiViewportPolicy::unrestricted());
        let resources = scene_execution.resource_context(&scene_resources, OperationPhase::Layout);
        let scene = layout::prepare(model, &plan, options, &resources)?;
        Some(paint::render(
            &scene,
            &options.with_color_mode(AsciiColorMode::Plain),
            &resources,
            scene_execution,
        )?)
    } else {
        None
    };
    let projection = if topology.is_some() {
        AsciiProjection::Diagrammatic
    } else {
        AsciiProjection::StructuredText
    };
    crate::output::render_reading_primary(
        &AsciiDiagram::Zenuml(model),
        source,
        options,
        execution,
        topology.as_deref(),
    )
    .map(|text| crate::output::RenderedPrimary::records(text, projection))
}

fn reserve<T>(items: &mut Vec<T>, additional: usize) -> Result<()> {
    items
        .try_reserve(additional)
        .map_err(|_| allocation_failed())
}

fn push<T>(items: &mut Vec<T>, item: T) -> Result<()> {
    reserve(items, 1)?;
    items.push(item);
    Ok(())
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_zenuml")
}
