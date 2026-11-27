//! Shared XHTML cascade and line measurement for layout and SVG fallback.

pub mod cascade;

use self::cascade::{CascadeIndex, ResolvedFallbackTypography, ResolvedForeignObject};
use crate::mermaid_style::HtmlTextGeometryDeclarations;
use crate::svg_scan::{checkpoint_loop, find_with_checkpoints};
use crate::text::html_wrap::{
    HtmlWhiteSpace, NormalizedHtmlText, PlannedHtmlLine, plan_html_lines_with_measurer,
};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, resolve_css_font_weight};
use std::borrow::Cow;
use std::ops::Range;

#[derive(Clone, Debug)]
enum HtmlTextFragment {
    Text {
        text: String,
        typography: ResolvedFallbackTypography,
        white_space: HtmlWhiteSpace,
        // Common ancestry depth and mode at the boundary with the previous text leaf.
        boundary_white_space: Option<(usize, HtmlWhiteSpace)>,
    },
    HardBreak(f64),
    BlockBoundary,
}

#[derive(Debug)]
struct StyledRange {
    range: Range<usize>,
    typography: ResolvedFallbackTypography,
}

#[derive(Debug, Default)]
pub struct NormalizedFallbackText {
    source: NormalizedHtmlText,
    runs: Vec<StyledRange>,
    breaks: Vec<(usize, f64)>,
}

pub struct FallbackTextRun<'a> {
    pub text: &'a str,
    pub typography: &'a ResolvedFallbackTypography,
    offset: usize,
}

impl NormalizedFallbackText {
    pub fn is_empty(&self) -> bool {
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

    pub fn line_runs<'a>(
        &'a self,
        line: &'a PlannedHtmlLine,
    ) -> impl Iterator<Item = FallbackTextRun<'a>> {
        self.runs_for_range(&line.text, line.source_range.clone(), line.selected_hyphen)
    }

    pub fn has_inline_styles(
        &self,
        line: &PlannedHtmlLine,
        base: &ResolvedFallbackTypography,
    ) -> bool {
        self.line_runs(line)
            .any(|run| !same_width_style(run.typography, base) || run.typography.fill != base.fill)
    }

    pub fn line_box_height(&self, line: &PlannedHtmlLine, default: f64) -> f64 {
        if line.text.is_empty() {
            let offset = line.source_range.start;
            return self
                .breaks
                .binary_search_by_key(&offset, |(end, _)| *end)
                .map_or(default, |index| self.breaks[index].1);
        }
        self.line_runs(line)
            .map(|run| {
                run.typography
                    .line_height
                    .max(run.typography.line_box_floor)
            })
            .fold(0.0, f64::max)
    }

    pub fn trailing_letter_spacing(&self, line: &PlannedHtmlLine) -> f64 {
        self.line_runs(line)
            .filter(|run| run.text.chars().any(|character| character != '\u{00ad}'))
            .last()
            .map_or(0.0, |run| run.typography.text_geometry.spacing.letters)
    }

    fn hard_break(&mut self, line_height: f64) {
        self.breaks.push((self.source.len(), line_height));
        self.source.hard_break();
    }

    pub fn wrap<E>(
        &self,
        max_width: Option<f64>,
        measurer: &dyn TextMeasurer,
        fallback: &TextStyle,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<PlannedHtmlLine>, E> {
        let uniform = self
            .runs
            .windows(2)
            .all(|runs| same_width_style(&runs[0].typography, &runs[1].typography));
        let uniform_style = self
            .runs
            .first()
            .map(|run| run.typography.text_style())
            .unwrap_or_else(|| fallback.clone());
        let uniform = uniform.then(|| {
            let spacing = self.runs.first().map_or(Default::default(), |run| {
                run.typography.text_geometry.spacing
            });
            (measurer, &uniform_style, spacing)
        });
        plan_html_lines_with_measurer(
            &self.source,
            max_width,
            uniform,
            &mut |text, range, selected_hyphen| {
                let mut width = 0.0;
                let mut group: Option<(&ResolvedFallbackTypography, usize, usize)> = None;
                for run in self.runs_for_range(text, range, selected_hyphen) {
                    let start = run.offset;
                    let end = start + run.text.len();
                    match group {
                        Some((typography, previous_start, previous_end))
                            if previous_end == start
                                && same_width_style(typography, run.typography) =>
                        {
                            group = Some((typography, previous_start, end));
                        }
                        previous => {
                            if let Some((typography, start, end)) = previous {
                                width += measure_run(measurer, &text[start..end], typography);
                            }
                            group = Some((run.typography, start, end));
                        }
                    }
                }
                if let Some((typography, start, end)) = group {
                    width += measure_run(measurer, &text[start..end], typography);
                } else {
                    width = measurer.measure_html_line_width_px(text, fallback);
                }
                Ok(width.max(0.0))
            },
            checkpoint,
        )
    }
}

