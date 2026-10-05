use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::math_label::{
    record_sequence_katex_terminal_emission, sequence_katex_label,
    write_sequence_katex_foreign_object,
};
use crate::sequence::{
    SequenceDrawnTextNode, SequenceMathHeightMode, SequenceStaticRectThemeReceipt,
    measure_sequence_drawn_line_height, sequence_drawn_text_first_y, sequence_drawn_text_style,
    sequence_drawn_text_y, sequence_note_final_wrapped_lines,
};
use merman_core::diagrams::sequence::{SequenceMessage, SequenceMessageKind};
use rustc_hash::FxHashMap;

use crate::diagram_theme::{SvgFilterRegion, SvgShadowEffect, SvgShadowEvidenceRecorder};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct SequenceNotePaintPlan {
    stroke_width: Option<f32>,
    radius: Option<f32>,
    effect: Option<SvgShadowEffect>,
    shadows: BTreeMap<String, (String, SvgFilterRegion)>,
    pub(super) bounds: Option<Bounds>,
}

impl SequenceNotePaintPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        model: &super::model::SequenceSvgModel,
        nodes: &FxHashMap<&str, &LayoutNode>,
        stroke_width: Option<f32>,
        radius: Option<f32>,
        effect: Option<SvgShadowEffect>,
        receipt: &mut SequenceStaticRectThemeReceipt,
        options: &SvgExecution<'_>,
    ) -> Result<Self> {
        let mut plan = Self {
            stroke_width,
            radius,
            effect,
            ..Self::default()
        };
        if plan.stroke_width.is_none() && plan.effect.is_none() {
            return Ok(plan);
        }
        // Notes have the SVG 1px default; actor strokeWidth does not apply here.
        let width = f64::from(stroke_width.unwrap_or(1.0));
        for (index, message) in model.messages.iter().enumerate() {
            options.work_meter().charge(1)?;
            if message.semantic_kind() != SequenceMessageKind::Note {
                continue;
            }
            let node_id = format!("note-{}", message.id);
            let Some(node) = nodes.get(node_id.as_str()) else {
                continue;
            };
            let (left, top) = node_left_top(node);
            let mut bounds = Bounds {
                min_x: left - width / 2.0,
                min_y: top - width / 2.0,
                max_x: left + node.width + width / 2.0,
                max_y: top + node.height + width / 2.0,
            };
            if let Some(effect) = &plan.effect {
                options
                    .work_meter()
                    .charge(effect.stages().len().saturating_mul(3))?;
                if let Some(materialized) = effect.materialize_rect(
                    &options.theme_resource_policy(),
                    node.width,
                    node.height,
                    width,
                )? {
                    let region = materialized.region();
                    let [x, y, w, h] = region.as_array().map(f64::from);
                    // Use the same outward-rounded region as the filter writer.
                    bounds = Bounds {
                        min_x: left + x * node.width,
                        min_y: top + y * node.height,
                        max_x: left + (x + w) * node.width,
                        max_y: top + (y + h) * node.height,
                    };
                    plan.shadows.insert(
                        node_id,
                        (
                            format!(
                                "{}-note-{index}-theme-effect-{}",
                                options.diagram_id_or("merman"),
                                effect.id()
                            ),
                            region,
                        ),
                    );
                } else {
                    receipt.effect_unhandled = true;
                }
            }
            if let Some(total) = &mut plan.bounds {
                total.min_x = total.min_x.min(bounds.min_x);
                total.min_y = total.min_y.min(bounds.min_y);
                total.max_x = total.max_x.max(bounds.max_x);
                total.max_y = total.max_y.max(bounds.max_y);
            } else {
                plan.bounds = Some(bounds);
            }
        }
        Ok(plan)
    }

    pub(super) fn len(&self) -> usize {
        self.shadows.len()
    }
}

