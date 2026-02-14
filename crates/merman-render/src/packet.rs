use crate::Result;
use crate::model::{Bounds, PacketBlockLayout, PacketDiagramLayout, PacketWordLayout};
use crate::text::TextMeasurer;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
struct PacketBlock {
    start: i64,
    end: i64,
    label: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PacketModel {
    packet: Vec<Vec<PacketBlock>>,
    title: Option<String>,
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
}

pub fn layout_packet_diagram(
    semantic: &serde_json::Value,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
) -> Result<PacketDiagramLayout> {
    let model: PacketModel = crate::json::from_value_ref(semantic)?;
    let _ = (model.acc_title.as_deref(), model.acc_descr.as_deref());

    fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
        let mut cur = cfg;
        for key in path {
            cur = cur.get(*key)?;
        }
        cur.as_bool()
    }

    fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
        let mut cur = cfg;
        for key in path {
            cur = cur.get(*key)?;
        }
        cur.as_f64()
            .or_else(|| cur.as_i64().map(|n| n as f64))
            .or_else(|| cur.as_u64().map(|n| n as f64))
    }

    fn config_i64(cfg: &Value, path: &[&str]) -> Option<i64> {
        let mut cur = cfg;
        for key in path {
            cur = cur.get(*key)?;
        }
        cur.as_i64()
    }

    // Mermaid 11.12.2 defaults (see `repo-ref/mermaid/.../packet/db.ts` + `DEFAULT_CONFIG.packet`).
    let show_bits: bool = config_bool(effective_config, &["packet", "showBits"]).unwrap_or(true);
    let row_height: f64 = config_f64(effective_config, &["packet", "rowHeight"])
        .unwrap_or(32.0)
        .max(1.0);
    let padding_x: f64 = config_f64(effective_config, &["packet", "paddingX"])
        .unwrap_or(5.0)
        .max(0.0);
    // Mermaid applies `showBits` after merging defaults+user config by bumping `paddingY` by +10.
    // See `repo-ref/mermaid/packages/mermaid/src/diagrams/packet/db.ts`.
    let mut padding_y: f64 = config_f64(effective_config, &["packet", "paddingY"])
        .unwrap_or(5.0)
        .max(0.0);
    if show_bits {
        padding_y += 10.0;
    }
    let bit_width: f64 = config_f64(effective_config, &["packet", "bitWidth"])
        .unwrap_or(32.0)
        .max(1.0);
    let bits_per_row: i64 = config_i64(effective_config, &["packet", "bitsPerRow"])
        .unwrap_or(32)
        .max(1);

    let total_row_height = row_height + padding_y;
    let title_from_semantic = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_from_meta = diagram_title.map(str::trim).filter(|t| !t.is_empty());
    let has_title = title_from_semantic.or(title_from_meta).is_some();

    let words_count = model.packet.len();
    let svg_height =
        total_row_height * ((words_count + 1) as f64) - if has_title { 0.0 } else { row_height };
    let svg_width = bit_width * (bits_per_row as f64) + 2.0;

    let mut words: Vec<PacketWordLayout> = Vec::new();
    for (row_number, word) in model.packet.iter().enumerate() {
        let word_y = (row_number as f64) * total_row_height + padding_y;
        let mut blocks: Vec<PacketBlockLayout> = Vec::new();
        for block in word {
            let block_x = ((block.start % bits_per_row) as f64) * bit_width + 1.0;
            let width = ((block.end - block.start + 1) as f64) * bit_width - padding_x;
            blocks.push(PacketBlockLayout {
                start: block.start,
                end: block.end,
                label: block.label.clone(),
                x: block_x,
                y: word_y,
                width,
                height: row_height,
            });
        }
        words.push(PacketWordLayout { blocks });
    }

    Ok(PacketDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: svg_width,
            max_y: svg_height.max(1.0),
        }),
        width: svg_width,
        height: svg_height.max(1.0),
        row_height,
        padding_x,
        padding_y,
        bit_width,
        bits_per_row,
        show_bits,
        words,
    })
}