fn measure_run(
    measurer: &dyn TextMeasurer,
    text: &str,
    typography: &ResolvedFallbackTypography,
) -> f64 {
    measurer.measure_html_line_width_px(text, &typography.text_style())
        + typography.text_geometry.spacing.advance(text)
}

fn same_width_style(left: &ResolvedFallbackTypography, right: &ResolvedFallbackTypography) -> bool {
    left.font_family == right.font_family
        && left.font_size.to_bits() == right.font_size.to_bits()
        && left.text_geometry.spacing == right.text_geometry.spacing
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

pub fn normalized_foreign_object_text<E>(
    resolved: &ResolvedForeignObject,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<NormalizedFallbackText, E> {
    let mut normalized = NormalizedFallbackText::default();
    let mut in_word = false;
    for (index, fragment) in resolved.fragments.iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        match fragment {
            HtmlTextFragment::HardBreak(height) => {
                normalized.hard_break(*height);
                in_word = false;
            }
            HtmlTextFragment::BlockBoundary => {
                if !normalized.source.at_line_start() {
                    normalized.source.hard_break();
                }
                in_word = false;
            }
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
                        normalized
                            .hard_break(typography.line_height.max(typography.line_box_floor));
                        in_word = false;
                    }
                    let decoded = decode_html_entities(part, checkpoint)?;
                    let text = typography
                        .text_geometry
                        .transform
                        .apply(&decoded, &mut in_word);
                    let start = normalized.source.len();
                    normalized.source.append(&text, *white_space, checkpoint)?;
                    normalized.push_range(start, typography);
                }
            }
        }
    }
    checkpoint()?;
    Ok(normalized)
}

/// Measures a sanitized XHTML label using its local text styles.
pub fn measure_styled_html_label(
    measurer: &dyn TextMeasurer,
    xhtml: &str,
    style: &TextStyle,
    declarations: HtmlTextGeometryDeclarations,
    max_width: Option<f64>,
    fixed_width: bool,
) -> Option<TextMetrics> {
    if xhtml.is_empty()
        || (declarations.is_empty() && !xhtml.contains("style="))
        || xhtml.contains("<img")
        || xhtml.contains("<svg")
        || xhtml.contains("class=\"fa")
    {
        return None;
    }
    let mut checkpoint = || (!measurer.cancellation_requested()).then_some(()).ok_or(());
    let resolve = |white_space, checkpoint: &mut _| {
        CascadeIndex::resolve_label_for_layout(xhtml, style, declarations, white_space, checkpoint)
            .ok()
            .flatten()
    };
    let max_width = max_width.filter(|width| width.is_finite() && *width > 0.0);
    let mut resolved = resolve(
        if fixed_width && max_width.is_some() {
            HtmlWhiteSpace::BreakSpaces
        } else {
            HtmlWhiteSpace::Normal
        },
        &mut checkpoint,
    )?;
    if !resolved.has_text_geometry {
        return None;
    }
    let mut text = normalized_foreign_object_text(&resolved, &mut checkpoint).ok()?;
    if text.is_empty() {
        return None;
    }
    let mut lines = text
        .wrap(
            max_width.filter(|_| fixed_width),
            measurer,
            style,
            &mut checkpoint,
        )
        .ok()?;
    let mut width = lines.iter().map(|line| line.width).fold(0.0, f64::max);
    if let Some(limit) = max_width
        && !fixed_width
        && width >= limit
    {
        resolved = resolve(HtmlWhiteSpace::BreakSpaces, &mut checkpoint)?;
        text = normalized_foreign_object_text(&resolved, &mut checkpoint).ok()?;
        lines = text
            .wrap(max_width, measurer, style, &mut checkpoint)
            .ok()?;
        width = lines.iter().map(|line| line.width).fold(limit, f64::max);
    }
    Some(TextMetrics {
        width: crate::text::round_to_1_64_px(width),
        height: lines
            .iter()
            .map(|line| text.line_box_height(line, resolved.typography.line_height))
            .sum(),
        line_count: lines.len(),
    })
}
