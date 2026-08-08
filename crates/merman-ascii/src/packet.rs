use crate::options::AsciiRenderOptions;
use crate::safe_text::encode_text_lines;
use crate::text::{
    display_width_with_profile, normalize_optional_text, push_wrapped_prefixed_line_with_profile,
    trim_trailing_blank_lines,
};
use merman_core::diagrams::packet::{PacketDiagramRenderModel, PacketRenderBlock};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    options: &AsciiRenderOptions,
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
        let blocks = row.iter().map(render_block).collect::<Vec<_>>().join(" | ");
        let prefix = format!("row {}: ", row_idx + 1);
        let continuation_prefix = " ".repeat(display_width_with_profile(
            &prefix,
            options.terminal_width_profile,
        ));
        push_wrapped_prefixed_line_with_profile(
            &mut lines,
            &prefix,
            &continuation_prefix,
            &blocks,
            SUMMARY_WRAP_WIDTH,
            options.terminal_width_profile,
        );
    }

    encode_text_lines(trim_trailing_blank_lines(lines), options)
}

fn render_block(block: &PacketRenderBlock) -> String {
    format!(
        "[{}..{}] {} ({} bits)",
        block.start, block.end, block.label, block.bits
    )
}
