use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextDocument, push_optional_document_field};
use merman_core::diagrams::packet::PacketDiagramModel;

pub(super) fn render_packet_diagram(
    model: &PacketDiagramModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources = execution.new_resource_context(merman_core::OperationPhase::Layout);
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    validate_packet_blocks(model, document.resources_mut(), execution)?;
    execution.rebind_resource_context(document.resources_mut(), merman_core::OperationPhase::Emit);

    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", model.acc_descr.as_deref())?;

    for (row_idx, row) in model.packet.iter().enumerate() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| line.write_fmt(format_args!("row {}:", row_idx + 1)))?;
        for block in row {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            document.resources_mut().charge_layout_work(1)?;
            document.push_line_with(|line| {
                line.write_fmt(format_args!(
                    "  - range=[{}..{}] bits={} label(bytes={})=",
                    block.start,
                    block.end,
                    block.bits,
                    block.label.len(),
                ))?;
                line.push_quoted_text(&block.label)
            })?;
        }
    }

    document.finish_with_execution(execution)
}

fn validate_packet_blocks(
    model: &PacketDiagramModel,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let block_count = model
        .packet
        .iter()
        .try_fold(0usize, |total, row| total.checked_add(row.len()));
    let block_count = block_count.ok_or_else(|| resources.work_overflow())?;
    resources.charge_layout_work(block_count)?;

    for row in &model.packet {
        for block in row {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            if block.start > block.end {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet blocks with end before start",
                });
            }
            let expected_bits = block
                .end
                .checked_sub(block.start)
                .and_then(|value| value.checked_add(1))
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet block range overflow",
                })?;
            if block.bits <= 0 {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet blocks must have positive bit counts",
                });
            }
            if block.bits != expected_bits {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet block bit count does not match inclusive range",
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;

    #[test]
    fn canonical_packet_matches_full_row_and_metadata_output() {
        use merman_core::{
            MermaidEngine, ParseOptions, RenderSemanticModel, SemanticDiagramModel, SemanticParser,
        };

        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        for source in [
            "packet\ntitle Header\naccTitle: Accessible\naccDescr: Complete packet\n0-7: \"Type\"\n8-39: \"Payload\"\n",
            "packet\n",
        ] {
            let canonical = SemanticParser::new().parse_diagram_sync(source).unwrap();
            let SemanticDiagramModel::Packet(model) = &canonical.model else {
                panic!("Packet model")
            };
            let full = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Packet(full) = full.model() else {
                panic!("Packet model")
            };
            assert!(std::ptr::eq(
                full.packet.as_ptr(),
                full.semantic.packet.as_ptr()
            ));
            let actual =
                render_packet_diagram(model, &options, AsciiExecution::for_test(&policy)).unwrap();
            let expected =
                render_packet_diagram(&full.semantic, &options, AsciiExecution::for_test(&policy))
                    .unwrap();
            assert_eq!(actual, expected, "{source}");
        }
    }
}
