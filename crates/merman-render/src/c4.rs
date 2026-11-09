use crate::model::{C4TextBlockLayout, C4TextRenderPlan, C4TextRowLayout};
use crate::resources::OperationWorkMeter;
use crate::text::{
    TextMeasurer, TextMetrics, TextStyle, measure_mermaid_text_dimensions_with_checkpoint,
    split_mermaid_common_br_lines, wrap_label_like_mermaid_lines_with_checkpoint,
};
use merman_model::families::c4::{C4DiagramModel, C4Shape};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

mod config;
mod theme;

pub(crate) use config::{
    C4_DEFAULT_FONT_FAMILY, C4_ELEMENT_TYPES, C4ConfigView, C4LayoutSettings, C4Look,
    default_use_max_width,
};
pub(crate) use theme::C4ClusterThemePlan;

type C4Model = C4DiagramModel;
type C4Conf = C4LayoutSettings;

// Mermaid's unified `fr-rect` handler reserves an eight-pixel frame on each side. Neo changes
// only the label padding; it does not use the separate `shadedProcess` geometry.
pub(crate) const C4_FRAMED_FRAME_WIDTH: f64 = 8.0;
pub(crate) const C4_NEO_FRAMED_LABEL_PADDING_X: f64 = 28.0;
pub(crate) const C4_NEO_FRAMED_LABEL_PADDING_Y: f64 = 12.0;

/// The unified Mermaid shape used to draw a C4 element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum C4NodeShape {
    Rounded,
    Framed,
    Person,
    Cylinder,
    HorizontalCylinder,
}

fn value_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn keyword_shape(value: Option<&str>) -> Option<C4NodeShape> {
    match value?.to_ascii_lowercase().as_str() {
        "person" => Some(C4NodeShape::Person),
        "box" | "rounded" => Some(C4NodeShape::Rounded),
        "component" => Some(C4NodeShape::Framed),
        "cylinder" | "database" | "db" => Some(C4NodeShape::Cylinder),
        "queue" | "pipe" => Some(C4NodeShape::HorizontalCylinder),
        _ => None,
    }
}

/// Resolves `$shape`, `$sprite`, `$tags`, and the legacy C4 element type in the
/// same order as Mermaid's `c4ShapeAdapter`.
pub(crate) fn c4_node_shape(shape: &C4Shape) -> C4NodeShape {
    keyword_shape(value_string(shape.shape.as_ref()))
        .or_else(|| keyword_shape(value_string(shape.sprite.as_ref())))
        .or_else(|| {
            value_string(shape.tags.as_ref()).and_then(|tags| {
                tags.split(',')
                    .find_map(|tag| keyword_shape(Some(tag.trim())))
            })
        })
        .unwrap_or_else(|| match shape.type_c4_shape.as_str() {
            "person" | "external_person" => C4NodeShape::Person,
            value if value.ends_with("_db") => C4NodeShape::Cylinder,
            value if value.ends_with("_queue") => C4NodeShape::HorizontalCylinder,
            _ => C4NodeShape::Rounded,
        })
}

fn c4_stereotype_name(type_c4_shape: &str) -> String {
    let base = type_c4_shape
        .strip_prefix("external_")
        .unwrap_or(type_c4_shape);
    let base = base
        .strip_suffix("_db")
        .or_else(|| base.strip_suffix("_queue"))
        .unwrap_or(base);
    match base {
        "person" => "Person".to_string(),
        "system" => "Software System".to_string(),
        "container" => "Container".to_string(),
        "component" => "Component".to_string(),
        other => other.replace('_', " "),
    }
}

