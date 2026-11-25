use crate::architecture::labels::{SvgLine, SvgWordType, wrap_svg_words_to_lines};

use super::super::{SvgOutput, escape_xml_into, fmt};

pub(super) fn write_svg_text_lines(out: &mut impl SvgOutput, lines: &[SvgLine]) {
    out.push_str(r#"<text y="-10.1" style="">"#);
    if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
        out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        return;
    }
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em">"#);
        } else if idx == 1 {
            out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="1em" dy="1.1em">"#);
        } else {
            let y_em = 1.0 + (idx as f64 - 1.0) * 1.1;
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan row" x="0" y="{:.1}em" dy="1.1em">"#,
                y_em
            );
        }
        for (word_idx, word) in line.iter().enumerate() {
            let (font_style, font_weight) = match word.word_type {
                SvgWordType::Normal => ("normal", "normal"),
                SvgWordType::Strong => ("normal", "bold"),
                SvgWordType::Em => ("italic", "normal"),
            };
            let _ = write!(
                out,
                r#"<tspan font-style="{font_style}" class="text-inner-tspan" font-weight="{font_weight}">"#,
            );
            if word_idx != 0 {
                out.push(' ');
            }
            escape_xml_into(out, word.content.as_str());
            out.push_str("</tspan>");
        }
        out.push_str("</tspan>");
    }
    out.push_str("</text>");
}

pub(super) fn write_architecture_service_title(
    out: &mut impl SvgOutput,
    title: &str,
    icon_size_px: f64,
    title_width_px: f64,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
) {
    let lines = wrap_svg_words_to_lines(title, title_width_px, measurer, style);
    write_architecture_centered_text(out, &lines, icon_size_px / 2.0, icon_size_px);
}

pub(super) fn write_architecture_centered_text(
    out: &mut impl SvgOutput,
    lines: &[SvgLine],
    x: f64,
    y: f64,
) {
    let _ = write!(
        out,
        r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle" transform="translate({x}, {y})"><g><rect class="background" style="stroke: none"/>"#,
        x = fmt(x),
        y = fmt(y),
    );
    write_svg_text_lines(out, lines);
    out.push_str("</g></g>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture::labels::{
        svg_line_computed_length_px, svg_line_formatted_bbox_width_px,
    };
    use crate::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::cell::RefCell;

    type MeasurementCall = (String, Option<String>, Option<String>);

    #[derive(Default)]
    struct ExactPrimitiveProbe {
        computed_calls: RefCell<Vec<MeasurementCall>>,
        bbox_calls: RefCell<Vec<MeasurementCall>>,
    }

    impl TextMeasurer for ExactPrimitiveProbe {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("Architecture createText must not use generic measurement")
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.computed_calls.borrow_mut().push((
                text.to_string(),
                style.font_weight.clone(),
                style.font_style.clone(),
            ));
            let scale = if style.font_weight.as_deref() == Some("bold") {
                20.0
            } else if style.font_style.as_deref() == Some("italic") {
                15.0
            } else {
                10.0
            };
            text.chars().count() as f64 * scale
        }

        fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
            self.bbox_calls.borrow_mut().push((
                text.to_string(),
                style.font_weight.clone(),
                style.font_style.clone(),
            ));
            (100.0, 111.0)
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            panic!("Architecture formatted rows must not use the single-tspan operation")
        }
    }

    fn text_style() -> crate::text::TextStyle {
        crate::text::TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        }
    }

    #[test]
    fn styled_svg_wrap_uses_computed_length_with_inner_run_styles() {
        let measurer = ExactPrimitiveProbe::default();
        let lines = wrap_svg_words_to_lines("a **b** *c*", 80.0, &measurer, &text_style());

        assert_eq!(lines.len(), 1);
        let calls = measurer.computed_calls.borrow();
        assert!(calls.iter().any(|call| {
            call == &(
                "a".to_string(),
                Some("normal".to_string()),
                Some("normal".to_string()),
            )
        }));
        assert!(calls.iter().any(|call| {
            call == &(
                " b".to_string(),
                Some("bold".to_string()),
                Some("normal".to_string()),
            )
        }));
        assert!(calls.iter().any(|call| {
            call == &(
                " c".to_string(),
                Some("normal".to_string()),
                Some("italic".to_string()),
            )
        }));
    }

    #[test]
    fn architecture_plain_service_title_matches_create_text_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = text_style();
        let mut out = String::new();

        write_architecture_service_title(&mut out, "s1", 80.0, 120.0, &measurer, &style);

        assert_eq!(
            out,
            r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle" transform="translate(40, 80)"><g><rect class="background" style="stroke: none"/><text y="-10.1" style=""><tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">s1</tspan></tspan></text></g></g>"#
        );
    }

    #[test]
    fn architecture_plain_words_match_single_line_create_text_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = text_style();
        let mut out = String::new();

        write_svg_text_lines(
            &mut out,
            &wrap_svg_words_to_lines("Service Farm", 120.0, &measurer, &style),
        );

        assert_eq!(
            out,
            r#"<text y="-10.1" style=""><tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">Service</tspan><tspan font-style="normal" class="text-inner-tspan" font-weight="normal"> Farm</tspan></tspan></text>"#
        );
    }

    #[test]
    fn emitted_line_bbox_uses_formatted_operation_and_homogeneous_run_style() {
        let measurer = ExactPrimitiveProbe::default();
        let style = text_style();
        let lines = wrap_svg_words_to_lines("**Bold words**", 1_000.0, &measurer, &style);
        let line = lines.first().expect("one bold line");

        assert_eq!(svg_line_computed_length_px(line, &measurer, &style), 200.0);
        assert_eq!(
            svg_line_formatted_bbox_width_px(line, &measurer, &style),
            211.0
        );
        assert_eq!(
            measurer.bbox_calls.borrow().as_slice(),
            &[(
                "Bold words".to_string(),
                Some("bold".to_string()),
                Some("normal".to_string()),
            )]
        );
    }
}
