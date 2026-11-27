use super::SequenceEmitCheckpoints;
use super::math_label::{sequence_katex_label, write_sequence_katex_foreign_object};
use super::prelude::*;
use crate::preparation::{
    SequenceDrawnTextNode, SequenceMathHeightMode, bracketize_sequence_block_label,
    measure_sequence_drawn_line_height, sequence_drawn_text_first_y, sequence_drawn_text_style,
    sequence_drawn_text_y,
};
use merman_render_core::text::line_break::split_mermaid_common_br_lines;

pub(super) struct LoopTextRenderContext<'a> {
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) style: &'a TextStyle,
    pub(super) config: &'a merman_model::presentation::PresentationConfig,
    pub(super) margin: f64,
    pub(super) typography: &'a crate::preparation::SequenceResolvedTypography,
    pub(super) math_sidecar: &'a crate::preparation::SequenceMathSidecar,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) struct LoopTextPlacement {
    pub(super) x: f64,
    pub(super) math_center_x: f64,
    pub(super) y0: f64,
    pub(super) block_start_y: f64,
    pub(super) max_width: Option<f64>,
    pub(super) use_tspan: bool,
}

impl<'a> LoopTextRenderContext<'a> {
    fn katex_label(
        &self,
        occurrence_id: &str,
    ) -> Option<super::math_label::SequenceKatexLabel<'_>> {
        let prepared = self.math_sidecar.terminal_for_occurrence(
            &crate::preparation::SequenceMathOccurrence::BlockLabel(occurrence_id.to_owned()),
        );
        sequence_katex_label(prepared, self.style, SequenceMathHeightMode::Draw)
    }
}

pub(super) fn display_block_label(raw_label: &str, always_show: bool) -> Option<String> {
    // Keep entity spelling intact through wrapping and break splitting.
    let t = raw_label.trim();
    if t.is_empty() {
        if always_show {
            // Mermaid renders empty block labels as a zero-width space inside `<tspan>`.
            Some("\u{200B}".to_string())
        } else {
            None
        }
    } else {
        Some(bracketize_sequence_block_label(t))
    }
}

pub(super) fn wrap_svg_text_lines(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: Option<f64>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<Vec<String>> {
    let lines = if let Some(width) = max_width {
        crate::preparation::wrap_sequence_label_like_mermaid_lines(
            text,
            measurer,
            style,
            width,
            checkpoints.text(),
        )?
    } else {
        let split_lines = split_mermaid_common_br_lines(text);
        let mut lines = Vec::with_capacity(split_lines.len());
        for line in split_lines {
            checkpoints.checkpoint()?;
            lines.push(line.to_string());
        }
        lines
    };
    if lines.is_empty() {
        Ok(vec!["".to_string()])
    } else {
        Ok(lines)
    }
}

pub(super) fn write_loop_text_lines(
    out: &mut impl SvgOutput,
    ctx: &LoopTextRenderContext<'_>,
    placement: LoopTextPlacement,
    occurrence_id: &str,
    text: &str,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    if let Some(katex) = ctx.katex_label(occurrence_id) {
        // Mermaid's drawKatex centers on the complete frame, independently of prose placement.
        let x = (placement.math_center_x - katex.width / 2.0).round();
        write_sequence_katex_foreign_object(out, &katex, x, placement.block_start_y.round());
        return ctx.checkpoints.checkpoint();
    }

    let drawn_style = sequence_drawn_text_style(ctx.style, ctx.config);
    let css = super::settings::sequence_text_style_attribute(ctx.style);
    let mut preceding_height = 0.0;
    let lines = wrap_svg_text_lines(
        text,
        ctx.measurer,
        ctx.style,
        placement.max_width,
        ctx.checkpoints,
    )?;
    for (i, line) in lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let first_y = sequence_drawn_text_first_y(placement.y0, ctx.margin);
        let y = sequence_drawn_text_y(first_y, ctx.margin, preceding_height);
        let dy = block_text_dy(i, ctx.margin, ctx.style.font_size);
        let prepared = crate::preparation::prepare_sequence_text_line(&line);
        let line = if prepared.is_empty() {
            "\u{200b}"
        } else {
            prepared.as_ref()
        };
        let style = ctx.typography.terminal_style("", css.clone());
        if placement.use_tspan {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText"{dy} style="{style}"><tspan x="{x}">{text}</tspan></text>"#,
                x = fmt(placement.x),
                y = fmt(y),
                style = escape_attr_display(&style),
                text = super::escape_prepared_text(line)
            );
        } else {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText"{dy} style="{style}">{text}</text>"#,
                x = fmt(placement.x),
                y = fmt(y),
                style = escape_attr_display(&style),
                text = super::escape_prepared_text(line)
            );
        }
        out.checkpoint()?;
        if ctx.margin > 0.0 {
            let node = if placement.use_tspan {
                SequenceDrawnTextNode::Tspan
            } else {
                SequenceDrawnTextNode::Direct
            };
            preceding_height += measure_sequence_drawn_line_height(
                ctx.measurer,
                line,
                &drawn_style,
                node,
                ctx.checkpoints.text(),
            )?;
        }
    }
    ctx.checkpoints.checkpoint()
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
pub(super) fn write_section_title_lines(
    out: &mut impl SvgOutput,
    ctx: &LoopTextRenderContext<'_>,
    x: f64,
    y0: f64,
    section_start_y: f64,
    max_width: Option<f64>,
    occurrence_id: &str,
    text: &str,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    if let Some(katex) = ctx.katex_label(occurrence_id) {
        let x = (x - katex.width / 2.0).round();
        let y = (section_start_y - katex.height).round();
        write_sequence_katex_foreign_object(out, &katex, x, y);
        return ctx.checkpoints.checkpoint();
    }

    let drawn_style = sequence_drawn_text_style(ctx.style, ctx.config);
    let css = super::settings::sequence_text_style_attribute(ctx.style);
    let mut preceding_height = 0.0;
    let lines = wrap_svg_text_lines(text, ctx.measurer, ctx.style, max_width, ctx.checkpoints)?;
    for (i, line) in lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let first_y = sequence_drawn_text_first_y(y0, ctx.margin);
        let y = sequence_drawn_text_y(first_y, ctx.margin, preceding_height);
        let dy = block_text_dy(i, ctx.margin, ctx.style.font_size);
        let prepared = crate::preparation::prepare_sequence_text_line(&line);
        let line = if prepared.is_empty() {
            "\u{200b}"
        } else {
            prepared.as_ref()
        };
        let style = ctx.typography.terminal_style("", css.clone());
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" class="sectionTitle"{dy} style="{style}">{text}</text>"#,
            x = fmt(x),
            y = fmt(y),
            style = escape_attr_display(&style),
            text = super::escape_prepared_text(line)
        );
        out.checkpoint()?;
        if ctx.margin > 0.0 {
            preceding_height += measure_sequence_drawn_line_height(
                ctx.measurer,
                line,
                &drawn_style,
                SequenceDrawnTextNode::Direct,
                ctx.checkpoints.text(),
            )?;
        }
    }
    ctx.checkpoints.checkpoint()
}

