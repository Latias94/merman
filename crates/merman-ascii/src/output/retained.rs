//! Compose spatial tracks and complete records under one retained-output admission.

use super::{AsciiDiagram, AsciiError, Result, SemanticFallbackWriter};
use crate::operation::AsciiExecution;
use crate::{AsciiColorMode, AsciiRenderOptions, AsciiSourceContext};
use merman_model::OperationPhase;

pub(super) fn render_primary(
    model: &AsciiDiagram<'_>,
    source: AsciiSourceContext<'_>,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
    topology: Option<&str>,
) -> Result<String> {
    let resources = execution
        .new_resource_context(OperationPhase::Semantic)
        .with_additional_retained_output(topology.map_or(0, str::len))?;
    let serialization = execution.with_render_ledger(&resources);
    super::preflight_semantic_model(model, source, serialization)?;
    let projection =
        super::semantic_fallback_projection(model, source, serialization).map_err(|error| {
            match error {
                super::SemanticFallbackError::Cancelled(error) => AsciiError::Cancelled(error),
                super::SemanticFallbackError::Resource(error) => error,
                super::SemanticFallbackError::Unavailable => AsciiError::UnsupportedFeature {
                    diagram_type: "graph",
                    feature: "invalid serialized semantic projection",
                },
            }
        })?;
    let retained = resources
        .clone()
        .with_additional_retained_output(projection.len())?;
    let record_execution = execution.with_render_ledger(&retained);
    let record_width = execution.max_width().unwrap_or(80);
    let mut records = SemanticFallbackWriter::new(
        record_execution,
        record_width,
        options.terminal_width_profile,
    );
    if let Some(topology) = topology.filter(|text| !text.is_empty()) {
        // Reflowing spatial lines would destroy their topology. Width policy runs later.
        records.max_width = usize::MAX;
        for line in topology.lines() {
            records.push(line.to_string())?;
        }
        records.push(String::new())?;
        records.max_width = record_width;
    }
    match model {
        #[cfg(feature = "diagram-gantt")]
        AsciiDiagram::Gantt(_) => {
            records.push("Gantt records".to_string())?;
            super::gantt::write_if_gantt(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-git-graph")]
        AsciiDiagram::GitGraph(_) => {
            super::git_graph::write_if_git_graph(model, &projection, &mut records)?;
        }
        _ => {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "graph",
                feature: "retained reading records",
            });
        }
    }
    let candidate = records.finish().into_text();
    drop(projection);
    if options.color_mode != AsciiColorMode::Html {
        return Ok(candidate);
    }
    let mut document = crate::safe_text::BudgetedTextDocument::from_resources(
        resources
            .scoped()
            .with_additional_retained_output(candidate.len())?,
        options,
    );
    execution.rebind_resource_context(document.resources_mut(), OperationPhase::Emit);
    for line in candidate.split('\n') {
        document.push_line(line)?;
    }
    document.finish()
}
