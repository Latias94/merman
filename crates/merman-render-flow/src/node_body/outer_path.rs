//! Single-contour Flowchart bodies using the Mermaid outer-path group.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{closed_path_d_from_points, mermaid_arc_points, paths::RoughGeometryTarget},
    svg_output::SvgOutput,
};

use super::{
    NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path, write_raw_filled_stroked_path,
};

/// Translation spelling used by the SVG group for a single contour.
#[derive(Clone, Copy)]
pub enum OuterPathTranslation {
    None,
    X(f64),
    Y(f64),
    Xy(f64, f64),
}

// Join literals so each branch keeps its original SVG append boundaries for byte admission
// and cancellation. The translated hand-drawn fallback can have different comma spacing.
macro_rules! write_outer_path_group {
    ($out:expr, $translation:expr, $xy_separator:literal, $tail:literal $(, $arg:expr)* $(,)?) => {
        match $translation {
            OuterPathTranslation::None => write!(
                $out,
                concat!(r#"<g class="basic label-container outer-path""#, $tail)
                $(, $arg)*
            ),
            OuterPathTranslation::X(x) => write!(
                $out,
                concat!(r#"<g class="basic label-container outer-path" transform="translate({},0)""#, $tail),
                fmt(x) $(, $arg)*
            ),
            OuterPathTranslation::Y(y) => write!(
                $out,
                concat!(r#"<g class="basic label-container outer-path" transform="translate(0,{})""#, $tail),
                fmt(y) $(, $arg)*
            ),
            OuterPathTranslation::Xy(x, y) => write!(
                $out,
                concat!(r#"<g class="basic label-container outer-path" transform="translate({}"#, $xy_separator, r#"{})""#, $tail),
                fmt(x), fmt(y) $(, $arg)*
            ),
        }
    };
}

impl OuterPathTranslation {
    pub(super) fn write_hand_drawn(
        self,
        out: &mut impl SvgOutput,
        body: &mut NodeBodySvg<'_>,
        path: &str,
    ) {
        if let Some((fill_d, stroke_d)) = body.hand_drawn_path_pair(path) {
            let _ = write_outer_path_group!(
                out,
                self,
                ",",
                r#" style="{}"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                escape_attr(body.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(body.fill_color),
                escape_attr(&stroke_d),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
            );
        } else {
            let _ = write_outer_path_group!(out, self, ", ", ">");
            write_raw_filled_stroked_path(out, path, body);
            out.push_str("</g>");
        }
    }
}

/// A closed contour with the existing outer-path paint and fallback rules.
pub struct OuterPathBody {
    path: String,
    translation: OuterPathTranslation,
}

impl OuterPathBody {
    pub fn from_points(points: &[(f64, f64)], translation: OuterPathTranslation) -> Self {
        Self {
            path: closed_path_d_from_points(points),
            translation,
        }
    }

    pub fn bow_tie(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding_x = if neo { 16.0 } else { padding };
        let padding_y = if neo { 12.0 } else { padding };
        let w = label_width + 2.0 * padding_x;
        let h = label_height + padding_y;
        let ry = h / 2.0;
        let rx = ry / (2.5 + h / 50.0);

        let mut points: Vec<(f64, f64)> = Vec::new();
        points.push((w / 2.0, -h / 2.0));
        points.push((-w / 2.0, -h / 2.0));
        points.extend(mermaid_arc_points(
            -w / 2.0,
            -h / 2.0,
            -w / 2.0,
            h / 2.0,
            rx,
            ry,
            false,
        ));
        points.push((w / 2.0, h / 2.0));
        points.extend(mermaid_arc_points(
            w / 2.0,
            h / 2.0,
            w / 2.0,
            -h / 2.0,
            rx,
            ry,
            true,
        ));
        Self::from_points(&points, OuterPathTranslation::X(rx / 2.0))
    }

    pub fn notched_pentagon(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding_x = if neo { 16.0 } else { padding };
        let padding_y = if neo { 12.0 } else { padding };
        let w = label_width + 2.0 * padding_x;
        let h = label_height + 2.0 * padding_y;
        let points = vec![
            ((-w / 2.0) * 0.8, -h / 2.0),
            ((w / 2.0) * 0.8, -h / 2.0),
            (w / 2.0, (-h / 2.0) * 0.6),
            (w / 2.0, h / 2.0),
            (-w / 2.0, h / 2.0),
            (-w / 2.0, (-h / 2.0) * 0.6),
        ];
        Self::from_points(&points, OuterPathTranslation::None)
    }

    pub fn write(self, out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>) {
        if body.geometry.hand_drawn {
            self.translation
                .write_hand_drawn(out, &mut body, &self.path);
            return;
        }

        let (fill_d, stroke_d) = body
            .solid_path_pair(&self.path)
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
        let _ = write_outer_path_group!(
            out,
            self.translation,
            ",",
            r#"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"#,
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
        let _ = prepare_node_path(&geometry, &self.path, RoughGeometryTarget::Query);
    }
}
