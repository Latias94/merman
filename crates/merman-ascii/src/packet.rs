use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::packet::PacketDiagramRenderModel;

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    document.push_optional_line(model.title.as_deref())?;
    document.push_optional_prefixed_line("accTitle: ", model.acc_title.as_deref())?;
    document.push_optional_prefixed_line("accDescr: ", model.acc_descr.as_deref())?;

    for (row_idx, row) in model.packet.iter().enumerate() {
        document.resources_mut().charge_layout_work(1)?;
        let prefix = format!("row {}: ", row_idx + 1);
        let continuation_prefix = " ".repeat(prefix.len());
        document.push_wrapped_prefixed_line_with(
            &prefix,
            &continuation_prefix,
            SUMMARY_WRAP_WIDTH,
            |line| {
                for (index, block) in row.iter().enumerate() {
                    if index > 0 {
                        line.push_str(" | ")?;
                    }
                    line.write_fmt(format_args!("[{}..{}] ", block.start, block.end))?;
                    line.push_str(&block.label)?;
                    line.write_fmt(format_args!(" ({} bits)", block.bits))?;
                }
                Ok(())
            },
        )?;
    }

    document.finish(options)
}
