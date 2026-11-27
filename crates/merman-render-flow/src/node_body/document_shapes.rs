//! Complete child SVG for the Flowchart document shapes.

use merman_render_core::rough_svg::closed_path_d_from_points as path_from_points;

mod lined_wave_document;
mod stacked_document;
mod tagged_wave_document;
mod wave_document;

pub use lined_wave_document::LinedWaveDocumentBody;
pub use stacked_document::StackedDocumentBody;
pub use tagged_wave_document::TaggedWaveDocumentBody;
pub use wave_document::WaveDocumentBody;

/// Offset for the existing facade-owned label wrapper.
#[derive(Clone, Copy)]
pub struct DocumentLabelOffset {
    pub dx: f64,
    pub dy: f64,
}

#[doc(hidden)]
pub fn generate_full_sine_wave_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    amplitude: f64,
    num_cycles: f64,
) -> Vec<(f64, f64)> {
    // Ported from Mermaid `generateFullSineWavePoints` (50 segments).
    let steps: usize = 50;
    let delta_x = x2 - x1;
    let delta_y = y2 - y1;
    let cycle_length = delta_x / num_cycles;
    let frequency = (2.0 * std::f64::consts::PI) / cycle_length;
    let mid_y = y1 + delta_y / 2.0;

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = (i as f64) / (steps as f64);
        let x = x1 + t * delta_x;
        let y = mid_y + amplitude * (frequency * (x - x1)).sin();
        points.push((x, y));
    }
    points
}
