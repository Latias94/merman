//! Flowchart v2 lined wave-edged rectangle.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::paths::{
        RoughGeometryTarget, roughjs_paths_for_polygon, roughjs_prepare_paths_for_polygon,
    },
    svg_output::SvgOutput,
};

use super::super::{
    NodeBodyGeometry, NodeBodySvg, OuterPathTranslation, escape_attr, prepare_node_path,
};
use super::{DocumentLabelOffset, generate_full_sine_wave_points, path_from_points};

pub struct LinedWaveDocumentBody {
    pub label: DocumentLabelOffset,
    width: f64,
    final_height: f64,
    wave_amplitude: f64,
    extension: f64,
}

impl LinedWaveDocumentBody {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding_x = if neo { 16.0 } else { padding };
        let padding_y = if neo { 12.0 } else { padding };
        let w = (label_width + 2.0 * padding_x).max(0.0);
        let h = (label_height + 2.0 * padding_y).max(0.0);
        let wave_amplitude = if neo { h / 4.0 } else { h / 8.0 };
        let final_h = h + wave_amplitude;
        let ext = (w / 2.0) * 0.1;

        // Mermaid keeps the authored padding in the label transform while the shape uses
        // look-specific axis padding. Convert that source transform to the local label wrapper base.
        let label = DocumentLabelOffset {
            dx: -padding_x + padding + ext / 2.0,
            dy: -padding_y + padding - wave_amplitude,
        };

        Self {
            label,
            width: w,
            final_height: final_h,
            wave_amplitude,
            extension: ext,
        }
    }

    fn points(&self) -> Vec<(f64, f64)> {
        let w = self.width;
        let final_h = self.final_height;
        let ext = self.extension;
        let mut points: Vec<(f64, f64)> = Vec::new();
        points.push((-w / 2.0 - ext, -final_h / 2.0));
        points.push((-w / 2.0 - ext, final_h / 2.0));
        points.extend(generate_full_sine_wave_points(
            -w / 2.0 - ext,
            final_h / 2.0,
            w / 2.0 + ext,
            final_h / 2.0,
            self.wave_amplitude,
            0.8,
        ));
        points.push((w / 2.0 + ext, -final_h / 2.0));
        points.push((-w / 2.0 - ext, -final_h / 2.0));
        points.push((-w / 2.0, -final_h / 2.0));
        points.push((-w / 2.0, (final_h / 2.0) * 1.1));
        points.push((-w / 2.0, -final_h / 2.0));

        points
    }

    pub fn write(self, out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>) {
        let wave_amplitude = self.wave_amplitude;
        let points = self.points();
        let path = path_from_points(&points);
        if body.geometry.hand_drawn {
            OuterPathTranslation::Y(-wave_amplitude / 2.0).write_hand_drawn(out, &mut body, &path);
            return;
        }

        let (fill_d, stroke_d) = body
            .timed_rough(|geometry| {
                roughjs_paths_for_polygon(
                    &points,
                    geometry.stroke_width,
                    geometry.work_meter,
                    geometry.randomness,
                )
            })
            .unwrap_or_else(|| (path.clone(), path));
        let _ = write!(
            out,
            r##"<g class="basic label-container outer-path" transform="translate(0,{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
            fmt(-wave_amplitude / 2.0),
            escape_attr(&fill_d),
            escape_attr(body.fill_color),
            escape_attr(body.style),
            escape_attr(&stroke_d),
            escape_attr(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
            escape_attr(body.style),
        );
    }

    pub fn advance_rough_geometry(self, geometry: NodeBodyGeometry<'_>) {
        let points = self.points();
        let path = path_from_points(&points);
        let target = RoughGeometryTarget::Query;
        if geometry.hand_drawn {
            let _ = prepare_node_path(&geometry, &path, target);
        } else {
            let _ = roughjs_prepare_paths_for_polygon(
                target,
                &points,
                geometry.stroke_width,
                geometry.work_meter,
                geometry.randomness,
            );
        }
    }
}
