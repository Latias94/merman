//! Flowchart v2 wave-edged rectangle.

use merman_render_core::svg_output::SvgOutput;

use super::super::{NodeBodyGeometry, NodeBodySvg, OuterPathBody, OuterPathTranslation};
use super::{DocumentLabelOffset, generate_full_sine_wave_points};

pub struct WaveDocumentBody {
    label_width: f64,
    label_height: f64,
    padding: f64,
    width: f64,
    height: f64,
    wave_amplitude: f64,
    path: OuterPathBody,
}

impl WaveDocumentBody {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding_x = if neo { 16.0 } else { padding };
        let padding_y = if neo { 12.0 } else { padding };
        // Use label dimensions because the layout box already includes the wave.
        let w = (label_width + 2.0 * padding_x).max(0.0);
        let h = (label_height + 2.0 * padding_y).max(0.0);
        let wave_amplitude = if neo { h / 4.0 } else { h / 8.0 };
        let final_h = h + wave_amplitude;

        // Mermaid keeps a minimum width (14px) for wave edged rectangles.
        let min_width = 14.0;
        let extra_w = ((min_width - w).max(0.0)) / 2.0;

        let mut points: Vec<(f64, f64)> = Vec::new();
        points.push((-w / 2.0 - extra_w, final_h / 2.0));
        points.extend(generate_full_sine_wave_points(
            -w / 2.0 - extra_w,
            final_h / 2.0,
            w / 2.0 + extra_w,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.push((w / 2.0 + extra_w, -final_h / 2.0));
        points.push((-w / 2.0 - extra_w, -final_h / 2.0));

        Self {
            label_width,
            label_height,
            padding,
            width: w,
            height: h,
            wave_amplitude,
            path: OuterPathBody::from_points(
                &points,
                OuterPathTranslation::Y(-wave_amplitude / 2.0),
            ),
        }
    }

    pub fn write(self, out: &mut impl SvgOutput, body: NodeBodySvg<'_>) -> DocumentLabelOffset {
        self.path.write(out, body);
        // Mirror Mermaid `waveEdgedRectangle.ts` label placement.
        DocumentLabelOffset {
            dx: -self.width / 2.0 + self.padding + self.label_width / 2.0,
            dy: -self.height / 2.0 + self.padding - self.wave_amplitude + self.label_height / 2.0,
        }
    }

    pub fn advance_rough_geometry(self, geometry: NodeBodyGeometry<'_>) {
        self.path.advance_rough_geometry(geometry);
    }
}
