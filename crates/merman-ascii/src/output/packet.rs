//! Complete narrow Packet records, including numeric values JSON cannot represent.

use super::{
    AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter, flatten_serialized_json,
};
use serde::Deserialize;

pub(super) fn write_if_packet(
    model: &AsciiDiagram<'_>,
    projection: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Packet(model) = model else {
        return Ok(false);
    };
    for (key, value) in [
        ("title", model.title.as_deref()),
        ("accTitle", model.acc_title.as_deref()),
        ("accDescr", model.acc_descr.as_deref()),
    ] {
        if let Some(value) = value {
            lines.push_quoted(&format!("{key}(bytes={})", value.len()), value)?;
        }
    }
    for (row_index, row) in model.packet.iter().enumerate() {
        lines.push(format!("row {}:", row_index + 1))?;
        for (field_index, block) in row.iter().enumerate() {
            lines.push(format!(
                "  field {}: range=[{}..{}] bits={}",
                field_index + 1,
                ryu_js::Buffer::new().format(block.start),
                ryu_js::Buffer::new().format(block.end),
                ryu_js::Buffer::new().format(block.bits),
            ))?;
            lines.push_quoted(
                &format!("  label(bytes={})", block.label.len()),
                &block.label,
            )?;
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Source<'a> {
        #[serde(borrow)]
        source_config: &'a RawValue,
        #[serde(borrow)]
        source_title: &'a RawValue,
    }
    lines
        .semantic_resources
        .charge_layout_work(projection.len())?;
    let source: Source<'_> =
        serde_json::from_slice(projection).map_err(|_| AsciiError::InvalidOption {
            field: "packet_projection",
            message: "invalid admitted Packet source metadata",
        })?;
    for (key, value) in [
        ("sourceConfig", source.source_config),
        ("sourceTitle", source.source_title),
    ] {
        if value.get() != "{}" && value.get() != "null" {
            flatten_serialized_json(key, value.get().as_bytes(), lines)?;
        }
    }
    Ok(true)
}
