//! Node geometry helpers (point generation and path assembly).
//!
//! These are ports of Mermaid rendering-element helpers and are used by multiple node shapes.

pub(super) use merman_render_core::rough_svg::closed_path_d_from_points as path_from_points;
