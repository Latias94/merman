use super::css::{extract_style_property_with_checkpoints, parse_css_px_value};
use crate::svg::pipeline::{
    SvgTagScanner, extract_exact_double_quoted_attr_with_checkpoints, start_tag_name,
};

pub(super) fn extract_inline_html_style_property<E>(
    html: &str,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let Some(style) = extract_exact_double_quoted_attr_with_checkpoints(html, "style", checkpoint)?
    else {
        return Ok(None);
    };
    extract_style_property_with_checkpoints(style, property, checkpoint)
}

pub(super) fn foreign_object_html_wrapping<E>(
    tag: &str,
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<f64>, E> {
    // Width belongs to the label container; wrapping permissions belong to each text run.
    let css_width = extract_inline_html_style_property(inner, "width", checkpoint)?
        .and_then(|value| parse_css_px_value(&value));
    let max_width = extract_inline_html_style_property(inner, "max-width", checkpoint)?
        .and_then(|value| parse_css_px_value(&value));
    let attr_width = extract_exact_double_quoted_attr_with_checkpoints(tag, "width", checkpoint)?
        .and_then(|value| value.parse::<f64>().ok());
    let width = match (css_width, max_width) {
        (Some(width), Some(maximum)) => Some(width.min(maximum)),
        _ => css_width.or(max_width).or(attr_width),
    }
    .filter(|width| width.is_finite() && *width > 0.0);
    checkpoint()?;
    Ok(width)
}

pub(super) fn standalone_node_label_box<E>(
    html: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<crate::html::label_box::HtmlLabelBox>, E> {
    let Some(content) = html.strip_suffix("</span></div>") else {
        return Ok(None);
    };
    if !content.starts_with("<div ")
        || !content
            .strip_suffix("<p></p>")
            .unwrap_or(content)
            .ends_with("</div>")
    {
        return Ok(None);
    }
    let mut scanner = SvgTagScanner::new(content);
    let Some(wrapper) = scanner.next_with_checkpoints(checkpoint)? else {
        return Ok(None);
    };
    let span_start = scanner.cursor();
    if start_tag_name(wrapper.raw()) != Some("div") || !content[span_start..].starts_with("<span ")
    {
        return Ok(None);
    }
    let Some(span) = scanner.next_with_checkpoints(checkpoint)? else {
        return Ok(None);
    };
    let classes =
        extract_exact_double_quoted_attr_with_checkpoints(span.raw(), "class", checkpoint)?;
    if !classes.is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == "nodeLabel")
    }) {
        return Ok(None);
    }
    crate::html::label_box::HtmlLabelBox::from_xhtml(&content[scanner.cursor()..], checkpoint)
}

#[cfg(test)]
mod tests {
    use super::foreign_object_html_wrapping;
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};
    use merman_render_core::text::html_wrap::{
        HtmlWhiteSpace, NormalizedHtmlText, PlannedHtmlLine, plan_html_lines_with_measurer,
    };
    use std::convert::Infallible;

    fn wrap_html_text<E>(
        text: &NormalizedHtmlText,
        max_width: Option<f64>,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<PlannedHtmlLine>, E> {
        plan_html_lines_with_measurer(
            text,
            max_width,
            Some((measurer, style, Default::default())),
            &mut |text, _, _| Ok(measurer.measure_html_line_width_px(text, style)),
            checkpoint,
        )
    }

    #[test]
    fn break_spaces_fallback_reuses_layout_lines_with_trailing_spaces() {
        let measurer =
            DeterministicTextMeasurer::default().with_width_callback(|text, _| text.len() as f64);
        let mut checkpoint = || Ok::<_, Infallible>(());
        let text = NormalizedHtmlText::single_mode(
            "iiii iiii iiii iiii iiii iiii",
            HtmlWhiteSpace::BreakSpaces,
        );
        let lines = wrap_html_text(
            &text,
            Some(9.5),
            measurer.as_ref(),
            &TextStyle::default(),
            &mut checkpoint,
        )
        .unwrap();
        let lines = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(lines, ["iiii ", "iiii ", "iiii ", "iiii ", "iiii iiii"]);
    }

    #[test]
    fn normal_fallback_keeps_existing_trimmed_line_behavior() {
        let measurer =
            DeterministicTextMeasurer::default().with_width_callback(|text, _| text.len() as f64);
        let mut checkpoint = || Ok::<_, Infallible>(());
        let text = NormalizedHtmlText::single_mode(
            "iiii iiii iiii iiii iiii iiii",
            HtmlWhiteSpace::Normal,
        );
        let lines = wrap_html_text(
            &text,
            Some(9.5),
            measurer.as_ref(),
            &TextStyle::default(),
            &mut checkpoint,
        )
        .unwrap();
        let lines = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(lines, ["iiii iiii", "iiii iiii", "iiii iiii"]);
    }

    #[test]
    fn nowrap_remains_unwrapped_and_break_spaces_errors_propagate() {
        let mut checkpoint = || Ok::<_, Infallible>(());
        let width = foreign_object_html_wrapping(
            r#"<foreignObject width="9.5">"#,
            r#"<div style="white-space: nowrap; width: 9.5px">iiii iiii</div>"#,
            &mut checkpoint,
        )
        .unwrap();
        let measurer = DeterministicTextMeasurer::default();
        let text = NormalizedHtmlText::single_mode("iiii iiii", HtmlWhiteSpace::Nowrap);
        let lines = wrap_html_text(
            &text,
            width,
            &measurer,
            &TextStyle::default(),
            &mut checkpoint,
        )
        .unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "iiii iiii");
        let mut checkpoint = || Err::<(), _>("canceled");
        let result = wrap_html_text(
            &text,
            width,
            &measurer,
            &TextStyle::default(),
            &mut checkpoint,
        );
        assert_eq!(result.unwrap_err(), "canceled");
    }
}
