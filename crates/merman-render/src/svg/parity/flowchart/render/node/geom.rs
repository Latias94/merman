//! Node geometry helpers (point generation and path assembly).
//!
//! These are ports of Mermaid rendering-element helpers and are used by multiple node shapes.

pub(super) use crate::svg::parity::roughjs_common::closed_path_d_from_points as path_from_points;

pub(in crate::svg::parity::flowchart) use merman_render_flow::node_body::document_shapes::generate_full_sine_wave_points;
