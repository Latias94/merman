use crate::Result;
use crate::model::{Bounds, PacketBlockLayout, PacketDiagramLayout, PacketWordLayout};
use crate::text::TextMeasurer;
use merman_core::diagrams::packet::PacketDiagramRenderModel;

mod config;

pub(crate) use config::PacketConfigView;

pub(crate) const PACKET_FONT_FAMILY_CSS: &str = r#""trebuchet ms",verdana,arial,sans-serif"#;

pub(crate) fn packet_title<'a>(
    semantic_title: Option<&'a str>,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    semantic_title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
}

pub(crate) fn packet_title_y(layout: &PacketDiagramLayout) -> f64 {
    layout.height - (layout.row_height + layout.padding_y) / 2.0
}

pub(crate) fn layout_packet_diagram_typed(
    model: &PacketDiagramRenderModel,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
) -> Result<PacketDiagramLayout> {
    let _ = (model.acc_title.as_deref(), model.acc_descr.as_deref());
    let cfg = PacketConfigView::new(effective_config)
        .layout_settings()
        .map_err(|error| crate::Error::InvalidModel {
            message: error.to_string(),
        })?;

    let total_row_height = cfg.row_height + cfg.padding_y;
    let has_title = packet_title(model.title.as_deref(), diagram_title).is_some();

    let words_count = model.packet.len();
    let svg_height = total_row_height * ((words_count + 1) as f64)
        - if has_title { 0.0 } else { cfg.row_height };
    let svg_width = cfg.bit_width * (cfg.bits_per_row as f64) + 2.0;

    let mut words: Vec<PacketWordLayout> = Vec::new();
    for (row_number, word) in model.packet.iter().enumerate() {
        let word_y = (row_number as f64) * total_row_height + cfg.padding_y;
        let mut blocks: Vec<PacketBlockLayout> = Vec::new();
        for block in word {
            let block_x = ((block.start % cfg.bits_per_row) as f64) * cfg.bit_width + 1.0;
            let width = ((block.end - block.start + 1) as f64) * cfg.bit_width - cfg.padding_x;
            blocks.push(PacketBlockLayout {
                start: block.start,
                end: block.end,
                label: block.label.clone(),
                x: block_x,
                y: word_y,
                width,
                height: cfg.row_height,
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
        row_height: cfg.row_height,
        padding_x: cfg.padding_x,
        padding_y: cfg.padding_y,
        bit_width: cfg.bit_width,
        bits_per_row: cfg.bits_per_row,
        show_bits: cfg.show_bits,
        words,
    })
}
