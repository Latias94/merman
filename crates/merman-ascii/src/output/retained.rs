//! Compose spatial tracks and complete records under one retained-output admission.

use super::{AsciiDiagram, AsciiError, Result, SemanticFallbackWriter};
use crate::operation::AsciiExecution;
use crate::{AsciiColorMode, AsciiRenderOptions, AsciiSourceContext};
use merman_model::OperationPhase;

pub(crate) fn render_primary(
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
    let record_width = execution.max_width().unwrap_or(80);
    let mut records =
        SemanticFallbackWriter::new(serialization, record_width, options.terminal_width_profile);
    // Semantic checks inspect the already-admitted borrowed JSON. Only the growing output
    // reserves it as another live buffer, rather than charging its inspection as a second copy.
    records.candidate_resources = records
        .candidate_resources
        .clone()
        .with_additional_retained_output(projection.len())?;
    if let Some(topology) = topology.filter(|text| !text.is_empty()) {
        // Reflowing spatial lines would destroy their topology. Width policy runs later.
        records.max_width = usize::MAX;
        for line in topology.lines() {
            records.push(line)?;
        }
        records.push("")?;
        records.max_width = record_width;
    }
    match model {
        #[cfg(feature = "diagram-agentflow")]
        AsciiDiagram::Agentflow(_) => {
            records.push("Agent workflow records")?;
            super::agentflow::write_if_agentflow(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-architecture")]
        AsciiDiagram::Architecture(_) => {
            records.push("Architecture records")?;
            super::architecture::write_if_architecture(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-c4")]
        AsciiDiagram::C4(_) => {
            records.push("C4 records")?;
            super::c4::write_if_c4(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-requirement")]
        AsciiDiagram::Requirement(_) => {
            records.push("Requirement records")?;
            super::requirement::write_if_requirement(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-pie")]
        AsciiDiagram::Pie(_) => {
            records.push("Pie data")?;
            super::pie::write_if_pie(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-zenuml")]
        AsciiDiagram::Zenuml(_) => {
            records.push("ZenUML records")?;
            super::zenuml::write_if_zenuml(model, &projection, &mut records)?;
        }
        #[cfg(feature = "diagram-gantt")]
        AsciiDiagram::Gantt(_) => {
            records.push("Gantt records")?;
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
    crate::safe_text::encode_safe_html_document(
        &candidate,
        &resources
            .scoped()
            .with_operation_phase(OperationPhase::Emit)
            .with_additional_retained_output(candidate.len())?,
    )
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests {
    use super::*;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::SemanticParser;

    #[test]
    fn migrated_families_admit_exact_overview_serialization_and_record_peaks() {
        let policy = AsciiResourcePolicy::unbounded();
        for source in [
            "agentflow-beta\nA[Alpha]@{instruction: run}\n",
            "architecture-beta\nservice a(server)[Gateway]\n",
            "C4Context\nPerson(a, \"Reader\", \"A customer\")\n",
            "requirementDiagram\nrequirement r {\ntext: Complete\n}\n",
            "pie\n\"A\": 1\n",
            "zenuml\nAlice->Bob: Hello\n",
        ] {
            let mut parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
            let control_heavy = match &mut parsed.model {
                merman_model::SemanticDiagramModel::Architecture(model) => {
                    model.title = Some("\u{1}".repeat(16_384));
                    true
                }
                merman_model::SemanticDiagramModel::Zenuml(model) => {
                    model.title = Some("\u{1}".repeat(16_384));
                    true
                }
                _ => false,
            };
            let metadata = AsciiSourceContext::from(&parsed.metadata);
            let model = AsciiDiagram::from_semantic(
                &parsed.model,
                metadata,
                AsciiExecution::for_test(&policy),
            )
            .unwrap();
            let projection = super::super::semantic_fallback_projection(
                &model,
                metadata,
                AsciiExecution::for_test(&policy),
            )
            .unwrap_or_else(|_| panic!("valid complete record projection"));
            let overview = "overview <&>";
            let plain = render_primary(
                &model,
                metadata,
                &AsciiRenderOptions::ascii(),
                AsciiExecution::for_test(&policy),
                Some(overview),
            )
            .unwrap();
            if control_heavy {
                assert!(
                    plain.len() < projection.len(),
                    "JSON inspection must not reserve a second serialized buffer"
                );
            }
            for color in [AsciiColorMode::Plain, AsciiColorMode::Html] {
                let options = AsciiRenderOptions::ascii().with_color_mode(color);
                let encoded = render_primary(
                    &model,
                    metadata,
                    &options,
                    AsciiExecution::for_test(&policy),
                    Some(overview),
                )
                .unwrap();
                let peak = (overview.len() + projection.len() + plain.len()).max(
                    if color == AsciiColorMode::Html {
                        overview.len() + plain.len() + encoded.len()
                    } else {
                        0
                    },
                );
                for limit in [peak - 1, peak] {
                    let bounded = policy
                        .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                        .unwrap();
                    let result = render_primary(
                        &model,
                        metadata,
                        &options,
                        AsciiExecution::for_test(&bounded),
                        Some(overview),
                    );
                    if limit == peak {
                        assert_eq!(result.unwrap(), encoded, "{source}");
                    } else {
                        assert!(
                            matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                            if details.limit == AsciiResourceLimitId::MaxOutputBytes),
                            "{source}"
                        );
                    }
                }
            }
        }
    }
}
