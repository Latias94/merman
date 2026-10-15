use super::super::*;
use crate::sequence::SequenceStaticRectThemeReceipt;

#[derive(Debug, Clone)]
pub(super) struct SequenceActivationPlan<'a> {
    geometry: &'a crate::sequence::SequencePreparedActivationGeometry,
    fill: String,
    stroke: String,
    typed_fill: bool,
    typed_stroke: bool,
    stroke_width: Option<f32>,
    radius: Option<f32>,
    effect: Option<crate::diagram_theme::SvgShadowEffect>,
    shadows: Vec<Option<(String, crate::diagram_theme::SvgFilterRegion)>>,
    pub(super) bounds: Option<Bounds>,
}

impl<'a> SequenceActivationPlan<'a> {
    pub(super) fn new(geometry: &'a crate::sequence::SequencePreparedActivationGeometry) -> Self {
        Self {
            geometry,
            fill: "#EDF2AE".to_owned(),
            stroke: "#666".to_owned(),
            typed_fill: false,
            typed_stroke: false,
            stroke_width: None,
            radius: None,
            effect: None,
            shadows: Vec::new(),
            bounds: None,
        }
    }

    pub(super) fn effect_count(&self) -> usize {
        self.shadows.iter().flatten().count()
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
    )]
    pub(super) fn prepare_paint(
        &mut self,
        typed_fill: bool,
        typed_stroke: bool,
        stroke_width: Option<f32>,
        radius: Option<f32>,
        effect: Option<crate::diagram_theme::SvgShadowEffect>,
        receipt: &mut SequenceStaticRectThemeReceipt,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        self.typed_fill = typed_fill;
        self.typed_stroke = typed_stroke;
        self.stroke_width = stroke_width;
        self.radius = radius;
        self.effect = effect;
        if stroke_width.is_none() && self.effect.is_none() {
            return Ok(());
        }
        if self.effect.is_some() {
            self.shadows.resize(self.geometry.groups.len(), None);
        }
        let width = f64::from(stroke_width.unwrap_or(1.0));
        for (index, rect) in self.geometry.groups.iter().enumerate() {
            options.work_meter().charge(1)?;
            let Some(rect) = rect else { continue };
            let mut bounds = Bounds {
                min_x: rect.startx - width / 2.0,
                min_y: rect.starty - width / 2.0,
                max_x: rect.startx + rect.width + width / 2.0,
                max_y: rect.starty + rect.height + width / 2.0,
            };
            if let Some(effect) = &self.effect {
                options
                    .work_meter()
                    .charge(effect.stages().len().saturating_mul(3))?;
                if let Some(shadow) = effect.materialize_rect(
                    &options.theme_resource_policy(),
                    rect.width,
                    rect.height,
                    width,
                )? {
                    let region = shadow.region();
                    let [x, y, w, h] = region.as_array().map(f64::from);
                    bounds = Bounds {
                        min_x: rect.startx + x * rect.width,
                        min_y: rect.starty + y * rect.height,
                        max_x: rect.startx + (x + w) * rect.width,
                        max_y: rect.starty + (y + h) * rect.height,
                    };
                    self.shadows[index] = Some((
                        format!(
                            "{}-activation-{index}-theme-effect-{}",
                            options.diagram_id_or("merman"),
                            effect.id()
                        ),
                        region,
                    ));
                } else {
                    receipt.effect_unhandled = true;
                }
            }
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
    theme_receipt: &mut SequenceStaticRectThemeReceipt,
    evidence: &crate::diagram_theme::SvgShadowEvidenceRecorder,
) -> Result<()> {
    let Some(group_index) = plan.geometry.group_by_start_id.get(message_id).copied() else {
        return Ok(());
    };

    // Mermaid creates a `<g>` placeholder at ACTIVE_START time and inserts the
    // `<rect class="activation{0..2}">` once ACTIVE_END is encountered.
    out.push_str("<g>");
    if let Some(Some(a)) = plan.geometry.groups.get(group_index) {
        let is_neo = compat.is_neo;
        let shadow = plan
            .shadows
            .get(group_index)
            .and_then(Option::as_ref)
            .zip(plan.effect.as_ref());
        let filter = shadow.map(|((id, region), effect)| {
            super::super::shadow::write_theme_shadow_application(out, id, effect, *region)
        });
        let _ = write!(
            out,
            r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="{stroke}" width="{w}" height="{h}" class="activation{idx}"{look_attr}"##,
            x = fmt(a.startx),
            y = fmt(a.starty),
            w = fmt(a.width),
            h = fmt(a.height),
            idx = a.class_idx,
            fill = escape_xml(&plan.fill),
            stroke = escape_xml(&plan.stroke),
            look_attr = if is_neo { r#" data-look="neo""# } else { "" },
        );
        let style =
            activation_palette_style(compat, a.actor_index, plan.typed_fill, plan.typed_stroke);
        if !style.is_empty() {
            let _ = write!(out, r#" style="{}""#, escape_attr(&style));
        }
        if let Some(width) = plan.stroke_width {
            let _ = write!(out, r#" stroke-width="{}""#, fmt(f64::from(width)));
        }
        if let Some(radius) = plan.radius {
            let _ = write!(out, r#" rx="{r}" ry="{r}""#, r = fmt(f64::from(radius)));
        }
        if let Some(filter) = filter {
            let _ = write!(out, r#" filter="{}""#, escape_attr(&filter));
        }
        out.push_str("/>");
        out.checkpoint()?;
        theme_receipt.record_rect_emission();
        if let Some(((id, region), effect)) = shadow {
            evidence.record_application(effect, id, *region);
            theme_receipt.record_effect_emission();
        } else if theme_receipt.effect_cleared {
            theme_receipt.record_effect_emission();
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn activation_palette_style(
    compat: &crate::sequence::SequenceCompatBinding,
    actor_index: usize,
    typed_fill: bool,
    typed_stroke: bool,
) -> String {
    if !compat.actor_palette_mode() {
        return String::new();
    }
    let (stroke, fill) = compat.activation_palette(actor_index);
    let mut style = String::new();
    for (property, value) in [("stroke", stroke), ("fill", fill)] {
        if (property == "fill" && typed_fill) || (property == "stroke" && typed_stroke) {
            continue;
        }
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
