use super::{SequenceActorPalette, SequencePreparedActivationGeometry};
use crate::model::SequenceActivationLayout;
use crate::rect_paint::include_rect_stroke_bounds;
use merman_render_core::{
    Bounds, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    svg_output::SvgOutput,
    theme_svg::cssom_color_value,
    xml::{escape_attr, escape_xml},
};
use std::fmt::Write as _;

#[derive(Debug, Clone)]
pub struct SequenceActivationPlan<'a> {
    layouts: &'a [SequenceActivationLayout],
    geometry: &'a SequencePreparedActivationGeometry,
    palette: &'a SequenceActorPalette,
    is_neo: bool,
    fill: String,
    stroke: String,
    stroke_width: Option<f32>,
    radius: Option<f32>,
    bounds: Option<Bounds>,
}

impl<'a> SequenceActivationPlan<'a> {
    pub fn prepare(
        layouts: &'a [SequenceActivationLayout],
        geometry: &'a SequencePreparedActivationGeometry,
        stroke_width: Option<f32>,
        radius: Option<f32>,
        is_neo: bool,
        palette: &'a SequenceActorPalette,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self> {
        let mut plan = Self {
            layouts,
            geometry,
            palette,
            is_neo,
            fill: "#EDF2AE".to_owned(),
            stroke: "#666".to_owned(),
            stroke_width,
            radius,
            bounds: None,
        };
        if let Some(width) = stroke_width.map(f64::from) {
            for group in &geometry.groups {
                work_meter.charge(1)?;
                let Some(group) = group else { continue };
                let rect = &layouts[group.layout_index];
                include_rect_stroke_bounds(
                    &mut plan.bounds,
                    [rect.x, rect.y, rect.width, rect.height],
                    width,
                );
            }
        }
        Ok(plan)
    }

    pub const fn bounds(&self) -> Option<&Bounds> {
        self.bounds.as_ref()
    }

    pub fn write_group(&self, out: &mut impl SvgOutput, message_id: &str) -> Result<()> {
        let Some(group) = self.geometry.group_for_start(message_id) else {
            return Ok(());
        };

        // Mermaid creates a `<g>` placeholder at ACTIVE_START time and inserts the
        // `<rect class="activation{0..2}">` once ACTIVE_END is encountered.
        out.push_str("<g>");
        if let Some(group) = group {
            let layout = &self.layouts[group.layout_index];
            let _ = write!(
                out,
                r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="{stroke}" width="{w}" height="{h}" class="activation{idx}"{look_attr}"##,
                x = fmt(layout.x),
                y = fmt(layout.y),
                w = fmt(layout.width),
                h = fmt(layout.height),
                idx = layout.depth % 3,
                fill = escape_xml(&self.fill),
                stroke = escape_xml(&self.stroke),
                look_attr = if self.is_neo {
                    r#" data-look="neo""#
                } else {
                    ""
                },
            );
            let style = activation_palette_style(self.palette, group.actor_index);
            if !style.is_empty() {
                let _ = write!(out, r#" style="{}""#, escape_attr(&style));
            }
            if let Some(width) = self.stroke_width {
                let _ = write!(out, r#" stroke-width="{}""#, fmt(f64::from(width)));
            }
            if let Some(radius) = self.radius {
                let _ = write!(out, r#" rx="{r}" ry="{r}""#, r = fmt(f64::from(radius)));
            }
            out.push_str("/>");
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }
}

fn activation_palette_style(palette: &SequenceActorPalette, actor_index: usize) -> String {
    let (stroke, fill) = palette.activation_colors(actor_index);
    let mut style = String::new();
    for (property, value) in [("stroke", stroke), ("fill", fill)] {
        if let Some(color) = value {
            let color = cssom_color_value(color);
            if color.is_empty() {
                continue;
            }
            if !style.is_empty() {
                style.push(' ');
            }
            let _ = write!(style, "{property}: {color};");
        }
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activation::SequenceActivationGeometryBuilder;
    use merman_render_core::resources::RenderResourcePolicy;

    #[test]
    fn activation_writer_keeps_empty_groups_and_scopes_palette_to_the_completed_bar() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut builder = SequenceActivationGeometryBuilder::new(10.0);
        builder.start("open", "A", 0, 50.0, 100.0);
        builder.start("closed", "B", 1, 100.0, 100.0);
        builder.end(1, 150.0).unwrap();
        let (layouts, geometry) = builder.finish(&work).unwrap();
        let palette = SequenceActorPalette::resolve(&serde_json::json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": [null, "red"],
                "bkgColorArray": ["blue", null],
                "mainBkg": "white"
            }
        }));
        let plan = SequenceActivationPlan::prepare(
            &layouts,
            &geometry,
            Some(2.0),
            Some(3.0),
            true,
            &palette,
            &work,
        )
        .unwrap();
        assert_eq!(work.used(), 2);
        let bounds = plan.bounds().unwrap();
        assert_eq!(
            (bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
            (94.0, 101.0, 106.0, 151.0)
        );

        let mut svg = String::new();
        plan.write_group(&mut svg, "unknown").unwrap();
        plan.write_group(&mut svg, "open").unwrap();
        plan.write_group(&mut svg, "closed").unwrap();
        assert_eq!(
            svg,
            r##"<g></g><g><rect x="95" y="102" fill="#EDF2AE" stroke="#666" width="10" height="48" class="activation0" data-look="neo" style="stroke: red; fill: white;" stroke-width="2" rx="3" ry="3"/></g>"##
        );
    }
}
