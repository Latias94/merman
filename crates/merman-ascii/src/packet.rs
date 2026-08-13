use crate::Result;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::text::{
    display_width, normalize_optional_text, push_wrapped_prefixed_line, trim_trailing_blank_lines,
};
use merman_core::diagrams::packet::{PacketDiagramRenderModel, PacketRenderBlock};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    _options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut lines = Vec::new();

    if let Some(title) = normalize_optional_text(model.title.as_deref()) {
        lines.push(title);
    }
    if let Some(acc_title) = normalize_optional_text(model.acc_title.as_deref()) {
        lines.push(format!("accTitle: {acc_title}"));
    }
    if let Some(acc_descr) = normalize_optional_text(model.acc_descr.as_deref()) {
        lines.push(format!("accDescr: {acc_descr}"));
    }

    for (row_idx, row) in model.packet.iter().enumerate() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        let mut blocks = Vec::with_capacity(row.len());
        for block in row {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            blocks.push(render_block(block));
        }
        let blocks = blocks.join(" | ");
        let prefix = format!("row {}: ", row_idx + 1);
        let continuation_prefix = " ".repeat(display_width(&prefix));
        push_wrapped_prefixed_line(
            &mut lines,
            &prefix,
            &continuation_prefix,
            &blocks,
            SUMMARY_WRAP_WIDTH,
        );
    }

    Ok(trim_trailing_blank_lines(lines).join("\n"))
}

fn render_block(block: &PacketRenderBlock) -> String {
    format!(
        "[{}..{}] {} ({} bits)",
        block.start, block.end, block.label, block.bits
    )
}