/// Returns the structured stereotype line used by the unified C4 label helper.
pub(crate) fn c4_stereotype_text(shape: &C4Shape) -> String {
    let stereotype = c4_stereotype_name(shape.type_c4_shape.as_str());
    match shape
        .techn
        .as_ref()
        .map(|text| text.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(technology) => format!("[{stereotype}: {technology}]"),
        None => format!("[{stereotype}]"),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextMeasure {
    width: f64,
    height: f64,
    line_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedTextMeasure {
    pub(crate) metrics: TextMeasure,
    pub(crate) render_plan: C4TextRenderPlan,
}

struct C4LegacyWrapMeasurer<'a> {
    measurer: &'a dyn TextMeasurer,
    work: &'a OperationWorkMeter,
}

impl TextMeasurer for C4LegacyWrapMeasurer<'_> {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        let dimensions = (|| -> crate::Result<_> {
            self.work.charge(text.len().saturating_add(1))?;
            measure_mermaid_text_dimensions_with_checkpoint(self.measurer, text, style, || {
                self.work.charge(1)?;
                Ok(())
            })
        })();
        // The wrapping helper checks the same sticky operation terminal immediately after
        // every infallible width probe, before it can consume a failed probe's result.
        dimensions.map_or_else(
            |_| TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            },
            |dimensions| TextMetrics {
                width: dimensions.width.max(0) as f64,
                height: dimensions.height.max(0) as f64,
                line_count: 1,
            },
        )
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure(text, style).width
    }
}

/// Prepares the legacy boundary/relationship text consumed by Mermaid's `byTspan` path.
///
/// As in `calcC4ShapeTextWH`, `text` retains `wrapLabel`'s actual output. Only inserted
/// common break tags delimit its rows; literal newlines and entity spellings remain text.
fn prepare_c4_legacy_text(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap: bool,
    text_limit_width: Option<f64>,
    work: &OperationWorkMeter,
) -> crate::Result<C4TextBlockLayout> {
    work.charge(text.len().saturating_add(1))?;
    let source_rows = split_mermaid_common_br_lines(text);
    if wrap {
        let text_limit_width = match text_limit_width {
            Some(width) => width,
            None => measure_mermaid_text_dimensions_with_checkpoint(measurer, text, style, || {
                work.charge(1)?;
                Ok(())
            })?
            .width
            .max(0) as f64,
        };
        let text = if source_rows.len() > 1 {
            // Mermaid's hasBreaks short-circuit preserves authored tag spellings and empty rows.
            text.to_owned()
        } else if text.bytes().all(|byte| byte == b' ') {
            // wrapLabel drops empty ASCII-space-delimited words, including all-space labels.
            String::new()
        } else {
            let lines = wrap_label_like_mermaid_lines_with_checkpoint(
                text,
                &C4LegacyWrapMeasurer { measurer, work },
                style,
                text_limit_width,
                || {
                    work.charge(1)?;
                    Ok(())
                },
            )?;
            for line in &lines {
                work.charge(line.len().saturating_add(1))?;
            }
            lines.join("<br/>")
        };
        work.charge(text.len().saturating_add(1))?;
        let dimensions =
            measure_mermaid_text_dimensions_with_checkpoint(measurer, &text, style, || {
                work.charge(1)?;
                Ok(())
            })?;
        return Ok(C4TextBlockLayout {
            line_count: split_mermaid_common_br_lines(&text).len(),
            text,
            y: 0.0,
            width: text_limit_width,
            height: dimensions.height.max(0) as f64,
            render_plan: None,
        });
    }

    // The unwrapped upstream branch probes each row independently; an empty row contributes
    // zero height here, unlike the wrapped calculateTextHeight call on the complete label.
    let mut width = 0.0_f64;
    let mut height = 0.0_f64;
    for line in &source_rows {
        work.charge(line.len().saturating_add(1))?;
        let dimensions =
            measure_mermaid_text_dimensions_with_checkpoint(measurer, line, style, || {
                work.charge(1)?;
                Ok(())
            })?;
        width = width.max(dimensions.width.max(0) as f64);
        height += dimensions.height.max(0) as f64;
    }
    Ok(C4TextBlockLayout {
        text: text.to_owned(),
        y: 0.0,
        width,
        height,
        line_count: source_rows.len(),
        render_plan: None,
    })
}

