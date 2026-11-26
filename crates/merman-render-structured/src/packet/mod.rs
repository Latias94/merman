//! Packet layout, presentation binding, and SVG emission.

use merman_model::families::packet::PacketDiagramModel;

mod config;
mod layout;
mod model;
mod style;
mod svg;

pub use layout::layout_packet_diagram_typed;
pub use model::{PacketBlockLayout, PacketDiagramLayout, PacketWordLayout};
pub use style::PacketStyle;
pub use svg::write_svg_contents;

fn terminal_title<'a>(
    model: &'a PacketDiagramModel,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
}
