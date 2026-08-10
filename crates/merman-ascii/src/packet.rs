use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::packet::PacketDiagramRenderModel;

pub fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    push_optional_framed_line(&mut document, "title", model.title.as_deref())?;
    push_optional_framed_line(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_framed_line(&mut document, "accDescr", model.acc_descr.as_deref())?;

    for (row_idx, row) in model.packet.iter().enumerate() {
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| line.write_fmt(format_args!("row {}:", row_idx + 1)))?;
        for block in row {
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

fn push_optional_framed_line(
    document: &mut BudgetedTextDocument,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    document.push_line_with(|line| {
        line.write_fmt(format_args!("{key}(bytes={})=", value.len()))?;
        line.push_quoted_text(value)
    })
}
