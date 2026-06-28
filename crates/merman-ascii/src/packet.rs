use crate::options::AsciiRenderOptions;
use crate::text::{normalize_optional_text, trim_trailing_blank_lines};
use merman_core::diagrams::packet::PacketDiagramRenderModel;

pub fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    _options: &AsciiRenderOptions,
) -> String {
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
        let blocks = row
            .iter()
            .map(|block| format!("[{}..{}] {}", block.start, block.end, block.label))
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("row {}: {}", row_idx + 1, blocks));
    }

    trim_trailing_blank_lines(lines).join("\n")
}
