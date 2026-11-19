//! Complete Block reading records without recursive subtree or per-field flattening.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use serde::Deserialize;
use std::collections::BTreeMap;

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(super) fn write_if_block(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Block(model) = model else {
        return Ok(false);
    };
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    write_plain_record(
        "Keys: B flat records (zero-based); E authored edges. Child order is authored; edge endpoints name IDs. null columns and -1 mean auto; null span means 1. Spacer width is already expanded.",
        lines,
    )?;
    write_plain_record(
        &model.root.map_or_else(
            || "root: null".to_string(),
            |index| format!("root: B{index}"),
        ),
        lines,
    )?;
    root.remove("root");
    let blocks: Vec<&RawValue> = take(&mut root, "blocks_flat", lines)?;
    if blocks.len() != model.blocks_flat.len() {
        return Err(invalid_projection());
    }
    for (index, (node, raw)) in model.blocks_flat.iter().zip(blocks).enumerate() {
        lines.admit_node(0)?;
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let key = format!("B{index}");
        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        record.text(" id=", false)?;
        record.text(&node.id, true)?;
        record.text(" type=", false)?;
        record.text(&node.block_type, true)?;
        record.text(" label=", false)?;
        record.text(&node.label, true)?;
        for name in ["id", "block_type", "label", "children"] {
            fields.remove(name);
        }
        for (name, display) in [
            ("columns", " columns="),
            ("width_in_columns", " span="),
            ("width", " spacer-count="),
        ] {
            record.text(display, false)?;
            record.text(
                fields.remove(name).ok_or_else(invalid_projection)?.get(),
                false,
            )?;
        }
        record.text(" children=[", false)?;
        for (position, child) in node.children.iter().enumerate() {
            if position > 0 {
                record.text(",", false)?;
            }
            record.text(&format!("B{child}"), false)?;
        }
        record.text("]", false)?;
        for (name, display) in [
            ("classes", " classes="),
            ("styles", " styles="),
            ("directions", " directions="),
        ] {
            record.text(display, false)?;
            record.text(
                fields.remove(name).ok_or_else(invalid_projection)?.get(),
                false,
            )?;
        }
        record.finish()?;
        details(&key, &fields, lines)?;
    }
    let edges: Vec<&RawValue> = take(&mut root, "edges", lines)?;
    if edges.len() != model.edges.len() {
        return Err(invalid_projection());
    }
    for (index, (edge, raw)) in model.edges.iter().zip(edges).enumerate() {
        lines.admit_node(0)?;
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let key = format!("E{index}");
        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        for (name, value) in [
            ("id", &edge.id),
            ("start", &edge.start),
            ("end", &edge.end),
            ("label", &edge.label),
        ] {
            record.text(&format!(" {name}="), false)?;
            record.text(value, true)?;
            fields.remove(name);
        }
        for (name, display) in [
            ("arrowTypeStart", " start-marker="),
            ("arrowTypeEnd", " end-marker="),
            ("thickness", " thickness="),
            ("pattern", " pattern="),
        ] {
            record.text(display, false)?;
            record.text(
                fields.remove(name).ok_or_else(invalid_projection)?.get(),
                false,
            )?;
        }
        record.finish()?;
        details(&key, &fields, lines)?;
    }
    let mut classes: Fields<'_> = take(&mut root, "class_defs", lines)?;
    for (index, name) in model.class_defs.keys().enumerate() {
        let raw = classes.remove(name).ok_or_else(invalid_projection)?;
        let key = format!("C{index}");
        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        record.text(" class=", false)?;
        record.text(name, true)?;
        record.text(" definition=", false)?;
        record.text(raw.get(), false)?;
        record.finish()?;
    }
    if !classes.is_empty() {
        return Err(invalid_projection());
    }
    let warnings: Vec<&RawValue> = take(&mut root, "warning_facts", lines)?;
    for (index, warning) in warnings.into_iter().enumerate() {
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("W{index} warning="), false)?;
        record.text(warning.get(), false)?;
        record.finish()?;
    }
    // Source title/configuration and any future envelope fields stay complete borrowed JSON.
    details("source", &root, lines)?;
    Ok(true)
}

fn take<'a, T: Deserialize<'a>>(
    fields: &mut Fields<'a>,
    name: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    decode(fields.remove(name).ok_or_else(invalid_projection)?, lines)
}

fn decode<'a, T: Deserialize<'a>>(
    raw: &'a RawValue,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    lines
        .semantic_resources
        .charge_layout_work(raw.get().len())?;
    serde_json::from_str(raw.get()).map_err(|_| invalid_projection())
}

fn invalid_projection() -> AsciiError {
    AsciiError::InvalidOption {
        field: "block_projection",
        message: "invalid admitted Block semantic records",
    }
}
