use super::super::*;

#[derive(Debug, Clone)]
pub(super) struct SequenceActivationPlan<'a> {
    layouts: &'a [crate::model::SequenceActivationLayout],
    geometry: &'a crate::sequence::SequencePreparedActivationGeometry,
    fill: String,
    stroke: String,
    stroke_width: Option<f32>,
    radius: Option<f32>,
    pub(super) bounds: Option<Bounds>,
}

impl<'a> SequenceActivationPlan<'a> {
    pub(super) fn new(
        layouts: &'a [crate::model::SequenceActivationLayout],
        geometry: &'a crate::sequence::SequencePreparedActivationGeometry,
    ) -> Self {
        Self {
            layouts,
            geometry,
            fill: "#EDF2AE".to_owned(),
            stroke: "#666".to_owned(),
            stroke_width: None,
            radius: None,
            bounds: None,
        }
    }

    pub(super) fn prepare_paint(
        &mut self,
        stroke_width: Option<f32>,
        radius: Option<f32>,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        self.stroke_width = stroke_width;
        self.radius = radius;
        let Some(width) = stroke_width.map(f64::from) else {
            return Ok(());
        };
        for group in &self.geometry.groups {
            options.work_meter().charge(1)?;
            let Some(group) = group else { continue };
            let rect = &self.layouts[group.layout_index];
            let bounds = Bounds {
                min_x: rect.x - width / 2.0,
                min_y: rect.y - width / 2.0,
                max_x: rect.x + rect.width + width / 2.0,
                max_y: rect.y + rect.height + width / 2.0,
            };
            if let Some(total) = &mut self.bounds {
                total.min_x = total.min_x.min(bounds.min_x);
                total.min_y = total.min_y.min(bounds.min_y);
                total.max_x = total.max_x.max(bounds.max_x);
                total.max_y = total.max_y.max(bounds.max_y);
            } else {
                self.bounds = Some(bounds);
            }
        }
        Ok(())
    }
}

pub(super) fn render_sequence_activation_group(
    out: &mut impl SvgOutput,
    plan: &SequenceActivationPlan,
    message_id: &str,
    compat: &crate::sequence::SequenceCompatBinding,
) -> Result<()> {
    let Some(group_index) = plan.geometry.group_by_start_id.get(message_id).copied() else {
        return Ok(());
    };

    // Mermaid creates a `<g>` placeholder at ACTIVE_START time and inserts the
    // `<rect class="activation{0..2}">` once ACTIVE_END is encountered.
    out.push_str("<g>");
    if let Some(Some(a)) = plan.geometry.groups.get(group_index) {
        let layout = &plan.layouts[a.layout_index];
        let is_neo = compat.is_neo;
        let _ = write!(
            out,
            r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="{stroke}" width="{w}" height="{h}" class="activation{idx}"{look_attr}"##,
            x = fmt(layout.x),
            y = fmt(layout.y),
            w = fmt(layout.width),
            h = fmt(layout.height),
            idx = layout.depth % 3,
            fill = escape_xml(&plan.fill),
            stroke = escape_xml(&plan.stroke),
            look_attr = if is_neo { r#" data-look="neo""# } else { "" },
        );
        let style = activation_palette_style(compat, a.actor_index);
        if !style.is_empty() {
            let _ = write!(out, r#" style="{}""#, escape_attr(&style));
        }
        if let Some(width) = plan.stroke_width {
            let _ = write!(out, r#" stroke-width="{}""#, fmt(f64::from(width)));
        }
        if let Some(radius) = plan.radius {
            let _ = write!(out, r#" rx="{r}" ry="{r}""#, r = fmt(f64::from(radius)));
        }
        out.push_str("/>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn activation_palette_style(
    compat: &crate::sequence::SequenceCompatBinding,
    actor_index: usize,
) -> String {
    if !compat.actor_palette_mode() {
        return String::new();
    }
    let (stroke, fill) = compat.activation_palette(actor_index);
    let mut style = String::new();
    for (property, value) in [("stroke", stroke), ("fill", fill)] {
        if let Some(color) = value {
            let color = super::super::util::cssom_color_value(color);
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
