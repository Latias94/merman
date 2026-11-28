//! Flowchart v2 tagged wave-edged rectangle.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::paths::{RoughGeometryTarget, roughjs_prepare_paths_for_hand_drawn_solid_svg_path},
    svg_output::SvgOutput,
};

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use super::{DocumentLabelOffset, generate_full_sine_wave_points, path_from_points};

pub struct TaggedWaveDocumentBody {
    pub label: DocumentLabelOffset,
    width: f64,
    height: f64,
    wave_amplitude: f64,
}

impl TaggedWaveDocumentBody {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64) -> Self {
        // The layout box already includes the wave and tag, so start from the label box.
        let w = (label_width + 2.0 * padding).max(0.0);
        let h = (label_height + 2.0 * padding).max(0.0);
        let wave_amplitude = h / 8.0;

        // Mermaid shifts label to the left padding origin and up by waveAmplitude/2.
        let label = DocumentLabelOffset {
            dx: -w / 2.0 + padding + label_width / 2.0,
            dy: -h / 2.0 + padding - wave_amplitude / 2.0 + label_height / 2.0,
        };

        Self {
            label,
            width: w,
            height: h,
            wave_amplitude,
        }
    }

    fn paths(&self) -> (String, String) {
        let w = self.width;
        let h = self.height;
        let wave_amplitude = self.wave_amplitude;
        let tag_width = 0.2 * w;
        let tag_height = 0.2 * h;
        let final_h = h + wave_amplitude;
        let ext = (w / 2.0) * 0.1;
        let mut points: Vec<(f64, f64)> = Vec::new();
        points.push((-w / 2.0 - ext, final_h / 2.0));
        points.extend(generate_full_sine_wave_points(
            -w / 2.0 - ext,
            final_h / 2.0,
            w / 2.0 + ext,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.push((w / 2.0 + ext, -final_h / 2.0));
        points.push((-w / 2.0 - ext, -final_h / 2.0));

        let x = -w / 2.0 + ext;
        let y = -final_h / 2.0 - tag_height * 0.4;
        let mut tag_points: Vec<(f64, f64)> = Vec::new();
        tag_points.push((x + w - tag_width, (y + h) * 1.3));
        tag_points.push((x + w, y + h - tag_height));
        tag_points.push((x + w, (y + h) * 0.9));
        tag_points.extend(generate_full_sine_wave_points(
            x + w,
            (y + h) * 1.25,
            x + w - tag_width,
            (y + h) * 1.3,
            -h * 0.02,
            0.5,
        ));

        (path_from_points(&points), path_from_points(&tag_points))
    }

    pub fn write(self, out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>) {
        let (wave, tag) = self.paths();
        if body.geometry.hand_drawn {
            let wave_paths = body.hand_drawn_path_pair(&wave);
            let tag_paths = body
                .timed_rough(|geometry| {
                    roughjs_prepare_paths_for_hand_drawn_solid_svg_path(
                        RoughGeometryTarget::Svg,
                        &tag,
                        geometry.stroke_width,
                        geometry.stroke_dasharray,
                        geometry.work_meter,
                        geometry.randomness,
                    )
                    .and_then(|paths| geometry.write_rough_paths(paths))
                })
                .filter(|(fill_d, stroke_d)| !fill_d.is_empty() && !stroke_d.is_empty());

            if let (Some((wave_fill_d, wave_stroke_d)), Some((tag_fill_d, tag_stroke_d))) =
                (wave_paths, tag_paths)
            {
                let _ = write!(
                    out,
                    r##"<g class="basic label-container outer-path" transform="translate(0,{})" style="{}"><g>"##,
                    fmt(-self.wave_amplitude / 2.0),
                    escape_attr(body.hand_drawn_group_style()),
                );
                body.write_hand_drawn_path_pair(out, &wave, &wave_fill_d, &wave_stroke_d, 4.0);
                let _ = write!(
                    out,
                    r##"</g><path d="{}" stroke="none" stroke-width="0" fill="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"##,
                    escape_attr(&tag_fill_d),
                    escape_attr(body.fill_color),
                    escape_attr(&tag_stroke_d),
                    escape_attr(body.stroke_color),
                    fmt(body.geometry.stroke_width as f64),
                    escape_attr(body.geometry.stroke_dasharray),
                );
                return;
            }

            let _ = write!(
                out,
                r##"<g class="basic label-container outer-path" transform="translate(0,{})"><g><path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/></g><path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/></g>"##,
                fmt(-self.wave_amplitude / 2.0),
                escape_attr(&wave),
                escape_attr(body.fill_color),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.style),
                escape_attr(&tag),
                escape_attr(body.fill_color),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.style),
            );
            return;
        }

        let (wave_fill_d, wave_stroke_d) = body
            .solid_path_pair(&wave)
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
        let (tag_fill_d, tag_stroke_d) = body
            .solid_path_pair(&tag)
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

        let _ = write!(
            out,
            r##"<g class="basic label-container outer-path" transform="translate(0,{})"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
            fmt(-self.wave_amplitude / 2.0),
            escape_attr(&wave_fill_d),
            escape_attr(body.fill_color),
            escape_attr(body.style),
            escape_attr(&wave_stroke_d),
            escape_attr(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
            escape_attr(body.style),
            escape_attr(&tag_fill_d),
            escape_attr(body.fill_color),
            escape_attr(body.style),
            escape_attr(&tag_stroke_d),
            escape_attr(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
            escape_attr(body.style),
        );
    }

    pub fn advance_rough_geometry(self, geometry: NodeBodyGeometry<'_>) {
        let (wave, tag) = self.paths();
        let target = RoughGeometryTarget::Query;
        let _ = prepare_node_path(&geometry, &wave, target);
        if geometry.hand_drawn {
            let _ = roughjs_prepare_paths_for_hand_drawn_solid_svg_path(
                target,
                &tag,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            );
        } else {
            let _ = prepare_node_path(&geometry, &tag, target);
        }
    }
}
