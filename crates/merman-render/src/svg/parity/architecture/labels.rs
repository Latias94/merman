#[cfg(merman_internal_theme_acceptance)]
use sha2::{Digest as _, Sha256};

#[cfg(merman_internal_theme_acceptance)]
use crate::architecture::labels::{
    ArchitectureTextAnchor, ArchitectureTextBaseline, architecture_svg_text_terminal_bounds,
};
use crate::architecture::labels::{
    SvgLine, SvgWordType, svg_lines_have_visible_words, wrap_svg_words_to_lines,
};
#[cfg(merman_internal_theme_acceptance)]
use crate::model::Bounds;

use super::super::{SvgOutput, escape_attr_into, escape_xml_into, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArchitectureTextWriterFacts {
    pub(super) has_visible_words: bool,
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) fragment_digest: [u8; 32],
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) run_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ArchitectureTextWriterEmission {
    #[cfg(merman_internal_theme_acceptance)]
    pub(super) bounds: Option<Bounds>,
    pub(super) facts: ArchitectureTextWriterFacts,
}

pub(super) fn write_svg_text_lines(
    out: &mut impl SvgOutput,
    lines: &[SvgLine],
    inline_style: Option<&str>,
) -> ArchitectureTextWriterFacts {
    let has_visible_words = svg_lines_have_visible_words(lines);
    #[cfg(merman_internal_theme_acceptance)]
    let start = out.len();
    out.push_str(r#"<text y="-10.1""#);
    out.push_str(r#" style=""#);
    if let Some(style) = inline_style {
        escape_attr_into(out, style);
    }
    out.push_str(r#"">"#);
    if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
        out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        #[cfg(merman_internal_theme_acceptance)]
        return architecture_text_writer_facts(out, start, 0, has_visible_words);
        #[cfg(not(merman_internal_theme_acceptance))]
        return ArchitectureTextWriterFacts { has_visible_words };
    }
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(r#"<tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em">"#);
        } else if idx == 1 {
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan row" x="0" y="1em" dy="1.1em">"#
            );
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
            if word_idx == 0 {
                escape_xml_into(out, word.content.as_str());
            } else {
                out.push(' ');
                escape_xml_into(out, word.content.as_str());
            }
            out.push_str("</tspan>");
        }
        out.push_str("</tspan>");
    }
    out.push_str("</text>");
    #[cfg(merman_internal_theme_acceptance)]
    {
        architecture_text_writer_facts(
            out,
            start,
            lines
                .len()
                .saturating_add(lines.iter().map(Vec::len).sum::<usize>()),
            has_visible_words,
        )
    }
    #[cfg(not(merman_internal_theme_acceptance))]
    {
        ArchitectureTextWriterFacts { has_visible_words }
    }
}

#[cfg(merman_internal_theme_acceptance)]
fn architecture_text_writer_facts(
    out: &impl SvgOutput,
    start: usize,
    run_count: usize,
    has_visible_words: bool,
) -> ArchitectureTextWriterFacts {
    let fragment = out.as_str().get(start..).unwrap_or_default();
    ArchitectureTextWriterFacts {
        has_visible_words,
        fragment_digest: Sha256::digest(fragment.as_bytes()).into(),
        run_count: if has_visible_words { run_count } else { 0 },
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
pub(super) fn write_architecture_service_title(
    out: &mut impl SvgOutput,
    title: &str,
    #[cfg(merman_internal_theme_acceptance)] service_x: f64,
    #[cfg(merman_internal_theme_acceptance)] service_y: f64,
    icon_size_px: f64,
    title_width_px: f64,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    inline_style: Option<&str>,
    #[cfg(merman_internal_theme_acceptance)] observe_bounds: bool,
) -> ArchitectureTextWriterEmission {
    let lines = wrap_svg_words_to_lines(title, title_width_px, measurer, style);
    #[cfg(merman_internal_theme_acceptance)]
    let terminal_bounds = observe_bounds.then(|| {
        architecture_svg_text_terminal_bounds(
            &lines,
            service_x + icon_size_px / 2.0,
            service_y + icon_size_px,
            ArchitectureTextAnchor::Middle,
            ArchitectureTextBaseline::Middle,
            measurer,
            style,
        )
    });
    let facts = write_architecture_centered_text(
        out,
        &lines,
        icon_size_px / 2.0,
        icon_size_px,
        inline_style,
    );
    ArchitectureTextWriterEmission {
        #[cfg(merman_internal_theme_acceptance)]
        bounds: terminal_bounds,
        facts,
    }
}

pub(super) fn write_architecture_centered_text(
    out: &mut impl SvgOutput,
    lines: &[SvgLine],
    x: f64,
    y: f64,
    inline_style: Option<&str>,
) -> ArchitectureTextWriterFacts {
    let _ = write!(
        out,
        r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle" transform="translate({x}, {y})"><g><rect class="background" style="stroke: none"/>"#,
        x = fmt(x),
        y = fmt(y)
    );
    let facts = write_svg_text_lines(out, lines, inline_style);
    out.push_str("</g></g>");
    facts
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

        let emission = write_architecture_service_title(
            &mut out,
            "s1",
            #[cfg(merman_internal_theme_acceptance)]
            0.0,
            #[cfg(merman_internal_theme_acceptance)]
            0.0,
            80.0,
            120.0,
            &measurer,
            &style,
            None,
            #[cfg(merman_internal_theme_acceptance)]
            true,
        );

        #[cfg(merman_internal_theme_acceptance)]
        {
            let bounds = emission.bounds.expect("requested service region");
            assert!(bounds.max_x > bounds.min_x && bounds.max_y > bounds.min_y);
        }
        #[cfg(not(merman_internal_theme_acceptance))]
        let _ = emission;
        assert_eq!(
            out,
            r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle" transform="translate(40, 80)"><g><rect class="background" style="stroke: none"/><text y="-10.1" style=""><tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">s1</tspan></tspan></text></g></g>"#
        );
    }

    #[test]
    #[cfg(merman_internal_theme_acceptance)]
    fn optional_service_region_preserves_the_emitted_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let mut observed = String::new();
        let mut unobserved = String::new();
        let observed_emission = write_architecture_service_title(
            &mut observed,
            "Service **title**",
            0.0,
            0.0,
            80.0,
            120.0,
            &measurer,
            &text_style(),
            None,
            true,
        );
        let unobserved_emission = write_architecture_service_title(
            &mut unobserved,
            "Service **title**",
            0.0,
            0.0,
            80.0,
            120.0,
            &measurer,
            &text_style(),
            None,
            false,
        );
        assert!(observed_emission.bounds.is_some());
        assert!(unobserved_emission.bounds.is_none());
        assert_eq!(observed, unobserved);
    }

    #[test]
    fn architecture_plain_words_match_single_line_create_text_dom() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = text_style();
        let mut out = String::new();

        write_svg_text_lines(
            &mut out,
            &wrap_svg_words_to_lines("Service Farm", 120.0, &measurer, &style),
            None,
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