/// Measures one section of the unified C4 label.
///
/// Unlike the legacy C4 path, the unified label helper sizes from the rendered
/// section's actual wrapped bounding box; the configured wrap width is only a
/// line-breaking constraint. Boundaries and relationships use the separate
/// legacy preparation above and retain Mermaid's fixed-width path.
fn measure_c4_unified_text(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap: bool,
    text_limit_width: f64,
) -> UnifiedTextMeasure {
    let source_lines = c4_source_word_lines(text);
    // Mermaid's `createText` installs `font-weight="normal"` and
    // `font-style="normal"` on the temporary tspans used by its line-fitting
    // probe. `c4LabelHelper` removes those attributes only after wrapping, so
    // the final section can inherit C4 typography such as the bold name style.
    // Keep those two measurement phases distinct: wrapping follows the
    // temporary normal text, while the final bounds use the rendered style.
    let mut wrapping_style = style.clone();
    wrapping_style.font_weight = Some("normal".to_string());
    wrapping_style.font_style = Some("normal".to_string());
    let rows = c4_wrap_source_word_lines(
        measurer,
        &source_lines,
        &wrapping_style,
        wrap.then_some(text_limit_width),
    );
    let mut left: f64 = 0.0;
    let mut right: f64 = 0.0;
    let mut top = f64::INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for (index, row) in rows.iter().enumerate() {
        let row_text = c4_visible_row(&row.words);
        if row_text.is_empty() {
            continue;
        }
        let (row_left, row_right) = measurer.measure_svg_text_bbox_x(&row_text, style);
        left = left.max(row_left.max(0.0));
        right = right.max(row_right.max(0.0));
        let y = measurer.measure_svg_create_text_bbox_y_offset_px(&row_text, style)
            + index as f64 * style.font_size.max(0.0) * 1.1;
        let height = measurer
            .measure_svg_raw_text_bbox_height_px(&row_text, style)
            .max(0.0);
        if height > 0.0 {
            top = top.min(y);
            bottom = bottom.max(y + height);
        }
    }
    // Rows are already final DOM text. Their decoded contents must not enter label parsing again.
    let has_visible_text = top.is_finite();
    let bbox_y = if has_visible_text { top } else { 0.0 };
    let metrics = TextMeasure {
        width: (left + right).max(0.0),
        height: if has_visible_text {
            (bottom - top).max(0.0)
        } else {
            0.0
        },
        line_count: if has_visible_text {
            rows.len().max(1)
        } else {
            0
        },
    };
    UnifiedTextMeasure {
        metrics,
        render_plan: C4TextRenderPlan {
            rows,
            bbox_x: -left,
            bbox_y,
        },
    }
}

fn c4_source_word_lines(text: &str) -> Vec<Vec<String>> {
    // Mermaid's nonMarkdownToLines treats literal `\\n`, actual newlines and `<br>` variants as
    // explicit line boundaries before tokenizing each trimmed line.
    crate::text::split_html_br_lines(text)
        .into_iter()
        .flat_map(|line| line.split("\\n"))
        .flat_map(|line| line.split('\n'))
        .map(|line| {
            crate::text::non_markdown_svg_words(line.trim())
                .map(str::to_string)
                .collect()
        })
        .collect()
}

fn c4_visible_row(row: &[String]) -> String {
    let mut visible = String::new();
    for (index, word) in row.iter().enumerate() {
        if index > 0 {
            visible.push(' ');
        }
        let decoded = crate::entities::decode_svg_text_content_entities(word);
        visible.push_str(decoded.as_ref());
    }
    visible
}

fn c4_row_width(measurer: &dyn TextMeasurer, row: &[String], style: &TextStyle) -> f64 {
    measurer.measure_svg_text_computed_length_px(&c4_visible_row(row), style)
}

