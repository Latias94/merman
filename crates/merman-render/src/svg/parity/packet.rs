mod canonical;
mod render;

pub(in crate::svg::parity) use canonical::{PacketSvgStyles, packet_text_class};
pub(super) use render::render_packet_diagram_svg_model;