fn block_text_dy(index: usize, margin: f64, font_size: f64) -> String {
    block_row_dy(index, margin, font_size)
        .map(|dy| format!(r#" dy="{}""#, fmt(dy)))
        .unwrap_or_default()
}

pub(super) fn block_row_dy(index: usize, margin: f64, font_size: f64) -> Option<f64> {
    (margin == 0.0 && index != 0).then(|| index as f64 * font_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::{
        resources::{OperationWorkMeter, RenderResourcePolicy},
        text::TextMetrics,
    };
    use std::cell::RefCell;

    #[derive(Default)]
    struct RowProbe(RefCell<Vec<&'static str>>);

    impl TextMeasurer for RowProbe {
        fn measure(&self, _: &str, _: &TextStyle) -> TextMetrics {
            panic!("block rows must use their final DOM height operation")
        }
        fn measure_svg_raw_text_bbox_height_px(&self, text: &str, _: &TextStyle) -> f64 {
            self.0.borrow_mut().push("raw");
            match text {
                "first" => 10.4,
                "\u{200b}" => 20.4,
                "&" => 8.0,
                _ => panic!("unexpected row"),
            }
        }
        fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, _: &TextStyle) -> f64 {
            self.0.borrow_mut().push("tspan");
            match text {
                "first" => 7.2,
                "\u{200b}" => 9.4,
                "&" => 8.0,
                _ => panic!("unexpected row"),
            }
        }
    }

    #[test]
    fn block_rows_use_per_shape_heights_and_only_zero_margin_emits_implicit_dy() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let style = TextStyle::default();
        let config = merman_model::presentation::PresentationConfig::default();
        let typography = crate::preparation::SequenceTypographyPlan::resolve(&config);
        for margin in [5.0, 0.0, -5.0] {
            for tspan in [true, false] {
                let probe = RowProbe::default();
                let ctx = LoopTextRenderContext {
                    typography: typography.loop_label(),
                    math_sidecar: &Default::default(),
                    measurer: &probe,
                    style: &style,
                    config: &config,
                    margin,
                    checkpoints: SequenceEmitCheckpoints::for_emit(&meter),
                };
                let mut out = String::new();
                if tspan {
                    write_loop_text_lines(
                        &mut out,
                        &ctx,
                        LoopTextPlacement {
                            x: 20.0,
                            math_center_x: 20.0,
                            y0: 10.25,
                            block_start_y: 0.0,
                            max_width: None,
                            use_tspan: true,
                        },
                        "test-label",
                        "first<br><br>&",
                    )
                    .unwrap();
                } else {
                    write_section_title_lines(
                        &mut out,
                        &ctx,
                        20.0,
                        10.25,
                        0.0,
                        None,
                        "test-label",
                        "first<br><br>&",
                    )
                    .unwrap();
                }
                let svg = format!("<svg>{out}</svg>");
                let document = roxmltree::Document::parse(&svg).unwrap();
                let rows: Vec<_> = document
                    .descendants()
                    .filter(|n| n.has_tag_name("text"))
                    .collect();
                let ys: Vec<_> = rows.iter().map(|n| n.attribute("y").unwrap()).collect();
                let expected = if margin > 0.0 {
                    if tspan {
                        vec!["13", "20", "29"]
                    } else {
                        vec!["13", "23", "44"]
                    }
                } else {
                    vec!["10.25"; 3]
                };
                assert_eq!(ys, expected);
                let dys: Vec<_> = rows.iter().map(|n| n.attribute("dy")).collect();
                assert_eq!(
                    dys,
                    if margin == 0.0 {
                        vec![None, Some("16"), Some("32")]
                    } else {
                        vec![None; 3]
                    }
                );
                assert_eq!(
                    *probe.0.borrow(),
                    if margin > 0.0 {
                        vec![if tspan { "tspan" } else { "raw" }; 3]
                    } else {
                        vec![]
                    }
                );
                assert!(out.contains('\u{200b}'));
                assert!(out.contains("&amp;"));
            }
        }
    }
}
