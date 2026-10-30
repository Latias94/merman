#[cfg(test)]
use crate::text::html_wrap::plan_html_lines;
use crate::text::html_wrap::{NormalizedHtmlText, PlannedHtmlLine, plan_html_lines_with_ranges};
use crate::text::{HtmlWhiteSpace, TextMeasurer, TextStyle, resolve_css_font_weight};
use std::borrow::Cow;
use std::ops::Range;

use super::cascade::ResolvedFallbackTypography;

use super::css::{extract_style_property_with_checkpoints, parse_css_px_value};
use crate::svg::pipeline::{
    checkpoint_loop, extract_exact_double_quoted_attr_with_checkpoints, find_with_checkpoints,
};

#[derive(Clone, Debug)]
pub(super) enum HtmlTextFragment {
    Text {
        text: String,
        typography: ResolvedFallbackTypography,
        white_space: HtmlWhiteSpace,
        // Common ancestry depth and mode at the boundary with the previous text leaf.
        boundary_white_space: Option<(usize, HtmlWhiteSpace)>,
    },
    HardBreak,
}

#[derive(Debug)]
struct StyledRange {
    range: Range<usize>,
    typography: ResolvedFallbackTypography,
}

#[derive(Debug, Default)]
pub(super) struct NormalizedFallbackText {
    source: NormalizedHtmlText,
    runs: Vec<StyledRange>,
}

pub(super) struct FallbackTextRun<'a> {
    pub(super) text: &'a str,
    pub(super) typography: &'a ResolvedFallbackTypography,
    offset: usize,
}

impl NormalizedFallbackText {
    pub(super) fn is_empty(&self) -> bool {
        self.source.is_empty()
    }

    fn push_range(&mut self, start: usize, typography: &ResolvedFallbackTypography) {
        let end = self.source.len();
        if start == end {
            return;
        }
        if let Some(previous) = self.runs.last_mut()
            && previous.range.end == start
            && previous.typography == *typography
        {
            previous.range.end = end;
        } else {
            self.runs.push(StyledRange {
                range: start..end,
                typography: typography.clone(),
            });
        }
    }

    fn runs_for_range<'a>(
        &'a self,
        text: &'a str,
        range: Range<usize>,
        selected_hyphen: bool,
    ) -> impl Iterator<Item = FallbackTextRun<'a>> {
        let source_start = range.start;
        let source_end = range.end;
        let first = self
            .runs
            .partition_point(|run| run.range.end <= source_start);
        self.runs[first..]
            .iter()
            .take_while(move |run| run.range.start < source_end)
            .filter_map(move |run| {
                let start = run.range.start.max(source_start) - source_start;
                let end = if selected_hyphen && run.range.end >= source_end {
                    text.len()
                } else {
                    run.range.end.min(source_end) - source_start
                };
                if start >= end {
                    return None;
                }
                Some(FallbackTextRun {
                    text: &text[start..end],
                    typography: &run.typography,
                    offset: start,
                })
            })
    }

    pub(super) fn line_runs<'a>(
        &'a self,
        line: &'a PlannedHtmlLine,
    ) -> impl Iterator<Item = FallbackTextRun<'a>> {
        self.runs_for_range(&line.text, line.source_range.clone(), line.selected_hyphen)
    }

    pub(super) fn has_inline_styles(
        &self,
        line: &PlannedHtmlLine,
        base: &ResolvedFallbackTypography,
    ) -> bool {
        self.line_runs(line)
            .any(|run| !same_font(run.typography, base) || run.typography.fill != base.fill)
    }

    pub(super) fn wrap<E>(
        &self,
        max_width: Option<f64>,
        measurer: &dyn TextMeasurer,
        fallback: &TextStyle,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<PlannedHtmlLine>, E> {
        let uniform = self
            .runs
            .windows(2)
            .all(|runs| same_font(&runs[0].typography, &runs[1].typography));
        let uniform_style = self
            .runs
            .first()
            .map(|run| run.typography.text_style())
            .unwrap_or_else(|| fallback.clone());
        let builtin = if uniform {
            measurer.begin_html_line_width(&uniform_style)
        } else {
            None
        };
        plan_html_lines_with_ranges(
            &self.source,
            max_width,
            builtin,
            &mut |text, range, selected_hyphen| {
                let mut width = 0.0;
                let mut group: Option<(&ResolvedFallbackTypography, usize, usize)> = None;
                for run in self.runs_for_range(text, range, selected_hyphen) {
                    let start = run.offset;
                    let end = start + run.text.len();
                    match group {
                        Some((typography, previous_start, previous_end))
                            if previous_end == start && same_font(typography, run.typography) =>
                        {
                            group = Some((typography, previous_start, end));
                        }
                        previous => {
                            if let Some((typography, start, end)) = previous {
                                width += measurer.measure_html_line_width_px(
                                    &text[start..end],
                                    &typography.text_style(),
                                );
                            }
                            group = Some((run.typography, start, end));
                        }
                    }
                }
                if let Some((typography, start, end)) = group {
                    width += measurer
                        .measure_html_line_width_px(&text[start..end], &typography.text_style());
                } else {
                    width = measurer.measure_html_line_width_px(text, fallback);
                }
                Ok(width)
            },
            checkpoint,
        )
    }
}