fn c4_split_long_word(
    measurer: &dyn TextMeasurer,
    word: &str,
    style: &TextStyle,
    max_width: f64,
) -> Vec<String> {
    let boundaries = word
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .chain(std::iter::once(word.len()))
        .collect::<Vec<_>>();
    if boundaries.len() <= 1 {
        return vec![word.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start + 1 < boundaries.len() {
        let mut end = start + 1;
        let mut best = end;
        while end < boundaries.len() {
            let candidate = &word[boundaries[start]..boundaries[end]];
            if measurer.measure_svg_text_computed_length_px(candidate, style) <= max_width {
                best = end;
                end += 1;
            } else {
                break;
            }
        }
        chunks.push(word[boundaries[start]..boundaries[best]].to_string());
        start = best;
    }
    chunks
}

fn c4_wrap_source_word_lines(
    measurer: &dyn TextMeasurer,
    source_lines: &[Vec<String>],
    style: &TextStyle,
    max_width: Option<f64>,
) -> Vec<C4TextRowLayout> {
    let Some(max_width) = max_width.filter(|width| width.is_finite() && *width > 0.0) else {
        return source_lines
            .iter()
            .cloned()
            .map(|words| C4TextRowLayout { words })
            .collect();
    };

    let mut wrapped = Vec::new();
    for source_line in source_lines {
        if source_line.is_empty() {
            wrapped.push(C4TextRowLayout { words: Vec::new() });
            continue;
        }
        if c4_row_width(measurer, source_line, style) <= max_width {
            wrapped.push(C4TextRowLayout {
                words: source_line.clone(),
            });
            continue;
        }

        let mut current = Vec::new();
        for word in source_line {
            let mut candidate = current.clone();
            candidate.push(word.clone());
            if c4_row_width(measurer, &candidate, style) <= max_width {
                current.push(word.clone());
                continue;
            }
            if !current.is_empty() {
                wrapped.push(C4TextRowLayout {
                    words: std::mem::take(&mut current),
                });
            }
            if c4_row_width(measurer, std::slice::from_ref(word), style) <= max_width {
                current.push(word.clone());
            } else {
                for chunk in c4_split_long_word(measurer, word, style, max_width) {
                    wrapped.push(C4TextRowLayout { words: vec![chunk] });
                }
            }
        }
        if !current.is_empty() {
            wrapped.push(C4TextRowLayout { words: current });
        }
    }

    if wrapped.is_empty() {
        vec![C4TextRowLayout { words: Vec::new() }]
    } else {
        wrapped
    }
}

mod layout;
mod text_paint;
mod typography;
pub(crate) use layout::layout_c4_diagram_typed;
pub(crate) use text_paint::C4TextPaintPlan;
pub(crate) use typography::C4TypographyThemePlan;

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use merman_model::{OperationControl, OperationPhase};

    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use crate::text::{TextMeasurer, TextMetrics};

    use super::{TextStyle, measure_c4_unified_text, prepare_c4_legacy_text};

    struct C4ProbeMeasurer;

    impl TextMeasurer for C4ProbeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 3,
            }
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_family.as_deref() == Some("sans-serif") {
                80.0
            } else {
                120.0
            }
        }

        fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            19.0
        }
    }

    #[test]
    fn c4_unwrapped_text_consumes_shared_mermaid_dimensions() {
        let measured = prepare_c4_legacy_text(
            &C4ProbeMeasurer,
            "configured text",
            &TextStyle::default(),
            false,
            Some(500.0),
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();

        assert_eq!(measured.width, 120.0);
        assert_eq!(measured.height, 19.0);
        assert_eq!(measured.line_count, 1);
    }

    #[test]
    fn c4_wrapped_text_uses_shared_dimensions_for_its_actual_common_rows() {
        let measured = prepare_c4_legacy_text(
            &C4ProbeMeasurer,
            "First</br>Second",
            &TextStyle::default(),
            true,
            Some(200.0),
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();

        assert_eq!(measured.width, 200.0);
        assert_eq!(measured.height, 38.0);
        assert_eq!(measured.line_count, 2);
        assert_eq!(measured.text, "First</br>Second");
    }

    #[derive(Default)]
    struct C4LegacyProbeMeasurer {
        probes: Cell<usize>,
        cancel: Option<OperationControl>,
    }

    impl TextMeasurer for C4LegacyProbeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("legacy C4 must use the calculateTextDimensions measurement operation");
        }

        fn measure_mermaid_calculate_text_dimensions(
            &self,
            text: &str,
            style: &TextStyle,
        ) -> TextMetrics {
            self.probes.set(self.probes.get() + 1);
            if let Some(control) = &self.cancel {
                control.cancel();
            }
            let sans = style.font_family.as_deref() == Some("sans-serif");
            TextMetrics {
                width: text.chars().filter(|ch| *ch != '\u{200b}').count() as f64
                    * if sans { 2.0 } else { 1.0 },
                height: if sans { 10.0 } else { 8.0 },
                line_count: 1,
            }
        }
    }

    #[test]
    fn c4_legacy_wrap_uses_the_shared_dual_font_probe_and_retains_its_rows() {
        let measured = prepare_c4_legacy_text(
            &C4LegacyProbeMeasurer::default(),
            "Alpha Beta Gamma",
            &TextStyle::default(),
            true,
            Some(14.0),
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();

        assert_eq!(measured.text, "Alpha<br/>Beta<br/>Gamma");
        assert_eq!(measured.line_count, 3);
        assert_eq!(measured.width, 14.0);
        assert_eq!(measured.height, 30.0);
    }

    #[test]
    fn c4_relationship_wrap_width_uses_the_shared_probe_before_preparing_rows() {
        let measured = prepare_c4_legacy_text(
            &C4LegacyProbeMeasurer::default(),
            "Alpha Beta",
            &TextStyle::default(),
            true,
            None,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();
        assert_eq!(measured.text, "Alpha<br/>Beta");
        assert_eq!(measured.line_count, 2);
        assert_eq!(measured.width, 20.0);
        assert_eq!(measured.height, 20.0);
    }

    #[test]
    fn c4_legacy_empty_rows_keep_the_upstream_wrapped_height_distinction() {
        for wrap in [false, true] {
            let measured = prepare_c4_legacy_text(
                &C4LegacyProbeMeasurer::default(),
                "A</br></br>B",
                &TextStyle::default(),
                wrap,
                Some(100.0),
                &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            )
            .unwrap();
            assert_eq!(measured.line_count, 3);
            assert_eq!(measured.height, if wrap { 30.0 } else { 20.0 });
            assert_eq!(measured.text, "A</br></br>B");
        }
    }

    #[test]
    fn c4_legacy_long_word_work_is_charged_before_each_growing_probe() {
        let text = "W".repeat(4096);
        let measurer = C4LegacyProbeMeasurer::default();
        let work = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 20_000)
                .unwrap(),
        );
        let error = prepare_c4_legacy_text(
            &measurer,
            &text,
            &TextStyle::default(),
            true,
            Some(4096.0),
            &work,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::Error::ResourceLimitExceeded(error) if error.limit == "max_layout_work_units"
        ));
        assert!(measurer.probes.get() > 0 && measurer.probes.get() < text.len());
        assert_eq!(
            work.checkpoint(OperationPhase::Emit).unwrap_err(),
            work.charge(1).unwrap_err(),
            "the wrapping failure remains the operation's sticky terminal"
        );
    }

    #[test]
    fn c4_legacy_wrap_cancellation_stops_before_the_next_font_or_candidate() {
        let control = OperationControl::new();
        let measurer = C4LegacyProbeMeasurer {
            cancel: Some(control.clone()),
            ..Default::default()
        };
        let work = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        let error = prepare_c4_legacy_text(
            &measurer,
            &"W".repeat(4096),
            &TextStyle::default(),
            true,
            Some(10.0),
            &work,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::Error::Cancelled(error) if error.phase == OperationPhase::Layout
        ));
        assert_eq!(measurer.probes.get(), 1);
    }

    struct C4UnifiedWrapProbeMeasurer;

    impl TextMeasurer for C4UnifiedWrapProbeMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            let width_per_character = if style.font_weight.as_deref() == Some("bold") {
                8.0
            } else {
                7.0
            };
            let lines = text.lines().collect::<Vec<_>>();
            TextMetrics {
                width: lines
                    .iter()
                    .map(|line| line.chars().count() as f64 * width_per_character)
                    .fold(0.0, f64::max),
                height: lines.len().max(1) as f64 * 20.0,
                line_count: lines.len().max(1),
            }
        }
    }

    #[test]
    fn c4_unified_text_wraps_before_inheriting_final_section_weight() {
        let style = TextStyle {
            font_weight: Some("bold".to_string()),
            ..TextStyle::default()
        };
        let measured = measure_c4_unified_text(
            &C4UnifiedWrapProbeMeasurer,
            "Mainframe Banking System",
            &style,
            true,
            176.0,
        );

        assert_eq!(measured.metrics.line_count, 1);
        assert_eq!(measured.render_plan.rows.len(), 1);
        assert!(measured.metrics.width > 176.0);
    }
    #[test]
    fn c4_prepared_entity_row_is_measured_without_reinterpreting_literal_breaks() {
        let measurer = crate::text::DeterministicTextMeasurer {
            char_width_factor: 1.0,
            ..Default::default()
        };
        let style = TextStyle {
            font_size: 10.0,
            ..Default::default()
        };
        let measured = measure_c4_unified_text(&measurer, "A&lt;br/&gt;B", &style, false, 500.0);
        assert_eq!(measured.render_plan.rows.len(), 1);
        assert_eq!(measured.metrics.line_count, 1);
        assert_eq!(measured.metrics.height, 11.0);
        assert_eq!(measured.metrics.width, 70.0);
    }

    #[test]
    fn c4_unified_explicit_source_rows_survive_auto_wrap_settings() {
        let measurer = crate::text::DeterministicTextMeasurer {
            char_width_factor: 1.0,
            ..Default::default()
        };
        let style = TextStyle {
            font_size: 10.0,
            ..Default::default()
        };
        for wrap in [false, true] {
            for text in ["Alice\\nSmith", "Alice\nSmith", "Alice<br/>Smith"] {
                let measured = measure_c4_unified_text(&measurer, text, &style, wrap, 500.0);
                let rows = measured
                    .render_plan
                    .rows
                    .iter()
                    .map(|row| super::c4_visible_row(&row.words))
                    .collect::<Vec<_>>();
                assert_eq!(rows, ["Alice", "Smith"], "wrap={wrap} text={text:?}");
                assert_eq!(measured.metrics.line_count, 2);
                assert_eq!(measured.metrics.height, 22.0);
                assert_eq!(measured.metrics.width, 50.0);
            }
        }
    }

    #[test]
    fn c4_source_breaks_preserve_empty_rows_and_do_not_manufacture_html_tags() {
        let rows = super::c4_source_word_lines("\\nA\\n\nB<br\n/>C<br>\\n");
        let visible = rows
            .iter()
            .map(|row| super::c4_visible_row(row))
            .collect::<Vec<_>>();
        assert_eq!(visible, ["", "A", "", "B", "C", "", ""]);

        let malformed = super::c4_source_word_lines("A<br\\n/>B");
        let visible = malformed
            .iter()
            .map(|row| super::c4_visible_row(row))
            .collect::<Vec<_>>();
        assert_eq!(visible, ["A br", "/ B"]);
    }
}
