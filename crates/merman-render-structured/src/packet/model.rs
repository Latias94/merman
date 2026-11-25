use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketBlockLayout {
    pub start: f64,
    pub end: f64,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketWordLayout {
    pub blocks: Vec<PacketBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketDiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    pub row_height: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub bit_width: f64,
    pub bits_per_row: f64,
    pub show_bits: bool,
    pub words: Vec<PacketWordLayout>,
}
