use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextDocument, push_optional_document_field};
use merman_core::diagrams::packet::PacketDiagramRenderModel;

pub(super) fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    validate_packet_blocks(model, document.resources_mut(), execution)?;

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

    document.finish(options)
}

fn validate_packet_blocks(
    model: &PacketDiagramRenderModel,
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