pub(super) struct SequenceNoteRenderContext<'a> {
    pub(super) text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) paint: &'a SequenceNotePaintPlan,
    pub(super) shadow_evidence: &'a SvgShadowEvidenceRecorder,
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) note_margin: f64,
    pub(super) wrap_padding: f64,
    pub(super) note_text_style: &'a TextStyle,
    pub(super) note_typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) sanitize_config: &'a merman_core::MermaidConfig,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) fn render_sequence_note(
    out: &mut impl SvgOutput,
    message_index: usize,
    msg: &SequenceMessage,
    ctx: &SequenceNoteRenderContext<'_>,
    theme_receipt: &mut SequenceStaticRectThemeReceipt,
) -> Result<()> {
    if msg.semantic_kind() != SequenceMessageKind::Note {
        return Ok(());
    }
    ctx.checkpoints.checkpoint()?;

    let id = &msg.id;
    let raw = msg.message_text();
    let node_id = format!("note-{id}");
    let Some(n) = ctx.nodes_by_id.get(node_id.as_str()).copied() else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::NoteLabel);
        return Ok(());
    };
    let (x, y) = node_left_top(n);
    let cx = x + (n.width / 2.0);
    let text_y = sequence_drawn_text_first_y(y, ctx.note_margin);
    let shadow = ctx
        .paint
        .shadows
        .get(&node_id)
        .zip(ctx.paint.effect.as_ref());
    let filter = shadow.map(|((id, region), effect)| {
        super::super::shadow::write_theme_shadow_application(out, id, effect, *region)
    });
    let _ = write!(out, r#"<g data-et="note" data-id="i{}">"#, escape_attr(id));
    let _ = write!(
        &mut *out,
        r##"<rect x="{x}" y="{y}" fill="#EDF2AE" stroke="#666" width="{w}" height="{h}" class="note"{look_attr}"##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height),
        look_attr = if crate::config::config_diagram_look(ctx.sanitize_config.as_value()).as_str()
            == "neo"
        {
            r#" data-look="neo""#
        } else {
            ""
        },
    );
    if let Some(width) = ctx.paint.stroke_width {
        let _ = write!(out, r#" stroke-width="{}""#, fmt(f64::from(width)));
    }
    if let Some(radius) = ctx.paint.radius {
        let _ = write!(out, r#" rx="{r}" ry="{r}""#, r = fmt(f64::from(radius)));
    }
    if let Some(filter) = &filter {
        let _ = write!(
            out,
            r#" filter="{filter}" style="filter:{filter};""#,
            filter = escape_attr(filter)
        );
    } else if theme_receipt.effect_cleared {
        out.push_str(r#" style="filter:none;""#);
    }
    out.push_str("/>");
    theme_receipt.record_rect_emission();
    if let Some(((id, region), effect)) = shadow {
        ctx.shadow_evidence.record_application(effect, id, *region);
        theme_receipt.record_effect_emission();
    } else if theme_receipt.effect_cleared {
        theme_receipt.record_effect_emission();
    }
    let prepared_math =
        ctx.math_sidecar
            .terminal_for_occurrence(&crate::sequence::SequenceMathOccurrence::Note(
                message_index,
            ));
    if let Some(katex) = sequence_katex_label(
        prepared_math,
        ctx.note_text_style,
        SequenceMathHeightMode::Draw,
    ) {
        write_sequence_katex_foreign_object(
            out,
            &katex,
            (x + n.width / 2.0 - katex.width / 2.0).round(),
            (y + n.height / 2.0 - katex.height / 2.0).round(),
        );
        record_sequence_katex_terminal_emission(
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::NoteLabel,
            &katex,
        );
        ctx.text_shadow.record_terminal(
            None,
            false,
            ctx.shadow_evidence,
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::NoteLabel,
        );
    } else if msg.wrap {
        // Mermaid@11.12.2 (Sequence) wraps notes *after* placement width is known:
        //   noteModel.message = wrapLabel(msg.message, noteModel.width - 2*wrapPadding, noteFont)
        //
        // Layout already computed the note box width (`n.width`) to match Mermaid's
        // `noteModel.width`, so wrap to `n.width - 2*wrapPadding` here.
        let lines = sequence_note_final_wrapped_lines(
            raw,
            n.width,
            2.0 * ctx.wrap_padding,
            ctx.measurer,
            ctx.note_text_style,
            ctx.checkpoints.text(),
        )?;
        render_sequence_note_lines(out, lines.iter().map(String::as_str), cx, text_y, ctx)?;
    } else {
        render_sequence_note_lines(out, crate::text::split_html_br_lines(raw), cx, text_y, ctx)?;
    }
    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}

fn render_sequence_note_lines<'a>(
    out: &mut impl SvgOutput,
    lines: impl IntoIterator<Item = &'a str>,
    cx: f64,
    text_y: f64,
    ctx: &SequenceNoteRenderContext<'_>,
) -> Result<()> {
    let drawn_style = sequence_drawn_text_style(ctx.note_text_style, ctx.sanitize_config);
    let css = super::settings::sequence_text_style_attribute(ctx.note_text_style);
    let mut preceding_height = 0.0;
    for (i, line) in lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(line);
        let text = if decoded.as_ref().is_empty() {
            "\u{200B}"
        } else {
            decoded.as_ref()
        };
        let y = sequence_drawn_text_y(text_y, ctx.note_margin, preceding_height);
        let style = ctx.note_typography.terminal_style("", css.clone());
        // SVG whitespace and the zero-width placeholder have no painted glyphs.
        // Keep the line for layout, but do not create an empty native filter group.
        let paintless = ctx.text_shadow.is_paintless(text);
        let shadow = Some(ctx)
            .filter(|_| !paintless)
            .map(|ctx| {
                ctx.text_shadow.write_definition(
                    out,
                    text,
                    cx,
                    y,
                    super::text_effect::TextShadowBaseline::NoteMiddle,
                    ctx.note_typography.terminal_text_style(),
                    ctx.measurer,
                )
            })
            .transpose()?
            .flatten();
        let filter = shadow
            .as_ref()
            .map(|shadow| format!(" filter=\"{}\"", escape_attr(&shadow.filter)))
            .unwrap_or_default();
        let _ = write!(
            &mut *out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="noteText" dy="1em" style="{style}"{filter}><tspan x="{x}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(y),
            style = escape_attr_display(&style),
            text = escape_xml(text)
        );
        ctx.typography_receipt
            .record_terminal_text(crate::sequence::SequenceTextSurface::NoteLabel);
        out.checkpoint()?;
        ctx.text_shadow.record_terminal(
            shadow.as_ref(),
            paintless,
            ctx.shadow_evidence,
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::NoteLabel,
        );
        if ctx.note_margin > 0.0 {
            preceding_height += measure_sequence_drawn_line_height(
                ctx.measurer,
                text,
                &drawn_style,
                SequenceDrawnTextNode::Tspan,
                ctx.checkpoints.text(),
            )?;
        }
    }
    ctx.checkpoints.checkpoint()
}

#[cfg(test)]
mod tests {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    #[test]
    fn empty_note_rows_render_the_upstream_zero_width_space() {
        crate::svg::parity::with_test_svg_execution(
            crate::DiagramFamilyId::SEQUENCE,
            &crate::svg::SvgRenderOptions::default(),
            |execution| {
                let mut out = String::new();
                let meter =
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
                let config = merman_core::MermaidConfig::default();
                let typography =
                    crate::sequence::SequenceTypographyPlan::resolve(&config, None, &meter)
                        .unwrap();
                let mut receipt =
                    crate::sequence::SequenceTypographyThemeReceipt::from_plan(&typography);
                let text_shadow = super::super::text_effect::SequenceTextShadow::resolve(
                    execution,
                    crate::sequence::SequenceTypographyRole::Note,
                    typography.note(),
                    &mut receipt,
                );
                super::render_sequence_note_lines(
                    &mut out,
                    ["first", "", "last"],
                    50.0,
                    10.0,
                    &super::SequenceNoteRenderContext {
                        text_shadow: &text_shadow,
                        paint: &Default::default(),
                        shadow_evidence: &Default::default(),
                        note_typography: typography.note(),
                        typography_receipt: &receipt,
                        math_sidecar: &Default::default(),
                        nodes_by_id: &Default::default(),
                        measurer: &crate::text::DeterministicTextMeasurer::default(),
                        note_margin: 10.0,
                        wrap_padding: 10.0,
                        note_text_style: &crate::text::TextStyle::default(),
                        sanitize_config: &merman_core::MermaidConfig::default(),
                        checkpoints: super::SequenceEmitCheckpoints::for_emit(&meter),
                    },
                )
                .unwrap();

                assert!(out.contains("<tspan x=\"50\">\u{200b}</tspan>"), "{out}");
            },
        );
    }
    #[test]
    fn note_rows_use_tspan_heights_and_keep_explicit_dy_at_nonpositive_margins() {
        crate::svg::parity::with_test_svg_execution(
            crate::DiagramFamilyId::SEQUENCE,
            &crate::svg::SvgRenderOptions::default(),
            |execution| {
                struct TspanProbe(std::cell::Cell<usize>);
                impl crate::text::TextMeasurer for TspanProbe {
                    fn measure(
                        &self,
                        _: &str,
                        _: &crate::text::TextStyle,
                    ) -> crate::text::TextMetrics {
                        panic!("notes must use the tspan height operation")
                    }
                    fn measure_svg_tspan_text_bbox_height_px(
                        &self,
                        text: &str,
                        style: &crate::text::TextStyle,
                    ) -> f64 {
                        assert_eq!(style.font_weight.as_deref(), Some("700"));
                        self.0.set(self.0.get() + 1);
                        match text {
                            "first" => 10.4,
                            "\u{200b}" => 20.4,
                            "&" => 8.0,
                            _ => panic!("unexpected row"),
                        }
                    }
                }
                let meter =
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
                let config = merman_core::MermaidConfig::default();
                let typography =
                    crate::sequence::SequenceTypographyPlan::resolve(&config, None, &meter)
                        .unwrap();
                let mut receipt =
                    crate::sequence::SequenceTypographyThemeReceipt::from_plan(&typography);
                let text_shadow = super::super::text_effect::SequenceTextShadow::resolve(
                    execution,
                    crate::sequence::SequenceTypographyRole::Note,
                    typography.note(),
                    &mut receipt,
                );
                let settings =
                    super::super::settings::SequenceRenderSettings::from_effective_config(
                        &serde_json::json!({"sequence": {"noteFontWeight": 700}}),
                    );
                for margin in [5.0, 0.0, -5.0] {
                    let probe = TspanProbe(std::cell::Cell::new(0));
                    let mut out = String::new();
                    super::render_sequence_note_lines(
                        &mut out,
                        ["first", "", "#38;"],
                        50.0,
                        crate::sequence::sequence_drawn_text_first_y(10.25, margin),
                        &super::SequenceNoteRenderContext {
                            text_shadow: &text_shadow,
                            paint: &Default::default(),
                            shadow_evidence: &Default::default(),
                            note_typography: typography.note(),
                            typography_receipt: &receipt,
                            math_sidecar: &Default::default(),
                            nodes_by_id: &Default::default(),
                            measurer: &probe,
                            note_margin: margin,
                            wrap_padding: 10.0,
                            note_text_style: &settings.note_text_style,
                            sanitize_config: &merman_core::MermaidConfig::default(),
                            checkpoints: super::SequenceEmitCheckpoints::for_emit(&meter),
                        },
                    )
                    .unwrap();
                    let svg = format!("<svg>{out}</svg>");
                    let document = roxmltree::Document::parse(&svg).unwrap();
                    let ys: Vec<_> = document
                        .descendants()
                        .filter(|n| n.has_tag_name("text"))
                        .map(|n| n.attribute("y").unwrap())
                        .collect();
                    assert_eq!(
                        ys,
                        if margin > 0.0 {
                            vec!["13", "23", "44"]
                        } else {
                            vec!["10.25"; 3]
                        }
                    );
                    for node in document
                        .descendants()
                        .filter(|node| node.has_tag_name("text"))
                    {
                        assert!(
                            node.attribute("style")
                                .unwrap()
                                .contains("font-weight: 700;")
                        );
                    }
                    assert_eq!(out.matches("dy=\"1em\"").count(), 3);
                    assert!(out.contains("&amp;</tspan>"));
                    assert_eq!(probe.0.get(), if margin > 0.0 { 3 } else { 0 });
                }
            },
        );
    }
}
