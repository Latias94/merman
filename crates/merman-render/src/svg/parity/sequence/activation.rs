use super::super::*;
use super::SequenceEmitCheckpoints;
use super::model::SequenceSvgModel;
use crate::sequence::SequenceStaticRectThemeReceipt;
use crate::sequence::sequence_activation_start_x;
use merman_core::diagrams::sequence::SequenceMessageKind;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
struct SequenceActivationStart {
    startx: f64,
    starty: f64,
    group_index: usize,
}

#[derive(Debug, Clone)]
struct SequenceActivationRect {
    startx: f64,
    starty: f64,
    width: f64,
    height: f64,
    class_idx: usize,
}

#[derive(Debug, Clone)]
pub(super) struct SequenceActivationPlan<'a> {
    groups: Vec<Option<SequenceActivationRect>>,
    group_by_start_id: FxHashMap<&'a str, usize>,
    rect_count: usize,
    fill: String,
    stroke: String,
    stroke_width: Option<f32>,
    radius: Option<f32>,
    effect: Option<crate::diagram_theme::SvgShadowEffect>,
    shadows: Vec<Option<(String, crate::diagram_theme::SvgFilterRegion)>>,
    pub(super) bounds: Option<Bounds>,
}

impl SequenceActivationPlan<'_> {
    pub(super) fn effect_count(&self) -> usize {
        self.shadows.iter().flatten().count()
    }

    pub(super) fn prepare_paint(
        &mut self,
        stroke_width: Option<f32>,
        radius: Option<f32>,
        effect: Option<crate::diagram_theme::SvgShadowEffect>,
        receipt: &mut SequenceStaticRectThemeReceipt,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        self.stroke_width = stroke_width;
        self.radius = radius;
        self.effect = effect;
        if stroke_width.is_none() && self.effect.is_none() {
            return Ok(());
        }
        if self.effect.is_some() {
            self.shadows.resize(self.groups.len(), None);
        }
        let width = f64::from(stroke_width.unwrap_or(1.0));
        for (index, rect) in self.groups.iter_mut().enumerate() {
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

    pub(super) const fn rect_count(&self) -> usize {
        self.rect_count
    }
}

pub(super) fn build_sequence_activation_plan<'a>(
    model: &'a SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    activation_width: f64,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<SequenceActivationPlan<'a>> {
    // Mermaid 11.15 draws activation rectangles through `svgDraw.getNoteRect()`, whose SVG
    // attributes are hard-coded; theme `activationBkgColor` is emitted in CSS but does not change
    // the rect `fill` attribute in the baseline SVGs.
    let fill = "#EDF2AE".to_string();
    let stroke = "#666".to_string();

    let mut last_line_y: Option<f64> = None;
    let mut activation_stacks: std::collections::BTreeMap<&str, Vec<SequenceActivationStart>> =
        std::collections::BTreeMap::new();
    let mut groups: Vec<Option<SequenceActivationRect>> = Vec::new();
    let mut rect_count = 0usize;
    let mut group_by_start_id: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());

    for (message_index, msg) in model.messages.iter().enumerate() {
        checkpoints.checkpoint_loop(message_index)?;
        if let Some(y) = msg_line_y(edges_by_id, &msg.id) {
            last_line_y = Some(y);
        }

        match msg.semantic_kind() {
            SequenceMessageKind::ActivationStart => {
                let Some(actor_id) = msg.from.as_deref() else {
                    continue;
                };
                let Some(cx) = actor_center_x(nodes_by_id, actor_id) else {
                    continue;
                };
                let has_any_activation = !activation_stacks.is_empty();
                let stack = activation_stacks.entry(actor_id).or_default();
                let stacked_size = stack.len();
                let startx = sequence_activation_start_x(cx, stacked_size, activation_width);

                let starty = last_line_y
                    .or_else(|| lifeline_y(edges_by_id, actor_id).map(|(y0, _y1)| y0))
                    .unwrap_or(0.0);
                let starty = if last_line_y.is_some() && has_any_activation {
                    starty + 2.0
                } else {
                    starty
                };

                let group_index = groups.len();
                groups.push(None);
                group_by_start_id.insert(msg.id.as_str(), group_index);
                stack.push(SequenceActivationStart {
                    startx,
                    starty,
                    group_index,
                });
            }
            SequenceMessageKind::ActivationEnd => {
                let Some(actor_id) = msg.from.as_deref() else {
                    continue;
                };
                let Some(stack) = activation_stacks.get_mut(actor_id) else {
                    continue;
                };
                let Some(start) = stack.pop() else {
                    continue;
                };

                let mut starty = start.starty;
                let mut vertical_pos = last_line_y.unwrap_or(starty);
                if starty + 18.0 > vertical_pos {
                    starty = vertical_pos - 6.0;
                    vertical_pos += 12.0;
                }

                let class_idx = stack.len() % 3;
                let rect = SequenceActivationRect {
                    startx: start.startx,
                    starty,
                    width: activation_width,
                    height: (vertical_pos - starty).max(0.0),
                    class_idx,
                };
                if let Some(slot) = groups.get_mut(start.group_index) {
                    *slot = Some(rect);
                    rect_count = rect_count.saturating_add(1);
                }
            }
            _ => {}
        }

        let _ = msg.activate;
    }

    checkpoints.checkpoint()?;
    Ok(SequenceActivationPlan {
        groups,
        group_by_start_id,
        rect_count,
        fill,
        stroke,
        stroke_width: None,
        radius: None,
        effect: None,
        shadows: Vec::new(),
        bounds: None,
    })
}

pub(super) fn render_sequence_activation_group(
    out: &mut impl SvgOutput,
    plan: &SequenceActivationPlan,
    message_id: &str,
    theme_receipt: &mut SequenceStaticRectThemeReceipt,
    evidence: &crate::diagram_theme::SvgShadowEvidenceRecorder,
) -> Result<()> {
    let Some(group_index) = plan.group_by_start_id.get(message_id).copied() else {
        return Ok(());
    };

    // Mermaid creates a `<g>` placeholder at ACTIVE_START time and inserts the
    // `<rect class="activation{0..2}">` once ACTIVE_END is encountered.
    out.push_str("<g>");
    if let Some(Some(a)) = plan.groups.get(group_index) {
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
            r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="{stroke}" width="{w}" height="{h}" class="activation{idx}""##,
            x = fmt(a.startx),
            y = fmt(a.starty),
            w = fmt(a.width),
            h = fmt(a.height),
            idx = a.class_idx,
            fill = escape_xml(&plan.fill),
            stroke = escape_xml(&plan.stroke),
        );
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

fn actor_center_x(nodes_by_id: &FxHashMap<&str, &LayoutNode>, actor_id: &str) -> Option<f64> {
    let node_id = format!("actor-top-{actor_id}");
    nodes_by_id.get(node_id.as_str()).copied().map(|n| n.x)
}

fn lifeline_y(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    actor_id: &str,
) -> Option<(f64, f64)> {
    let edge_id = format!("lifeline-{actor_id}");
    let e = edges_by_id.get(edge_id.as_str()).copied()?;
    let y0 = e.points.first()?.y;
    let y1 = e.points.last()?.y;
    Some((y0, y1))
}

fn msg_line_y(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    msg_id: &str,
) -> Option<f64> {
    let edge_id = format!("msg-{msg_id}");
    let e = edges_by_id.get(edge_id.as_str()).copied()?;
    Some(e.points.first()?.y)
}