fn same_font(left: &ResolvedFallbackTypography, right: &ResolvedFallbackTypography) -> bool {
    left.font_family == right.font_family
        && left.font_size.to_bits() == right.font_size.to_bits()
        && resolve_css_font_weight(left.font_weight.as_deref().unwrap_or("normal"), 400)
            == resolve_css_font_weight(right.font_weight.as_deref().unwrap_or("normal"), 400)
        && left
            .font_style
            .as_deref()
            .unwrap_or("normal")
            .eq_ignore_ascii_case(right.font_style.as_deref().unwrap_or("normal"))
}

fn replace_with_checkpoint<E>(
    input: &str,
    needle: &str,
    replacement: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    if needle.is_empty() {
        checkpoint()?;
        return Ok(input.to_string());
    }
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(relative) = find_with_checkpoints(&input[cursor..], needle, checkpoint)? {
        let start = cursor + relative;
        output.push_str(&input[cursor..start]);
        output.push_str(replacement);
        cursor = start + needle.len();
    }
    output.push_str(&input[cursor..]);
    checkpoint()?;
    Ok(output)
}

fn decode_mermaid_entity_placeholders<'a, E>(
    text: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Cow<'a, str>, E> {
    if !text.contains('ﬂ') && !text.contains('¶') {
        return Ok(Cow::Borrowed(text));
    }

    let restored = replace_with_checkpoint(text, "ﬂ°°", "&#", checkpoint)?;
    let restored = replace_with_checkpoint(&restored, "ﬂ°", "&", checkpoint)?;
    replace_with_checkpoint(&restored, "¶ß", ";", checkpoint).map(Cow::Owned)
}

fn decode_html_entities<E>(
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    checkpoint()?;
    let mut current = text.to_string();
    for _ in 0..3 {
        checkpoint()?;
        if !current.contains('&') && !current.contains('ﬂ') && !current.contains('¶') {
            break;
        }
        // Mermaid's placeholders are not HTML entities. Restore that wrapper first,
        // then let the shared HTML entity decoder handle the browser-facing syntax.
        let restored = decode_mermaid_entity_placeholders(&current, checkpoint)?;
        checkpoint()?;
        let next = decode_html_entities_streaming(restored.as_ref(), checkpoint)?;
        if next == current {
            break;
        }
        current = next;
    }
    Ok(current)
}

fn decode_html_entities_streaming<E>(
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    use merman_model::text::{
        DecodedHtmlFragment, visit_decoded_html_entity_fragments_with_checkpoints,
    };

    let mut output = String::with_capacity(text.len());
    visit_decoded_html_entity_fragments_with_checkpoints(text, &mut *checkpoint, |fragment| {
        match fragment {
            DecodedHtmlFragment::Borrowed(value) => output.push_str(value),
            DecodedHtmlFragment::Scalar(value) => output.push(value),
        }
        Ok(())
    })?;
    Ok(output)
}

fn normalize_html_source_newlines<E>(
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut result = String::with_capacity(text.len());
    let mut previous_cr = false;
    for (index, ch) in text.chars().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        if ch == '\n' && previous_cr {
            previous_cr = false;
            continue;
        }
        previous_cr = ch == '\r';
        result.push(if previous_cr { '\n' } else { ch });
    }
    checkpoint()?;
    Ok(result)
}

pub(super) fn normalized_foreign_object_text<E>(
    fragments: &[HtmlTextFragment],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<NormalizedFallbackText, E> {
    let mut normalized = NormalizedFallbackText::default();
    for (index, fragment) in fragments.iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        match fragment {
            HtmlTextFragment::HardBreak => normalized.source.hard_break(),
            HtmlTextFragment::Text {
                text,
                white_space,
                boundary_white_space,
                typography,
            } => {
                if let Some((depth, mode)) = boundary_white_space {
                    normalized.source.inline_boundary(*depth, *mode);
                }
                // HTML parsing normalizes literal source CR before resolving character references.
                let text = normalize_html_source_newlines(text, checkpoint)?;
                // Mermaid's literal escape is an explicit break independently of CSS mode.
                for (part_index, part) in text.split("\\n").enumerate() {
                    if part_index > 0 {
                        normalized.source.hard_break();
                    }
                    let decoded = decode_html_entities(part, checkpoint)?;
                    let start = normalized.source.len();
                    normalized
                        .source
                        .append(&decoded, *white_space, checkpoint)?;
                    normalized.push_range(start, typography);
                }
            }
        }
    }
    checkpoint()?;
    Ok(normalized)
}

#[cfg(test)]
pub(super) fn wrap_html_text<E>(
    text: &NormalizedHtmlText,
    max_width: Option<f64>,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<PlannedHtmlLine>, E> {
    plan_html_lines(
        text,
        max_width,
        measurer.begin_html_line_width(style),
        &mut |text| Ok(measurer.measure_html_line_width_px(text, style)),
        checkpoint,
    )
}

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

#[cfg(test)]
mod tests {
    use super::{foreign_object_html_wrapping, wrap_html_text};
    use crate::text::html_wrap::{HtmlWhiteSpace, NormalizedHtmlText};
    use crate::text::{DeterministicTextMeasurer, TextStyle};
    use std::convert::Infallible;

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
