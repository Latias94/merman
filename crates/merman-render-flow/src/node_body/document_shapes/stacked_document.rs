//! Flowchart v2 stacked document.

use merman_render_core::{
    number_format::canonical_number as fmt, rough_svg::paths::RoughGeometryTarget,
    svg_output::SvgOutput, xml::escape_xml_display,
};

use super::super::{NodeBodyGeometry, NodeBodySvg, prepare_node_path};
use super::path_from_points;

pub struct StackedDocumentBody {
    group_dy: f64,
    outer_path: String,
    inner_path: String,
}

impl StackedDocumentBody {
    pub fn from_points(group_dy: f64, outer: &[(f64, f64)], inner: &[(f64, f64)]) -> Self {
        Self {
            group_dy,
            outer_path: path_from_points(outer),
            inner_path: path_from_points(inner),
        }
    }

    pub fn write(self, out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>) {
        if body.geometry.hand_drawn {
            let _ = write!(
                out,
                r#"<g class="basic label-container outer-path" transform="translate(0,{})" style="{}">"#,
                fmt(self.group_dy),
                escape_xml_display(body.rough_group_style),
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="basic label-container outer-path" transform="translate(0,{})">"#,
                fmt(self.group_dy),
            );
        }
        // Each layer generates its Rough paths after its opening group is written.
        write_layer(out, &mut body, &self.outer_path);
        out.push_str("<g>");
        write_layer(out, &mut body, &self.inner_path);
        out.push_str("</g></g>");
    }

    pub fn advance_rough_geometry(self, geometry: NodeBodyGeometry<'_>) {
        let target = RoughGeometryTarget::Query;
        for path in [&self.outer_path, &self.inner_path] {
            let _ = prepare_node_path(&geometry, path, target);
        }
    }
}

fn write_layer(out: &mut impl SvgOutput, body: &mut NodeBodySvg<'_>, path: &str) {
    let paths = if body.geometry.hand_drawn {
        body.hand_drawn_path_pair(path)
    } else {
        body.solid_path_pair(path)
    };
    if let Some((fill_d, stroke_d)) = paths {
        if body.geometry.hand_drawn {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_xml_display(&fill_d),
                escape_xml_display(body.fill_color),
                escape_xml_display(&stroke_d),
                escape_xml_display(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_xml_display(body.geometry.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                escape_xml_display(&fill_d),
                escape_xml_display(body.fill_color),
                escape_xml_display(body.style),
                escape_xml_display(&stroke_d),
                escape_xml_display(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_xml_display(body.geometry.stroke_dasharray),
                escape_xml_display(body.style),
            );
        }
    } else if body.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/>"#,
            escape_xml_display(path),
            escape_xml_display(body.fill_color),
            escape_xml_display(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_xml_display(body.style),
        );
    }
}
