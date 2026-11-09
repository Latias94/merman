#[cfg(feature = "layout-elk")]
use crate::elk_options::layout_options as er_elk_layout_options;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, ErDiagramLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::{
    MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, MermaidMarkdownWordType, TextMeasurer, TextMetrics,
    TextStyle, WrapMode, measure_mermaid_text_dimensions,
};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
#[cfg(feature = "layout-elk")]
use merman_layout_elk as elk;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, OnceLock};

mod config;
mod swimlane;
mod text_facts;
mod theme;

use crate::layout_backend::GraphLayoutBackend;
use config::ErLayoutSettings;
pub(crate) use config::{ErConfigView, ErEntityMeasurementSettings};
pub(crate) use swimlane::ErSwimlanePaintLayout;
pub(crate) use swimlane::relationship_html_source as swimlane_relationship_html_source;
pub(crate) use theme::{
    ErAttributeTextRole, ErBaseFontSizePlan, ErCssBinding, ErEntitySourceStyle, ErEntityThemePlan,
    ErEntityThemeReceipt, subgraph_svg_text_facts,
};

pub(crate) type ErEntity = merman_model::families::er::ErEntity;
pub(crate) type ErRelationship = merman_model::families::er::ErRelationship;
pub(crate) type ErSubgraph = merman_model::families::er::ErSubgraph;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ErHtmlLabelLayout {
    pub(crate) width_budget: i64,
    pub(crate) break_spaces: bool,
}

#[derive(Debug)]
pub(crate) struct ErBoxLabel {
    markdown_input: String,
    rendered_text: String,
    xhtml_fragment: String,
    generic_workaround: bool,
    svg_markup: Option<String>,
    html_layout: Option<ErHtmlLabelLayout>,
    terminal_font_size: Option<f64>,
    visible_style_facts: OnceLock<text_facts::ErTextFacts>,
}

impl ErBoxLabel {
    pub(crate) fn from_source(source: &str) -> Self {
        Self::from_source_with_generic_types(source, true)
    }

    pub(crate) fn from_source_with_generic_types(source: &str, expand_generic_types: bool) -> Self {
        // erBox receives source spelling. Browser entity decoding belongs to createText's
        // HTML insertion or SVG word terminals, after authored line breaks are selected.
        let restored = merman_model::text::restore_mermaid_entity_spelling(source);
        let source = restored.as_ref().trim();
        let parsed = if expand_generic_types {
            merman_model::text::parse_generic_types(source)
        } else {
            source.to_string()
        };
        let markdown_input = if parsed != source {
            parsed.replace('<', "&lt;").replace('>', "&gt;")
        } else {
            source.to_string()
        };
        // addText flattens entity-authored angle references too, after createText.
        // drawRect never enters addText's textContent transformation.
        let generic_workaround = expand_generic_types
            && (markdown_input.contains("&lt;") || markdown_input.contains("&gt;"));
        let xhtml_fragment =
            crate::text::mermaid_markdown_to_xhtml_label_fragment(&markdown_input, true);
        let rendered_text = if generic_workaround {
            // addText restores only angle-bracket spellings through textContent after
            // createText. Other entities retain the depth left by the terminal decoder.
            crate::text::mermaid_xhtml_label_text_content(&xhtml_fragment)
                .unwrap_or(parsed)
                .replace("&lt;", "<")
                .replace("&gt;", ">")
        } else {
            source.to_string()
        };

        Self {
            markdown_input,
            rendered_text,
            xhtml_fragment,
            generic_workaround,
            svg_markup: None,
            html_layout: None,
            terminal_font_size: None,
            visible_style_facts: OnceLock::new(),
        }
    }

    pub(crate) fn markdown_input(&self) -> &str {
        &self.markdown_input
    }

    pub(crate) fn rendered_text(&self) -> &str {
        &self.rendered_text
    }

    pub(crate) fn xhtml_fragment(&self) -> &str {
        &self.xhtml_fragment
    }

    pub(crate) fn uses_generic_workaround(&self) -> bool {
        self.generic_workaround
    }

    pub(crate) fn svg_text_source(&self) -> &str {
        if self.generic_workaround {
            &self.rendered_text
        } else {
            &self.markdown_input
        }
    }

    pub(crate) fn svg_markup(&self) -> &str {
        self.svg_markup
            .as_deref()
            .expect("ER SVG labels are prepared before emission")
    }

    pub(crate) fn html_layout(&self) -> Option<ErHtmlLabelLayout> {
        self.html_layout
    }

    fn retained_buffer_bytes(&self) -> usize {
        self.markdown_input
            .capacity()
            .saturating_add(self.rendered_text.capacity())
            .saturating_add(self.xhtml_fragment.capacity())
            .saturating_add(self.svg_markup.as_ref().map_or(0, String::capacity))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "ER keeps source font layers and the independent createText width budget explicit."
    )]
    fn prepare_metrics(
        &mut self,
        measurer: &dyn TextMeasurer,
        probe_style: &TextStyle,
        source_style: &theme::ErEntitySourceStyle,
        width: i64,
        wrap_mode: WrapMode,
        root_font_size: f64,
        probe_font_size: Option<f64>,
        work_meter: &OperationWorkMeter,
    ) -> Result<TextMetrics> {
        use merman_model::OperationPhase;

        work_meter.checkpoint(OperationPhase::Layout)?;
        let text_style = source_style.text_style(probe_style, root_font_size);
        self.terminal_font_size = source_style.terminal_font_size(probe_font_size);
        if wrap_mode == WrapMode::HtmlLike {
            if self.rendered_text().is_empty() {
                self.html_layout = Some(ErHtmlLabelLayout {
                    width_budget: width.max(0),
                    break_spaces: false,
                });
                return Ok(TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                });
            }
            // createText measures its original group/div/span before addText may remove
            // span descendants. Its selected whitespace state survives that removal.
            let span_style = source_style.text_style(&text_style, root_font_size);
            if !self.uses_generic_workaround() {
                self.terminal_font_size = source_style.terminal_font_size(self.terminal_font_size);
            }
            let max_width = Some(width.max(0) as f64);
            let nowrap = crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
                measurer,
                self.xhtml_fragment(),
                &span_style,
                None,
                root_font_size,
            );
            work_meter.checkpoint(OperationPhase::Layout)?;
            // The initial table-cell bbox is capped by max-width. The later table's
            // min-content expansion can exceed that cap without undoing this decision.
            let break_spaces = width > 0 && nowrap.width >= width as f64;
            let mut metrics = crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
                measurer,
                self.xhtml_fragment(),
                &span_style,
                max_width,
                root_font_size,
            );
            work_meter.checkpoint(OperationPhase::Layout)?;
            let layout = ErHtmlLabelLayout {
                width_budget: width.max(0),
                break_spaces,
            };
            if self.uses_generic_workaround() {
                // textContent leaves only the group and div source-font layers.
                self.xhtml_fragment = crate::svg::escape_xml(self.rendered_text());
                metrics =
                    self.measure_html_text_content(measurer, &text_style, layout, work_meter)?;
            }
            self.html_layout = Some(layout);
            return Ok(metrics);
        }
        let source = self.markdown_input();
        let source = source
            .strip_prefix('`')
            .and_then(|text| text.strip_suffix('`'))
            .unwrap_or(source);
        let lines = crate::text::mermaid_markdown_to_wrapped_word_lines(
            measurer,
            source,
            probe_style,
            Some(width.max(0) as f64),
            wrap_mode,
        );
        work_meter.checkpoint(OperationPhase::Layout)?;
        let flattened_first_row = self.uses_generic_workaround().then(|| {
            lines
                .first()
                .into_iter()
                .flatten()
                .enumerate()
                .fold(String::new(), |mut text, (index, (word, _))| {
                    if index > 0 {
                        text.push(' ');
                    }
                    text.push_str(
                        &crate::text::SvgTextEntityMode::CreateTextSource.visible_word(word),
                    );
                    text
                })
                .replace("&lt;", "<")
                .replace("&gt;", ">")
        });
        let metrics = match flattened_first_row.as_deref() {
            Some(first_row) => {
                measure_er_flattened_svg_rows(measurer, &lines, first_row, &text_style, work_meter)?
            }
            None => crate::text::measure_svg_markdown_word_lines(
                measurer,
                &lines,
                &text_style,
                crate::text::SvgTextEntityMode::CreateTextSource,
            ),
        };
        work_meter.checkpoint(OperationPhase::Layout)?;
        self.svg_markup = Some(self.prepare_svg_markup(
            &lines,
            flattened_first_row.as_deref(),
            &source_style.label_style_css(false),
            work_meter,
        )?);
        work_meter.checkpoint(OperationPhase::Layout)?;
        Ok(metrics)
    }

    fn measure_html_text_content(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        layout: ErHtmlLabelLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<TextMetrics> {
        use crate::text::html_wrap::{NormalizedHtmlText, plan_html_lines};
        let mut checkpoint = || {
            work_meter
                .checkpoint(merman_model::OperationPhase::Layout)
                .map_err(Error::from)
        };
        let mut normalized = NormalizedHtmlText::default();
        normalized.append(
            self.rendered_text(),
            if layout.break_spaces {
                crate::text::HtmlWhiteSpace::BreakSpaces
            } else {
                crate::text::HtmlWhiteSpace::Nowrap
            },
            &mut checkpoint,
        )?;
        let lines = plan_html_lines(
            &normalized,
            layout.break_spaces.then_some(layout.width_budget as f64),
            measurer.begin_html_line_width(style),
            &mut |text| {
                checkpoint()?;
                let width = measurer.measure_html_line_width_px(text, style);
                checkpoint()?;
                Ok(width)
            },
            &mut || {
                work_meter
                    .checkpoint(merman_model::OperationPhase::Layout)
                    .map_err(Error::from)
            },
        )?;
        let mut width = if layout.break_spaces {
            layout.width_budget as f64
        } else {
            0.0
        };
        let mut height = 0.0;
        for line in &lines {
            checkpoint()?;
            width = width.max(line.width);
            height += measurer.measure_html_line_height_px(&line.text, style);
            checkpoint()?;
        }
        Ok(TextMetrics {
            width,
            height,
            line_count: lines.len(),
        })
    }

    pub(crate) fn visible_style_facts(&self) -> &crate::text::VisibleTextStyleFacts {
        &self.text_facts().visible
    }

    fn text_facts(&self) -> &text_facts::ErTextFacts {
        self.visible_style_facts.get_or_init(|| {
            if let Some(markup) = &self.svg_markup {
                text_facts::ErTextFacts::svg(markup, self.terminal_font_size)
            } else {
                assert!(
                    self.html_layout.is_some(),
                    "ER text facts require a prepared terminal"
                );
                text_facts::ErTextFacts::html(&self.xhtml_fragment, self.terminal_font_size)
            }
        })
    }
}

fn measure_er_flattened_svg_rows(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<(String, crate::text::MermaidMarkdownWordType)>],
    first_row: &str,
    style: &TextStyle,
    work_meter: &OperationWorkMeter,
) -> Result<TextMetrics> {
    let mut width = 0.0_f64;
    let mut ink: Option<(f64, f64)> = None;
    for (index, words) in lines.iter().enumerate() {
        work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
        let mut observe = |text: &str, face: &TextStyle| -> Result<()> {
            if crate::text::trim_html_collapsible_ascii_whitespace(text).is_empty() {
                return Ok(());
            }
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            let top = measurer.measure_svg_create_text_bbox_y_offset_px(text, face)
                + index as f64 * style.font_size * 1.1;
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            let height = measurer.measure_svg_tspan_text_bbox_height_px(text, face);
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            if height > 0.0 {
                let bottom = top + height;
                ink = Some(ink.map_or((top, bottom), |(min, max)| (min.min(top), max.max(bottom))));
            }
            Ok(())
        };
        if index == 0 {
            width = width.max(measurer.measure_svg_tspan_text_bbox_width_px(first_row, style));
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            observe(first_row, style)?;
        } else {
            let measured = crate::text::measure_svg_markdown_word_lines(
                measurer,
                std::slice::from_ref(words),
                style,
                crate::text::SvgTextEntityMode::CreateTextSource,
            );
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            width = width.max(measured.width);
            for (word_index, (word, kind)) in words.iter().enumerate() {
                let visible = crate::text::SvgTextEntityMode::CreateTextSource.visible_word(word);
                let text = if word_index == 0 {
                    visible.into_owned()
                } else {
                    format!(" {visible}")
                };
                observe(&text, &crate::text::svg_markdown_word_style(style, *kind))?;
            }
        }
    }
    Ok(TextMetrics {
        width: crate::text::round_to_1_64_px(width),
        height: ink.map_or(0.0, |(top, bottom)| (bottom - top).max(0.0)),
        line_count: lines.len(),
    })
}

#[derive(Debug)]
pub(crate) struct ErPreparedRelationshipLabel {
    metrics: TextMetrics,
    terminal: ErRelationshipLabelTerminal,
    _retained: Option<PreparedTextRetainedReservation>,
}

#[derive(Debug, Clone)]
pub(crate) struct ErPreparedSvgRelationshipLabel {
    pub(crate) word_lines: Vec<Vec<(String, MermaidMarkdownWordType)>>,
    pub(crate) ink_bounds: Bounds,
    pub(crate) background_bounds: Option<Bounds>,
}

const ER_SVG_RELATIONSHIP_LABEL_PADDING_PX: f64 = 2.0;

impl ErPreparedSvgRelationshipLabel {
    fn new(
        source: &str,
        has_background: bool,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> Self {
        let word_lines = if source.is_empty() {
            vec![Vec::new()]
        } else {
            // Pinned SVG preprocessing still wraps when markdownAutoWrap is false.
            crate::text::mermaid_markdown_to_wrapped_word_lines(
                measurer,
                source,
                style,
                Some(MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX),
                WrapMode::SvgLike,
            )
        };
        // Use the same centered word-terminal bounds as Class relationships. Advance
        // determines row centering; glyph extents determine ink and background geometry.
        let mut horizontal: Option<(f64, f64)> = None;
        let mut vertical: Option<(f64, f64)> = None;
        for (row_index, row) in word_lines.iter().enumerate() {
            if measurer.cancellation_requested() {
                break;
            }
            let mut advance = 0.0;
            let mut row_bounds: Option<(f64, f64)> = None;
            for (word_index, (word, kind)) in row.iter().enumerate() {
                if measurer.cancellation_requested() {
                    break;
                }
                let visible = crate::text::SvgTextEntityMode::CreateTextSource.visible_word(word);
                let text = if word_index == 0 {
                    visible
                } else {
                    std::borrow::Cow::Owned(format!(" {visible}"))
                };
                let face = crate::text::svg_markdown_word_style(style, *kind);
                let word_advance = measurer.measure_svg_text_computed_length_px(&text, &face);
                if !crate::text::trim_html_collapsible_ascii_whitespace(&text).is_empty() {
                    let (left, right) = measurer.measure_svg_text_bbox_x(&text, &face);
                    let center = advance + word_advance / 2.0;
                    let min = center - left;
                    let max = center + right;
                    row_bounds =
                        Some(row_bounds.map_or((min, max), |(x0, x1)| (x0.min(min), x1.max(max))));
                    let top = measurer.measure_svg_create_text_bbox_y_offset_px(&text, &face)
                        + row_index as f64 * style.font_size * 1.1;
                    let height = measurer.measure_svg_tspan_text_bbox_height_px(&text, &face);
                    if height > 0.0 {
                        let bottom = top + height;
                        vertical = Some(
                            vertical
                                .map_or((top, bottom), |(y0, y1)| (y0.min(top), y1.max(bottom))),
                        );
                    }
                }
                advance += word_advance;
            }
            if measurer.cancellation_requested() {
                break;
            }
            if let Some((mut min, mut max)) = row_bounds {
                min -= advance / 2.0;
                max -= advance / 2.0;
                if let Some((_, kind)) = row.first()
                    && row.iter().all(|(_, next)| next == kind)
                {
                    // A uniform row can use the complete centered-text bbox directly.
                    // Decode each retained word once, without parsing terminal text as HTML.
                    let text = row
                        .iter()
                        .map(|(word, _)| {
                            crate::text::SvgTextEntityMode::CreateTextSource.visible_word(word)
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let face = crate::text::svg_markdown_word_style(style, *kind);
                    let (left, right) = measurer.measure_svg_text_bbox_x(&text, &face);
                    if left.is_finite() && right.is_finite() && left + right > 0.0 {
                        min = -left;
                        max = right;
                    }
                }
                horizontal =
                    Some(horizontal.map_or((min, max), |(x0, x1)| (x0.min(min), x1.max(max))));
            }
        }
        let (min_x, max_x) = horizontal.unwrap_or_default();
        let (min_y, max_y) = vertical.unwrap_or_default();
        let ink_bounds = Bounds {
            min_x,
            max_x,
            min_y,
            max_y,
        };
        // createFormattedText expands text.getBBox() by 2px on each side, but only
        // when the raw relationship source is truthy. Whitespace has a 4x4 background.
        let background_bounds = has_background.then_some(Bounds {
            min_x: min_x - ER_SVG_RELATIONSHIP_LABEL_PADDING_PX,
            max_x: max_x + ER_SVG_RELATIONSHIP_LABEL_PADDING_PX,
            min_y: min_y - ER_SVG_RELATIONSHIP_LABEL_PADDING_PX,
            max_y: max_y + ER_SVG_RELATIONSHIP_LABEL_PADDING_PX,
        });
        Self {
            word_lines,
            ink_bounds,
            background_bounds,
        }
    }

    fn metrics(&self) -> TextMetrics {
        let bounds = self.background_bounds.as_ref().unwrap_or(&self.ink_bounds);
        TextMetrics {
            width: (bounds.max_x - bounds.min_x).max(0.0),
            height: (bounds.max_y - bounds.min_y).max(0.0),
            line_count: self.word_lines.len(),
        }
    }

    fn retained_buffer_bytes(&self) -> usize {
        self.word_lines.iter().fold(
            self.word_lines
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<(String, MermaidMarkdownWordType)>>()),
            |bytes, row| {
                row.iter().fold(
                    bytes.saturating_add(
                        row.capacity()
                            .saturating_mul(
                                std::mem::size_of::<(String, MermaidMarkdownWordType)>(),
                            ),
                    ),
                    |bytes, (word, _)| bytes.saturating_add(word.capacity()),
                )
            },
        )
    }
}

#[derive(Debug, Clone)]
enum ErRelationshipLabelTerminal {
    Markdown {
        xhtml_fragment: String,
        svg: Option<ErPreparedSvgRelationshipLabel>,
        font_size_px: f64,
        visible_style_facts: OnceLock<text_facts::ErTextFacts>,
    },
    Swimlane(swimlane::ErSwimlaneRelationshipLabel),
}

impl ErPreparedRelationshipLabel {
    fn new(
        text: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        html_labels: bool,
        markdown_auto_wrap: bool,
        root_font_size: f64,
    ) -> Self {
        let has_background = !text.is_empty();
        let source = text.trim().to_string();
        let xhtml_fragment =
            crate::text::mermaid_markdown_to_xhtml_label_fragment(&source, markdown_auto_wrap);
        // insertEdgeLabel leaves createText's width at its default, independently of
        // flowchart.wrappingWidth. Only HTML substitutes nonbreaking spaces when
        // markdownAutoWrap is false.
        let max_width = Some(MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX);
        let svg = (!html_labels)
            .then(|| ErPreparedSvgRelationshipLabel::new(&source, has_background, measurer, style));
        let metrics = if let Some(svg) = &svg {
            svg.metrics()
        } else if source.is_empty() {
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            }
        } else {
            crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
                measurer,
                &xhtml_fragment,
                style,
                max_width,
                root_font_size,
            )
        };
        Self {
            metrics,
            terminal: ErRelationshipLabelTerminal::Markdown {
                xhtml_fragment,
                svg,
                font_size_px: style.font_size,
                visible_style_facts: OnceLock::new(),
            },
            _retained: None,
        }
    }

    fn retained_buffer_bytes(&self) -> usize {
        match &self.terminal {
            ErRelationshipLabelTerminal::Markdown {
                xhtml_fragment,
                svg,
                ..
            } => xhtml_fragment.capacity().saturating_add(
                svg.as_ref()
                    .map_or(0, ErPreparedSvgRelationshipLabel::retained_buffer_bytes),
            ),
            ErRelationshipLabelTerminal::Swimlane(label) => label.retained_buffer_bytes(),
        }
    }

    pub(crate) fn xhtml_fragment(&self) -> &str {
        match &self.terminal {
            ErRelationshipLabelTerminal::Markdown { xhtml_fragment, .. } => xhtml_fragment,
            ErRelationshipLabelTerminal::Swimlane(label) => label.xhtml_fragment().unwrap_or(""),
        }
    }

    pub(crate) fn svg(&self) -> Option<&ErPreparedSvgRelationshipLabel> {
        match &self.terminal {
            ErRelationshipLabelTerminal::Markdown { svg, .. } => svg.as_ref(),
            ErRelationshipLabelTerminal::Swimlane(_) => None,
        }
    }

    pub(crate) fn html_is_constrained(&self) -> bool {
        // Min-content can exceed the budget for long words or NBSP-connected text.
        // Those labels still use createText's table/break-spaces wrapper.
        self.metrics.width >= MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX
    }

    pub(crate) fn swimlane(&self) -> Option<&swimlane::ErSwimlaneRelationshipLabel> {
        match &self.terminal {
            ErRelationshipLabelTerminal::Swimlane(label) => Some(label),
            ErRelationshipLabelTerminal::Markdown { .. } => None,
        }
    }

    fn dimensions(&self) -> (f64, f64) {
        (self.metrics.width.max(0.0), self.metrics.height.max(0.0))
    }

    pub(crate) fn visible_style_facts(&self) -> &crate::text::VisibleTextStyleFacts {
        &self.text_facts().visible
    }

    fn text_facts(&self) -> &text_facts::ErTextFacts {
        match &self.terminal {
            ErRelationshipLabelTerminal::Markdown {
                xhtml_fragment,
                svg,
                font_size_px,
                visible_style_facts,
                ..
            } => visible_style_facts.get_or_init(|| {
                if let Some(svg) = svg {
                    let mut text = String::new();
                    for (word, _) in svg.word_lines.iter().flatten() {
                        text.push_str(
                            &crate::text::SvgTextEntityMode::CreateTextSource.visible_word(word),
                        );
                        text.push(' ');
                    }
                    text_facts::ErTextFacts::plain(&text, Some(*font_size_px))
                } else {
                    text_facts::ErTextFacts::html(xhtml_fragment, Some(*font_size_px))
                }
            }),
            ErRelationshipLabelTerminal::Swimlane(label) => label.text_facts(),
        }
    }

    fn new_swimlane(
        text: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        config: &merman_model::MermaidConfig,
    ) -> Self {
        let label = swimlane::ErSwimlaneRelationshipLabel::prepare(text, measurer, style, config);
        Self {
            metrics: label.metrics(),
            terminal: ErRelationshipLabelTerminal::Swimlane(label),
            _retained: None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ErPreparedLabels {
    entity_measures: BTreeMap<String, ErEntityMeasure>,
    relationship_labels: Vec<ErPreparedRelationshipLabel>,
    entity_font_size_px: f64,
    attribute_font_size_px: f64,
    relationship_font_size_px: f64,
    // The theme plan prepares cluster labels after layout and uses this same retained ledger.
    retained_work_meter: Option<Arc<OperationWorkMeter>>,
    _retained: Option<PreparedTextRetainedReservation>,
}

impl ErPreparedLabels {
    fn prepare_for_layout(
        model: &merman_model::families::er::ErDiagramModel,
        measurer: &dyn TextMeasurer,
        settings: &ErLayoutSettings,
        config: &Value,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let swimlane = crate::layout_backend::resolve_er_graph_layout(config).backend
            == GraphLayoutBackend::Swimlane;
        let sanitize_config = swimlane.then(|| {
            merman_model::MermaidConfig::from_value(merman_model::json::clone_value_nonrecursive(
                config,
            ))
        });
        Self::prepare_with_provider(
            model,
            measurer,
            settings,
            crate::config::config_bool(config, &["markdownAutoWrap"]).unwrap_or(true),
            sanitize_config.as_ref(),
            work_meter,
        )
    }

    #[cfg(test)]
    fn prepare(
        model: &merman_model::families::er::ErDiagramModel,
        measurer: &dyn TextMeasurer,
        settings: &ErLayoutSettings,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        Self::prepare_with_provider(model, measurer, settings, true, None, work_meter)
    }

    fn prepare_with_provider(
        model: &merman_model::families::er::ErDiagramModel,
        measurer: &dyn TextMeasurer,
        settings: &ErLayoutSettings,
        markdown_auto_wrap: bool,
        swimlane_config: Option<&merman_model::MermaidConfig>,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let owner_bytes = (std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>())
            .saturating_add(
                model
                    .relationships
                    .len()
                    .saturating_mul(std::mem::size_of::<ErPreparedRelationshipLabel>()),
            );
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes)?;
        let retained = work_meter.reserve_prepared_text_retained_bytes(owner_bytes)?;
        let mut entity_measures = BTreeMap::new();
        for entity in model.entities.values() {
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            work_meter
                .preflight_prepared_text_retained_bytes(std::mem::size_of::<ErEntityMeasure>())?;
            work_meter.charge(entity.css_classes.len())?;
            for raw in &entity.css_styles {
                work_meter.charge(raw.len())?;
            }
            for class_name in entity.css_classes.split_whitespace() {
                if let Some(class) = model.classes.get(class_name) {
                    for raw in class.styles.iter().chain(&class.text_styles) {
                        work_meter.charge(raw.len())?;
                    }
                }
            }
            let source_style = theme::compile_er_entity_source_style(entity, &model.classes);
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            let mut measure = measure_entity_box_with_source(
                entity,
                measurer,
                &settings.label_style,
                &settings.attr_style,
                &settings.calculation_style,
                settings.entity_measurement,
                source_style,
                work_meter,
            )?;
            let bytes = measure.retained_bytes().saturating_add(entity.id.len());
            measure._retained = Some(work_meter.reserve_prepared_text_retained_bytes(bytes)?);
            entity_measures.insert(entity.id.clone(), measure);
        }
        let mut relationship_labels = Vec::with_capacity(model.relationships.len());
        for relationship in &model.relationships {
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            let mut label = if let Some(config) = swimlane_config {
                ErPreparedRelationshipLabel::new_swimlane(
                    &relationship.role_a,
                    measurer,
                    &settings.relationship_label_style,
                    config,
                )
            } else {
                ErPreparedRelationshipLabel::new(
                    &relationship.role_a,
                    measurer,
                    &settings.relationship_label_style,
                    settings.relationship_html_labels,
                    markdown_auto_wrap,
                    settings.label_style.font_size,
                )
            };
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            let bytes = label.retained_buffer_bytes();
            label._retained = Some(work_meter.reserve_prepared_text_retained_bytes(bytes)?);
            relationship_labels.push(label);
        }
        Ok(Self {
            entity_measures,
            relationship_labels,
            entity_font_size_px: settings.label_style.font_size,
            attribute_font_size_px: settings.attr_style.font_size,
            relationship_font_size_px: settings.relationship_label_style.font_size,
            retained_work_meter: Some(Arc::clone(work_meter)),
            _retained: Some(retained),
        })
    }

    pub(crate) fn entity(&self, entity_id: &str) -> Option<&ErEntityMeasure> {
        self.entity_measures.get(entity_id)
    }

    pub(crate) fn relationship(&self, index: usize) -> Option<&ErPreparedRelationshipLabel> {
        self.relationship_labels.get(index)
    }

    pub(crate) const fn entity_font_size_px(&self) -> f64 {
        self.entity_font_size_px
    }

    pub(crate) const fn attribute_font_size_px(&self) -> f64 {
        self.attribute_font_size_px
    }

    pub(crate) const fn relationship_font_size_px(&self) -> f64 {
        self.relationship_font_size_px
    }
}

#[cfg(test)]
pub(crate) fn prepare_er_labels(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Arc<ErPreparedLabels> {
    let settings = ErConfigView::new(effective_config)
        .layout_settings_with_font_family(&model.direction, None);
    let work_meter = Arc::new(OperationWorkMeter::new(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    Arc::new(
        ErPreparedLabels::prepare_for_layout(
            model,
            measurer,
            &settings,
            effective_config,
            &work_meter,
        )
        .expect("prepare ER labels"),
    )
}

fn er_swimlane_title_metrics(
    source: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    wrap_mode: WrapMode,
) -> TextMetrics {
    // Top-level lanes use labelRect semantics, without the table's generic transformation.
    let label = ErBoxLabel::from_source_with_generic_types(source, false);
    if label.rendered_text().is_empty() {
        return TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }

    match wrap_mode {
        WrapMode::HtmlLike => crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
            measurer,
            label.xhtml_fragment(),
            style,
            None,
            style.font_size,
        ),
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
            crate::text::measure_markdown_with_inline_styles(
                measurer,
                label.svg_text_source(),
                style,
                None,
                wrap_mode,
            )
        }
    }
}

fn er_subgraph_title_metrics(
    subgraph: &ErSubgraph,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    html_labels: bool,
) -> TextMetrics {
    let wrap_mode = if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    if subgraph.label_type == "string" || subgraph.label_type == "text" {
        measurer.measure_wrapped(&subgraph.title, style, None, wrap_mode)
    } else if html_labels {
        // Ordinary clusters project the raw title, without erBox's entity generic conversion.
        let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(&subgraph.title, true);
        crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
            measurer,
            &fragment,
            style,
            None,
            style.font_size,
        )
    } else {
        crate::text::measure_markdown_with_inline_styles(
            measurer,
            &subgraph.title,
            style,
            None,
            wrap_mode,
        )
    }
}

pub(crate) fn calculate_text_width_like_mermaid_px(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
) -> i64 {
    measure_mermaid_text_dimensions(measurer, text, style).width
}

#[derive(Debug)]
pub(crate) struct ErEntityMeasureRow {
    pub type_label: ErBoxLabel,
    pub name_label: ErBoxLabel,
    pub key_label: ErBoxLabel,
    pub comment_label: ErBoxLabel,
    pub height: f64,
    pub cell_widths: [f64; 4],
    pub cell_heights: [f64; 4],
}

#[derive(Debug)]
pub(crate) struct ErEntityMeasure {
    pub(crate) source_style: theme::ErEntitySourceStyle,
    pub width: f64,
    pub height: f64,
    pub text_padding: f64,
    pub label: ErBoxLabel,
    pub label_html_width: f64,
    pub label_height: f64,
    pub unformatted_label_metrics: TextMetrics,
    pub has_key: bool,
    pub has_comment: bool,
    pub type_col_w: f64,
    pub name_col_w: f64,
    pub key_col_w: f64,
    pub comment_col_w: f64,
    pub rows: Vec<ErEntityMeasureRow>,
    _retained: Option<PreparedTextRetainedReservation>,
}

impl ErEntityMeasure {
    fn retained_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>()
            .saturating_add(self.label.retained_buffer_bytes())
            .saturating_add(self.source_style.retained_declaration_bytes())
            .saturating_add(
                self.rows
                    .capacity()
                    .saturating_mul(std::mem::size_of::<ErEntityMeasureRow>()),
            );
        for row in &self.rows {
            for label in [
                &row.type_label,
                &row.name_label,
                &row.key_label,
                &row.comment_label,
            ] {
                bytes = bytes.saturating_add(label.retained_buffer_bytes());
            }
        }
        bytes
    }
}

#[cfg(test)]
pub(crate) fn measure_entity_box(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    settings: ErEntityMeasurementSettings,
) -> ErEntityMeasure {
    let work_meter = OperationWorkMeter::new(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    );
    measure_entity_box_with_source(
        entity,
        measurer,
        label_style,
        attr_style,
        label_style,
        settings,
        theme::compile_er_entity_source_style(entity, &indexmap::IndexMap::new()),
        &work_meter,
    )
    .expect("measure ER entity")
}

#[allow(
    clippy::too_many_arguments,
    reason = "ER entity preparation separates terminal styles, root width probes, and operation ownership."
)]
fn measure_entity_box_with_source(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    calculation_style: &TextStyle,
    settings: ErEntityMeasurementSettings,
    source_style: theme::ErEntitySourceStyle,
    work_meter: &OperationWorkMeter,
) -> Result<ErEntityMeasure> {
    // Measure through the same HTML/SVG branch used by Mermaid erBox.ts and the painter.
    // The operation-owned measurer supplies browser metrics or the deterministic fallback.

    // Mermaid's ER renderer (erBox.ts) uses `config.htmlLabels` inconsistently:
    // - It passes `useHtmlLabels: config.htmlLabels` into `createText`, where `undefined`
    //   effectively behaves as `true` due to JS default parameters.
    // - It uses `if (!config.htmlLabels) { PADDING *= 1.25; TEXT_PADDING *= 1.25; }`, where
    //   `undefined` behaves as `false` and triggers the multiplier even when HTML labels are used.
    //
    // Upstream SVG fixtures at Mermaid@11.12.2 reflect this quirk. The padding multiplier still
    // keys off the raw truthiness (`undefined` behaves like `false`) even though the rendered labels
    // use HTML `<foreignObject>` output by default.
    let html_labels_raw = settings.html_labels_raw;

    // Mermaid ER unified shape (`erBox.ts`) uses:
    // - PADDING = config.er.diagramPadding (default 20 in Mermaid 11.12.2 schema defaults)
    // - TEXT_PADDING = config.er.entityPadding (default 15)
    let mut padding = settings.diagram_padding;
    let mut text_padding = settings.entity_padding;
    let min_w = settings.min_entity_width;
    let wrapping_width_px = settings.wrapping_width_px;

    // Attribute-free entities use drawRect/labelHelper, which does not call addText's
    // parseGenericTypes workaround.
    let mut label = ErBoxLabel::from_source_with_generic_types(
        entity.label_source_for_render(),
        !entity.attributes.is_empty(),
    );
    let label_probe_style = source_style.text_style(label_style, label_style.font_size);
    let attr_probe_style = source_style.text_style(attr_style, label_style.font_size);
    let label_probe_size = source_style.terminal_font_size(Some(label_style.font_size));
    let attr_probe_size = source_style.terminal_font_size(Some(attr_style.font_size));

    // No attributes: use `drawRect`-like padding rules from Mermaid erBox.ts.
    if entity.attributes.is_empty() {
        let label_pad_x = padding;
        let label_pad_y = padding * 1.5;
        // Mermaid's `drawRect` branch clamps to `minEntityWidth` based on `calculateTextWidth()`,
        // not on the HTML label bbox. Preserve that quirk: upstream can end up with nodes that are
        // narrower than `minEntityWidth` when `calculateTextWidth()` is larger than the HTML bbox
        // used by `drawRect`.
        let calc_w = calculate_text_width_like_mermaid_px(
            measurer,
            calculation_style,
            label.markdown_input(),
        );
        let clamp_to_min_w = (calc_w as f64 + label_pad_x * 2.0) < min_w;
        let label_max_width_px = if clamp_to_min_w {
            min_w.round().max(0.0) as i64
        } else {
            wrapping_width_px
        };
        let label_metrics = label.prepare_metrics(
            measurer,
            &label_probe_style,
            &source_style,
            label_max_width_px,
            settings.label_wrap_mode,
            label_style.font_size,
            label_probe_size,
            work_meter,
        )?;
        let label_html_width = label_metrics.width.max(0.0);
        // The painter emits this same prepared terminal. Reusing its metrics preserves
        // entity depth and avoids measuring encoded source as already-decoded text.
        let unformatted_label_metrics = label_metrics;
        let width = if clamp_to_min_w {
            min_w.max(label_html_width + label_pad_x * 2.0)
        } else {
            label_html_width + label_pad_x * 2.0
        };
        let height = label_metrics.height + label_pad_y * 2.0;
        return Ok(ErEntityMeasure {
            source_style,
            width: width.max(1.0),
            height: height.max(1.0),
            text_padding,
            label,
            label_html_width,
            label_height: label_metrics.height.max(0.0),
            unformatted_label_metrics,
            has_key: false,
            has_comment: false,
            type_col_w: 0.0,
            name_col_w: 0.0,
            key_col_w: 0.0,
            comment_col_w: 0.0,
            rows: Vec::new(),
            _retained: None,
        });
    }

    let label_text_width_px =
        calculate_text_width_like_mermaid_px(measurer, calculation_style, label.markdown_input());
    let label_metrics = label.prepare_metrics(
        measurer,
        &label_probe_style,
        &source_style,
        label_text_width_px.saturating_add(100),
        settings.label_wrap_mode,
        label_style.font_size,
        label_probe_size,
        work_meter,
    )?;
    let label_html_width = label_metrics.width.max(0.0);

    // Mermaid erBox.ts only applies the `* 1.25` multiplier after the "drawRect" early-return.
    // Keep that behavior: nodes without an attribute table should *not* inherit the multiplier.
    if !html_labels_raw {
        padding *= 1.25;
        text_padding *= 1.25;
    }

    let mut rows: Vec<ErEntityMeasureRow> = Vec::new();

    let mut max_type_raw_w: f64 = 0.0;
    let mut max_name_raw_w: f64 = 0.0;
    let mut max_keys_raw_w: f64 = 0.0;
    let mut max_comment_raw_w: f64 = 0.0;

    let mut max_type_col_w: f64 = 0.0;
    let mut max_name_col_w: f64 = 0.0;
    let mut max_keys_col_w: f64 = 0.0;
    let mut max_comment_col_w: f64 = 0.0;

    let mut total_rows_h = 0.0;

    for a in &entity.attributes {
        work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
        let mut type_label = ErBoxLabel::from_source(&a.ty);
        let mut name_label = ErBoxLabel::from_source(&a.name);
        let mut key_label = ErBoxLabel::from_source(&a.keys.join(","));
        let mut comment_label = ErBoxLabel::from_source(&a.comment);
        let cell_max_widths_px =
            [&type_label, &name_label, &key_label, &comment_label].map(|label| {
                calculate_text_width_like_mermaid_px(
                    measurer,
                    calculation_style,
                    label.markdown_input(),
                )
                .saturating_add(100)
            });
        let [type_m, name_m, keys_m, comment_m] = [
            (&mut type_label, cell_max_widths_px[0]),
            (&mut name_label, cell_max_widths_px[1]),
            (&mut key_label, cell_max_widths_px[2]),
            (&mut comment_label, cell_max_widths_px[3]),
        ]
        .map(|(label, width)| {
            label.prepare_metrics(
                measurer,
                &attr_probe_style,
                &source_style,
                width,
                settings.label_wrap_mode,
                label_style.font_size,
                attr_probe_size,
                work_meter,
            )
        });
        let (type_m, name_m, keys_m, comment_m) = (type_m?, name_m?, keys_m?, comment_m?);

        let type_w = type_m.width;
        let name_w = name_m.width;
        max_type_raw_w = max_type_raw_w.max(type_w);
        max_name_raw_w = max_name_raw_w.max(name_w);
        max_type_col_w = max_type_col_w.max(type_w + padding);
        max_name_col_w = max_name_col_w.max(name_w + padding);

        let keys_w = keys_m.width;
        max_keys_raw_w = max_keys_raw_w.max(keys_w);
        max_keys_col_w = max_keys_col_w.max(keys_w + padding);

        let comment_w = comment_m.width;
        max_comment_raw_w = max_comment_raw_w.max(comment_w);
        max_comment_col_w = max_comment_col_w.max(comment_w + padding);

        let row_h = type_m
            .height
            .max(name_m.height)
            .max(keys_m.height)
            .max(comment_m.height)
            + text_padding;

        rows.push(ErEntityMeasureRow {
            type_label,
            name_label,
            key_label,
            comment_label,
            height: row_h.max(1.0),
            cell_widths: [type_w, name_w, keys_w, comment_w].map(|width| width.max(0.0)),
            cell_heights: [
                type_m.height,
                name_m.height,
                keys_m.height,
                comment_m.height,
            ]
            .map(|height| height.max(0.0)),
        });
        total_rows_h += row_h.max(1.0);
    }

    let mut total_width_sections = 4usize;
    let mut has_key = true;
    let mut has_comment = true;
    if max_keys_col_w <= padding {
        has_key = false;
        max_keys_col_w = 0.0;
        total_width_sections = total_width_sections.saturating_sub(1);
    }
    if max_comment_col_w <= padding {
        has_comment = false;
        max_comment_col_w = 0.0;
        total_width_sections = total_width_sections.saturating_sub(1);
    }

    // Mermaid adds extra padding to attribute components to accommodate the entity name width.
    // Mermaid uses the HTML label bbox (`getBoundingClientRect`) as `nameBBox.width`.
    let name_w_min = label_html_width + padding * 2.0;
    let mut max_width = max_type_col_w + max_name_col_w + max_keys_col_w + max_comment_col_w;
    if name_w_min - max_width > 0.0 && total_width_sections > 0 {
        let diff = name_w_min - max_width;
        let per = diff / total_width_sections as f64;
        max_type_col_w += per;
        max_name_col_w += per;
        if has_key {
            max_keys_col_w += per;
        }
        if has_comment {
            max_comment_col_w += per;
        }
        max_width = max_type_col_w + max_name_col_w + max_keys_col_w + max_comment_col_w;
    }

    let shape_bbox_w = label_html_width
        .max(max_type_raw_w)
        .max(max_name_raw_w)
        .max(max_keys_raw_w)
        .max(max_comment_raw_w);

    let width = (shape_bbox_w + padding * 2.0).max(max_width);
    let name_h = label_metrics.height + text_padding;
    let height = total_rows_h + name_h;
    Ok(ErEntityMeasure {
        source_style,
        width: width.max(1.0),
        height: height.max(1.0),
        text_padding,
        label,
        label_html_width,
        label_height: label_metrics.height.max(0.0),
        unformatted_label_metrics: label_metrics,
        has_key,
        has_comment,
        type_col_w: max_type_col_w.max(0.0),
        name_col_w: max_name_col_w.max(0.0),
        key_col_w: max_keys_col_w.max(0.0),
        comment_col_w: max_comment_col_w.max(0.0),
        rows,
        _retained: None,
    })
}

fn parse_er_rel_idx_from_edge_name(name: &str) -> Option<usize> {
    let rest = name.strip_prefix("er-rel-")?;
    let mut end = 0usize;
    for (idx, ch) in rest.char_indices() {
        if !ch.is_ascii_digit() {
            break;
        }
        end = idx + ch.len_utf8();
    }
    if end == 0 {
        return None;
    }
    rest[..end].parse::<usize>().ok()
}

fn is_er_self_loop_dummy_node_id(id: &str) -> bool {
    // Mermaid's dagre renderer creates self-loop helper nodes using `${nodeId}---${nodeId}---{1|2}`.
    let Some((base, suffix)) = id.rsplit_once("---") else {
        return false;
    };
    if !matches!(suffix, "1" | "2") {
        return false;
    }
    let Some((left, right)) = base.split_once("---") else {
        return false;
    };
    left == right
}

#[derive(Debug, Clone)]
struct LayoutEdgeParts {
    id: String,
    from: String,
    to: String,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
    start_marker: Option<String>,
    end_marker: Option<String>,
    stroke_dasharray: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErSelfLoopSide {
    Top,
    Bottom,
    Left,
    Right,
}

fn er_default_self_loop_side(rankdir: RankDir) -> ErSelfLoopSide {
    match rankdir {
        RankDir::BT => ErSelfLoopSide::Bottom,
        RankDir::LR => ErSelfLoopSide::Right,
        RankDir::RL => ErSelfLoopSide::Left,
        RankDir::TB => ErSelfLoopSide::Top,
    }
}

fn er_self_loop_side(
    node: &NodeLabel,
    hints: impl IntoIterator<Item = (f64, f64)>,
    rankdir: RankDir,
) -> ErSelfLoopSide {
    let hints = hints.into_iter().collect::<Vec<_>>();
    if hints.is_empty() {
        return er_default_self_loop_side(rankdir);
    }

    let (sum_x, sum_y) = hints.iter().fold((0.0, 0.0), |(x, y), (hint_x, hint_y)| {
        (x + hint_x, y + hint_y)
    });
    let count = hints.len() as f64;
    let dx = sum_x / count - node.x.unwrap_or(0.0);
    let dy = sum_y / count - node.y.unwrap_or(0.0);
    if dx.abs() > dy.abs() {
        if dx > 0.0 {
            ErSelfLoopSide::Right
        } else {
            ErSelfLoopSide::Left
        }
    } else if dy > 0.0 {
        ErSelfLoopSide::Bottom
    } else if dy < 0.0 {
        ErSelfLoopSide::Top
    } else {
        er_default_self_loop_side(rankdir)
    }
}

fn er_self_loop_points(
    node: &NodeLabel,
    side: ErSelfLoopSide,
    label_width: f64,
) -> Vec<LayoutPoint> {
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let half_width = node.width / 2.0;
    let half_height = node.height / 2.0;
    let max_span = (node.width * 0.8).clamp(36.0, 100.0);
    let span = label_width.max(node.width * 0.35).clamp(36.0, max_span);
    let depth = node
        .width
        .min(node.height)
        .mul_add(0.45, 0.0)
        .clamp(24.0, 48.0);

    let mut points = match side {
        ErSelfLoopSide::Bottom => vec![
            LayoutPoint {
                x: x - span / 2.0,
                y: y + half_height,
            },
            LayoutPoint {
                x: x - span / 2.0,
                y: y + half_height + depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y + half_height + depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y + half_height,
            },
        ],
        ErSelfLoopSide::Right => vec![
            LayoutPoint {
                x: x + half_width,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x + half_width + depth,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x + half_width + depth,
                y: y + span / 2.0,
            },
            LayoutPoint {
                x: x + half_width,
                y: y + span / 2.0,
            },
        ],
        ErSelfLoopSide::Left => vec![
            LayoutPoint {
                x: x - half_width,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x - half_width - depth,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x - half_width - depth,
                y: y + span / 2.0,
            },
            LayoutPoint {
                x: x - half_width,
                y: y + span / 2.0,
            },
        ],
        ErSelfLoopSide::Top => vec![
            LayoutPoint {
                x: x - span / 2.0,
                y: y - half_height,
            },
            LayoutPoint {
                x: x - span / 2.0,
                y: y - half_height - depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y - half_height - depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y - half_height,
            },
        ],
    };

    // Mermaid's edge painter clips the first/last points against the node by shooting a ray from
    // the node center toward the adjacent inner bend. Keep the compact route's inner points
    // intact, but expose the same boundary intersections in the public LayoutEdge so the
    // headless SVG path/data-points match the browser renderer.
    if points.len() >= 4 {
        let first_inner = points[1].clone();
        let last_index = points.len() - 1;
        let last_inner = points[last_index - 1].clone();
        points[0] = er_intersect_node_rect(node, &first_inner);
        let last_boundary = er_intersect_node_rect(node, &last_inner);
        points[last_index] = last_boundary;
    }
    points
}

fn er_intersect_node_rect(node: &NodeLabel, point: &LayoutPoint) -> LayoutPoint {
    // Port of Mermaid's intersect.rect: the ray starts at the node center and ends at the
    // requested route point. This deliberately handles the self-loop endpoint case separately
    // from `clip_edge_endpoints`, whose segment clipping assumes the point is already inside the
    // rectangle and would leave raw boundary coordinates unchanged.
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let dx = point.x - x;
    let dy = point.y - y;
    let mut half_width = node.width / 2.0;
    let mut half_height = node.height / 2.0;

    let (sx, sy) = if dy.abs() * half_width > dx.abs() * half_height {
        if dy < 0.0 {
            half_height = -half_height;
        }
        let sx = if dy == 0.0 {
            0.0
        } else {
            (half_height * dx) / dy
        };
        (sx, half_height)
    } else {
        if dx < 0.0 {
            half_width = -half_width;
        }
        let sy = if dx == 0.0 {
            0.0
        } else {
            (half_width * dy) / dx
        };
        (half_width, sy)
    };

    LayoutPoint {
        x: x + sx,
        y: y + sy,
    }
}

fn er_self_loop_label_position(
    points: &[LayoutPoint],
    side: ErSelfLoopSide,
    label_width: f64,
    label_height: f64,
    node: &NodeLabel,
) -> (f64, f64) {
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let gap = 4.0;
    match side {
        ErSelfLoopSide::Bottom => (
            x,
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max)
                + label_height / 2.0
                + gap,
        ),
        ErSelfLoopSide::Right => (
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max)
                + label_width / 2.0
                + gap,
            y,
        ),
        ErSelfLoopSide::Left => (
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min)
                - label_width / 2.0
                - gap,
            y,
        ),
        ErSelfLoopSide::Top => (
            x,
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min)
                - label_height / 2.0
                - gap,
        ),
    }
}

// Layout-engine fallback used before SVG path insertion. This deliberately stays outside the SVG
// parity layer: rendered-path change detection and five-decimal placement belong to
// `svg::parity::edge_label_geometry`, while this function only supplies a missing Dagre/ELK anchor.
fn calc_label_position(points: &[LayoutPoint]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some((points[0].x, points[0].y));
    }

    let mut total = 0.0;
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        let dy = points[i].y - points[i - 1].y;
        total += (dx * dx + dy * dy).sqrt();
    }
    let mut remaining = total / 2.0;
    for i in 1..points.len() {
        let p0 = &points[i - 1];
        let p1 = &points[i];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let seg = (dx * dx + dy * dy).sqrt();
        if seg == 0.0 {
            continue;
        }
        if seg < remaining {
            remaining -= seg;
            continue;
        }
        let t = (remaining / seg).clamp(0.0, 1.0);
        return Some((p0.x + t * dx, p0.y + t * dy));
    }
    Some((points.last()?.x, points.last()?.y))
}

type Rect = crate::geom::Box2;

fn intersect_segment_with_rect(
    p0: &LayoutPoint,
    p1: &LayoutPoint,
    rect: Rect,
) -> Option<LayoutPoint> {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }

    let mut candidates: Vec<(f64, LayoutPoint)> = Vec::new();
    let eps = 1e-9;
    let min_x = rect.min_x();
    let max_x = rect.max_x();
    let min_y = rect.min_y();
    let max_y = rect.max_y();

    if dx.abs() > eps {
        for x_edge in [min_x, max_x] {
            let t = (x_edge - p0.x) / dx;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let y = p0.y + t * dy;
            if y + eps >= min_y && y <= max_y + eps {
                candidates.push((t, LayoutPoint { x: x_edge, y }));
            }
        }
    }

    if dy.abs() > eps {
        for y_edge in [min_y, max_y] {
            let t = (y_edge - p0.y) / dy;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let x = p0.x + t * dx;
            if x + eps >= min_x && x <= max_x + eps {
                candidates.push((t, LayoutPoint { x, y: y_edge }));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates
        .into_iter()
        .find(|(t, _)| *t >= 0.0)
        .map(|(_, p)| p)
}

fn clip_edge_endpoints(points: &mut [LayoutPoint], from: Rect, to: Rect) {
    if points.len() < 2 {
        return;
    }
    if from.contains_point(points[0].x, points[0].y)
        && let Some(p) = intersect_segment_with_rect(&points[0], &points[1], from)
    {
        points[0] = p;
    }
    let last = points.len() - 1;
    if to.contains_point(points[last].x, points[last].y)
        && let Some(p) = intersect_segment_with_rect(&points[last], &points[last - 1], to)
    {
        points[last] = p;
    }
}

fn er_marker_id(card: &str, suffix: &str) -> Option<String> {
    match card {
        "ONLY_ONE" => Some(format!("ONLY_ONE_{suffix}")),
        "ZERO_OR_ONE" => Some(format!("ZERO_OR_ONE_{suffix}")),
        "ONE_OR_MORE" => Some(format!("ONE_OR_MORE_{suffix}")),
        "ZERO_OR_MORE" => Some(format!("ZERO_OR_MORE_{suffix}")),
        // Mermaid CLI ER output does not emit a dedicated MD_PARENT marker.
        "MD_PARENT" => None,
        _ => None,
    }
}

/// Lays out an ER model with an optional renderer-owned inherited typography override.
pub(crate) fn layout_er_diagram_typed_with_resolved_typography(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    #[cfg(feature = "layout-elk")] operation_seed: elk::ElkOperationSeed,
    font_family_css: Option<&str>,
    font_size_px: Option<f64>,
    work_meter: Arc<crate::resources::OperationWorkMeter>,
) -> Result<ErDiagramLayout> {
    let mut work_control = OperationLayoutWorkControl::new(Arc::clone(&work_meter));
    let settings = ErConfigView::new(effective_config).layout_settings_with_resolved_typography(
        &model.direction,
        font_family_css,
        font_size_px,
    );
    let adapter_work = er_layout_adapter_work(model, &work_control)?;
    work_control.charge_adapter(adapter_work)?;
    validate_er_relationship_endpoints(model)?;
    let prepared_labels = Arc::new(ErPreparedLabels::prepare_for_layout(
        model,
        measurer,
        &settings,
        effective_config,
        &work_meter,
    )?);
    let selection = crate::layout_backend::resolve_er_graph_layout(effective_config);
    selection.validate_rootless_graph()?;

    if selection.backend == GraphLayoutBackend::Swimlane {
        return swimlane::layout_er_diagram_swimlane_typed(
            model,
            effective_config,
            measurer,
            &settings,
            prepared_labels,
            work_meter,
        );
    }

    #[cfg(feature = "layout-elk")]
    if selection.backend == GraphLayoutBackend::Elk {
        return layout_er_diagram_elk_typed(
            model,
            effective_config,
            measurer,
            settings,
            Arc::clone(&prepared_labels),
            operation_seed,
            &mut work_control,
        );
    }

    layout_er_diagram_dagre_typed(
        model,
        measurer,
        settings,
        prepared_labels,
        &mut work_control,
    )
}

fn validate_er_relationship_endpoints(
    model: &merman_model::families::er::ErDiagramModel,
) -> Result<()> {
    let mut node_ids: HashSet<&str> = model
        .entities
        .values()
        .map(|entity| entity.id.as_str())
        .collect();
    node_ids.extend(model.subgraphs.iter().map(|subgraph| subgraph.id.as_str()));
    for relationship in &model.relationships {
        // The render model indexes entities by source name, while relationships store the
        // renderer-facing generated `entity-*` ids.  Validate against the entity values rather
        // than the map keys; checking the keys would reject every ordinary parsed relationship
        // before Dagre/ELK gets a chance to lay it out.
        if !node_ids.contains(relationship.entity_a.as_str())
            || !node_ids.contains(relationship.entity_b.as_str())
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    relationship.entity_a, relationship.entity_b
                ),
            });
        }
    }
    Ok(())
}

fn er_layout_adapter_work(
    model: &merman_model::families::er::ErDiagramModel,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    let attribute_count = model.entities.values().try_fold(0usize, |count, entity| {
        work_control.checked_add(count, entity.attributes.len())
    })?;
    let entity_work = work_control.checked_mul(model.entities.len(), 12)?;
    let attribute_work = work_control.checked_mul(attribute_count, 6)?;
    let relationship_work = work_control.checked_mul(model.relationships.len(), 10)?;
    let class_work = work_control.checked_mul(model.classes.len(), 3)?;
    let subgraph_membership = model.subgraphs.iter().try_fold(0usize, |total, subgraph| {
        work_control.checked_add(total, subgraph.nodes.len())
    })?;
    let subgraph_work = work_control.checked_add(
        work_control.checked_mul(model.subgraphs.len(), 12)?,
        subgraph_membership,
    )?;
    work_control.checked_add(
        work_control.checked_add(entity_work, attribute_work)?,
        work_control.checked_add(
            work_control.checked_add(relationship_work, class_work)?,
            subgraph_work,
        )?,
    )
}

fn layout_er_diagram_dagre_typed(
    model: &merman_model::families::er::ErDiagramModel,
    measurer: &dyn TextMeasurer,
    settings: ErLayoutSettings,
    prepared_labels: Arc<ErPreparedLabels>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<ErDiagramLayout> {
    let membership_count = model.subgraphs.iter().try_fold(0usize, |count, subgraph| {
        work_control.checked_add(count, subgraph.nodes.len())
    })?;
    let helper_count = work_control.checked_mul(
        model
            .relationships
            .iter()
            .filter(|r| r.entity_a == r.entity_b)
            .count(),
        2,
    )?;
    let parent_count = work_control.checked_add(membership_count, helper_count)?;
    if parent_count != 0 {
        let slots = work_control.checked_add(
            work_control.checked_add(model.entities.len(), model.subgraphs.len())?,
            helper_count,
        )?;
        // Only Dagre builds this parent plan. Admit its scratch, checked forest batch and
        // numeric-key updates conservatively before allocating any of that derived graph.
        let height = usize::BITS as usize - slots.saturating_sub(1).leading_zeros() as usize + 1;
        let parent_work = work_control.checked_mul(
            work_control.checked_add(slots, parent_count)?,
            work_control.checked_mul(height, 12)?,
        )?;
        work_control.charge_adapter(parent_work)?;
    }

    let ErLayoutSettings {
        graph: graph_label,
        label_style,
        attr_style: _,
        calculation_style: _,
        relationship_label_style: _,
        relationship_html_labels,
        entity_measurement: _,
    } = settings;

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        // Mermaid's Dagre adapter always enables `compound: true`, even without clusters.
        compound: true,
    });
    let rankdir = graph_label.rankdir;
    g.set_graph(graph_label);

    fn parse_entity_counter_from_id(id: &str) -> Option<usize> {
        let (_prefix, tail) = id.rsplit_once('-')?;
        tail.parse::<usize>().ok()
    }

    // Groups are first-class compound nodes in Mermaid's ER graph. Keep the source names in the
    // semantic model and translate entity members to their renderer-facing `entity-*` ids only
    // at this boundary.
    let subgraph_ids: HashSet<&str> = model
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect();
    let entity_id_by_name: HashMap<&str, &str> = model
        .entities
        .iter()
        .map(|(name, entity)| (name.as_str(), entity.id.as_str()))
        .collect();
    let entity_name_by_id: HashMap<&str, &str> = model
        .entities
        .iter()
        .map(|(name, entity)| (entity.id.as_str(), name.as_str()))
        .collect();
    let mut subgraph_title_metrics: HashMap<String, (f64, f64)> = HashMap::new();

    // Insert groups in reverse declaration order, matching Mermaid's `getData()` projection and
    // keeping nested group order deterministic for Dagre's compound ranking.
    for subgraph in model.subgraphs.iter().rev() {
        let metrics =
            er_subgraph_title_metrics(subgraph, measurer, &label_style, relationship_html_labels);
        let has_children = subgraph.nodes.iter().any(|member| {
            subgraph_ids.contains(member.as_str())
                || entity_id_by_name.contains_key(member.as_str())
        });
        let node_label = if has_children {
            NodeLabel::default()
        } else {
            NodeLabel {
                width: (metrics.width + 16.0).max(16.0),
                height: (metrics.height + 16.0).max(16.0),
                ..Default::default()
            }
        };
        subgraph_title_metrics.insert(
            subgraph.id.clone(),
            (metrics.width.max(0.0), metrics.height.max(0.0)),
        );
        g.set_node(subgraph.id.clone(), node_label);
    }

    // Nodes.
    let mut entities_in_layout_order: Vec<&ErEntity> = model.entities.values().collect();
    entities_in_layout_order.sort_by(|a, b| {
        let a_key = (parse_entity_counter_from_id(&a.id), a.id.as_str());
        let b_key = (parse_entity_counter_from_id(&b.id), b.id.as_str());
        a_key.cmp(&b_key)
    });

    for e in entities_in_layout_order {
        // Mermaid's `getData()` omits an entity whose source name collides with a subgraph id.
        if entity_name_by_id
            .get(e.id.as_str())
            .is_some_and(|name| subgraph_ids.contains(*name))
        {
            continue;
        }
        let measure = prepared_labels
            .entity(&e.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared labels are missing entity {}", e.id),
            })?;
        g.set_node(
            e.id.clone(),
            NodeLabel {
                width: measure.width,
                height: measure.height,
                ..Default::default()
            },
        );
    }

    // Retain the last membership assignment in its original mutation order. Graphlib's checked
    // batch requires each child once; replaying only its last assignment preserves reparenting and
    // ordinary child-key order without a potentially quadratic ancestor scan per declaration.
    let mut parent_assignments = Vec::new();
    let mut last_parent_assignment = HashMap::new();
    for subgraph in &model.subgraphs {
        for member in &subgraph.nodes {
            let child_id = if subgraph_ids.contains(member.as_str()) {
                member.clone()
            } else if let Some(entity_id) = entity_id_by_name.get(member.as_str()) {
                (*entity_id).to_string()
            } else {
                continue;
            };
            if child_id != subgraph.id
                && let (Some(child), Some(parent)) = (g.node_ix(&child_id), g.node_ix(&subgraph.id))
            {
                last_parent_assignment.insert(child, parent_assignments.len());
                parent_assignments.push((child, parent));
            }
        }
    }

    // Edges. Mermaid ER uses edge labels ("roleA") and the unified renderer routes through the
    // generic dagre pipeline, which accounts for label bbox in spacing. Mirror that by giving
    // dagre real label sizes here.
    for (idx, r) in model.relationships.iter().enumerate() {
        if g.node(&r.entity_a).is_none() || g.node(&r.entity_b).is_none() {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    r.entity_a, r.entity_b
                ),
            });
        }

        // Mermaid's dagre renderer splits self-loops into three edges and introduces two helper
        // nodes (labelRect). Mermaid initializes them at 10x10, but after `updateNodeBounds(...)`
        // an empty labelRect collapses to ~0.1x0.1 and that is what Dagre uses for spacing.
        // Match that here for layout parity.
        if r.entity_a == r.entity_b {
            let node_id = r.entity_a.as_str();
            let special_1 = format!("{node_id}---{node_id}---1");
            let special_2 = format!("{node_id}---{node_id}---2");
            let parent = g
                .node_ix(node_id)
                .and_then(|child| last_parent_assignment.get(&child))
                .map(|&index| parent_assignments[index].1);

            if g.node(&special_1).is_none() {
                g.set_node(
                    special_1.clone(),
                    NodeLabel {
                        width: 0.1,
                        height: 0.1,
                        ..Default::default()
                    },
                );
            }
            if g.node(&special_2).is_none() {
                g.set_node(
                    special_2.clone(),
                    NodeLabel {
                        width: 0.1,
                        height: 0.1,
                        ..Default::default()
                    },
                );
            }
            if let Some(parent) = parent {
                for helper in [&special_1, &special_2] {
                    if let Some(child) = g.node_ix(helper) {
                        last_parent_assignment.insert(child, parent_assignments.len());
                        parent_assignments.push((child, parent));
                    }
                }
            }

            let (label_w, label_h) = prepared_labels
                .relationship(idx)
                .map(ErPreparedRelationshipLabel::dimensions)
                .unwrap_or_default();

            // First segment: keep start marker, no label.
            g.set_edge_named(
                r.entity_a.clone(),
                special_1.clone(),
                Some(format!("er-rel-{idx}-cyclic-0")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Mid segment: carries the relationship label, no markers.
            g.set_edge_named(
                special_1.clone(),
                special_2.clone(),
                Some(format!("er-rel-{idx}")),
                Some(EdgeLabel {
                    width: label_w.max(0.0),
                    height: label_h.max(0.0),
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Last segment: keep end marker, no label.
            g.set_edge_named(
                special_2.clone(),
                r.entity_a.clone(),
                Some(format!("er-rel-{idx}-cyclic-2")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            continue;
        }

        let name = format!("er-rel-{idx}");
        let (label_w, label_h) = prepared_labels
            .relationship(idx)
            .map(ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        g.set_edge_named(
            r.entity_a.clone(),
            r.entity_b.clone(),
            Some(name),
            Some(EdgeLabel {
                width: label_w.max(0.0),
                height: label_h.max(0.0),
                labelpos: LabelPos::C,
                labeloffset: 10.0,
                minlen: 1,
                weight: 1.0,
                ..Default::default()
            }),
        );
    }

    // The adapter reservation precedes this scratch and all helper expansion. Check the sticky
    // operation terminal again after label measurement, before reporting an invalid forest.
    work_control.checked_add(0, 0)?;
    let mut ordinal = 0;
    parent_assignments.retain(|(child, _)| {
        let keep = last_parent_assignment.get(child) == Some(&ordinal);
        ordinal += 1;
        keep
    });
    if let Err(error) = g.try_set_unparented_parents_ix(&parent_assignments) {
        let message = match error {
            dugong::graphlib::GraphError::ParentCycle {
                child_ix,
                parent_ix,
            } => format!(
                "ER parent cycle setting {} as parent of {}",
                g.node_id_by_ix(parent_ix).unwrap_or("<removed>"),
                g.node_id_by_ix(child_ix).unwrap_or("<removed>"),
            ),
            error => format!("invalid ER parent assignments: {error}"),
        };
        return Err(Error::InvalidModel { message });
    }

    dugong::layout_controlled(&mut g, work_control)
        .map_err(|error| work_control.map_dugong_error(error))?;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    let mut clusters = Vec::new();
    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
            continue;
        };
        let is_cluster = subgraph_ids.contains(id.as_str());
        let x = n.x.unwrap_or(0.0);
        let y = n.y.unwrap_or(0.0);
        let mut width = n.width.max(1.0);
        let mut height = n.height.max(1.0);
        if is_cluster {
            let (title_width, title_height) = subgraph_title_metrics
                .get(&id)
                .copied()
                .unwrap_or((0.0, 0.0));
            // Mermaid's ER group renderer keeps an 8px title inset and widens empty groups to
            // contain their title. Compound Dagre groups already include their children; only
            // the title-driven minimum is added here.
            width = width.max(title_width + 16.0);
            height = height.max(title_height + 16.0);
            if let Some(subgraph) = model.subgraphs.iter().find(|subgraph| subgraph.id == id) {
                let padding = 8.0;
                let title_label = LayoutLabel {
                    x,
                    y: y - height / 2.0 + padding + title_height / 2.0,
                    width: title_width,
                    height: title_height,
                };
                let effective_dir = subgraph
                    .dir
                    .clone()
                    .unwrap_or_else(|| model.direction.clone());
                clusters.push(crate::model::LayoutCluster {
                    id: id.clone(),
                    x,
                    y,
                    width,
                    height,
                    diff: (title_width - width) / 2.0 - padding / 2.0,
                    offset_y: title_height - padding / 2.0,
                    title: subgraph.title.clone(),
                    title_label,
                    requested_dir: subgraph.dir.clone(),
                    effective_dir,
                    padding,
                    title_margin_top: 0.0,
                    title_margin_bottom: 0.0,
                });
            }
        }
        nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster,
            label_width: None,
            label_height: None,
        });
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut node_rect_by_id: HashMap<String, Rect> = HashMap::new();
    for n in &nodes {
        node_rect_by_id.insert(n.id.clone(), Rect::from_center(n.x, n.y, n.width, n.height));
    }

    let mut edges_by_id: HashMap<String, LayoutEdgeParts> = HashMap::new();
    for key in g.edge_keys() {
        let Some(e) = g.edge_by_key(&key) else {
            continue;
        };
        let mut points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let id = key
            .name
            .clone()
            .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w));

        let rel_idx = key
            .name
            .as_ref()
            .and_then(|name| parse_er_rel_idx_from_edge_name(name))
            .and_then(|idx| model.relationships.get(idx).map(|_| idx));

        let rel = rel_idx.and_then(|idx| model.relationships.get(idx));
        let (base_start_marker, base_end_marker, stroke_dasharray) = if let Some(rel) = rel {
            let card_a = rel.rel_spec.card_a.as_str();
            let card_b = rel.rel_spec.card_b.as_str();
            let rel_type = rel.rel_spec.rel_type.as_str();
            let start_marker = er_marker_id(card_b, "START");
            let end_marker = er_marker_id(card_a, "END");
            let stroke_dasharray = if rel_type == "NON_IDENTIFYING" {
                Some("8,8".to_string())
            } else {
                None
            };
            (start_marker, end_marker, stroke_dasharray)
        } else {
            (None, None, None)
        };

        if !is_er_self_loop_dummy_node_id(&key.v)
            && !is_er_self_loop_dummy_node_id(&key.w)
            && let (Some(from_rect), Some(to_rect)) = (
                node_rect_by_id.get(&key.v).copied(),
                node_rect_by_id.get(&key.w).copied(),
            )
        {
            clip_edge_endpoints(&mut points, from_rect, to_rect);
        }

        let (start_marker, end_marker) =
            if is_er_self_loop_dummy_node_id(&key.v) && is_er_self_loop_dummy_node_id(&key.w) {
                (None, None)
            } else if id.ends_with("-cyclic-0") {
                (base_start_marker, None)
            } else if id.ends_with("-cyclic-2") {
                (None, base_end_marker)
            } else {
                (base_start_marker, base_end_marker)
            };

        let (label_width, label_height) = rel_idx
            .and_then(|index| prepared_labels.relationship(index))
            .map(ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        let label = if label_width <= 0.0
            || label_height <= 0.0
            || id.ends_with("-cyclic-0")
            || id.ends_with("-cyclic-2")
        {
            None
        } else {
            // Mermaid uses Dagre's computed edge label center (`edge.x/edge.y`) rather than a
            // polyline midpoint. Prefer those coordinates when present.
            let (x, y) =
                e.x.zip(e.y)
                    .or_else(|| calc_label_position(&points))
                    .unwrap_or((0.0, 0.0));
            Some(LayoutLabel {
                x,
                y,
                width: label_width,
                height: label_height,
            })
        };

        edges_by_id.insert(
            id,
            LayoutEdgeParts {
                id: key
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w)),
                from: key.v.clone(),
                to: key.w.clone(),
                points,
                label,
                start_marker,
                end_marker,
                stroke_dasharray,
            },
        );
    }

    // Keep the Dagre graph projection intact for layout artifacts. Mermaid's common painter
    // consumes a second projection where self-loop segments are merged into one visible edge;
    // build that projection below without discarding the helper ranks from the canonical layout.
    let to_layout_edge = |edge: LayoutEdgeParts| LayoutEdge {
        id: edge.id,
        from: edge.from,
        to: edge.to,
        from_cluster: None,
        to_cluster: None,
        points: edge.points,
        label: edge.label,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: edge.start_marker,
        end_marker: edge.end_marker,
        stroke_dasharray: edge.stroke_dasharray,
    };

    let mut layout_edges = edges_by_id
        .values()
        .cloned()
        .map(to_layout_edge)
        .collect::<Vec<_>>();
    layout_edges.sort_by(|left, right| left.id.cmp(&right.id));

    let mut render_edges: Vec<LayoutEdge> = Vec::with_capacity(model.relationships.len());
    for (idx, relationship) in model.relationships.iter().enumerate() {
        let edge_id = format!("er-rel-{idx}");
        if relationship.entity_a == relationship.entity_b {
            let first_id = format!("{edge_id}-cyclic-0");
            let last_id = format!("{edge_id}-cyclic-2");
            let first = edges_by_id.get(&first_id).cloned();
            let middle = edges_by_id.get(&edge_id).cloned();
            let last = edges_by_id.get(&last_id).cloned();

            let Some(node) = g.node(&relationship.entity_a).cloned() else {
                if let Some(edge) = middle.or(first).or(last) {
                    render_edges.push(to_layout_edge(edge));
                }
                continue;
            };
            let dummy_ids = [
                format!("{}---{}---1", relationship.entity_a, relationship.entity_a),
                format!("{}---{}---2", relationship.entity_a, relationship.entity_a),
            ];
            let mut hints = dummy_ids
                .iter()
                .filter_map(|id| g.node(id))
                .filter_map(|node| Some((node.x?, node.y?)))
                .collect::<Vec<_>>();
            if hints.is_empty() {
                for part in [first.as_ref(), middle.as_ref(), last.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    hints.extend(part.points.iter().map(|point| (point.x, point.y)));
                }
            }
            let side = er_self_loop_side(&node, hints, rankdir);
            let label = middle
                .as_ref()
                .and_then(|part| part.label.clone())
                .or_else(|| first.as_ref().and_then(|part| part.label.clone()))
                .or_else(|| last.as_ref().and_then(|part| part.label.clone()));
            let label_width = label.as_ref().map(|label| label.width).unwrap_or(0.0);
            let points = er_self_loop_points(&node, side, label_width);
            let label = label.map(|mut label| {
                let (x, y) =
                    er_self_loop_label_position(&points, side, label.width, label.height, &node);
                label.x = x;
                label.y = y;
                label
            });
            let self_loop_cluster = subgraph_ids
                .contains(relationship.entity_a.as_str())
                .then(|| relationship.entity_a.clone());

            render_edges.push(LayoutEdge {
                id: edge_id.clone(),
                from: relationship.entity_a.clone(),
                to: relationship.entity_b.clone(),
                from_cluster: self_loop_cluster.clone(),
                to_cluster: self_loop_cluster,
                points,
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: er_marker_id(relationship.rel_spec.card_b.as_str(), "START"),
                end_marker: er_marker_id(relationship.rel_spec.card_a.as_str(), "END"),
                stroke_dasharray: (relationship.rel_spec.rel_type == "NON_IDENTIFYING")
                    .then(|| "8,8".to_string()),
            });
            continue;
        } else if let Some(edge) = edges_by_id.get(&edge_id).cloned() {
            render_edges.push(to_layout_edge(edge));
        }
    }

    let bounds = er_layout_bounds(&nodes, &layout_edges);

    Ok(ErDiagramLayout {
        nodes,
        edges: layout_edges,
        render_edges,
        swimlane: None,
        clusters,
        bounds,
        prepared_labels,
    })
}

fn er_layout_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for node in nodes {
        let half_width = node.width / 2.0;
        let half_height = node.height / 2.0;
        points.push((node.x - half_width, node.y - half_height));
        points.push((node.x + half_width, node.y + half_height));
    }
    for edge in edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
        if let Some(label) = &edge.label {
            let half_width = label.width / 2.0;
            let half_height = label.height / 2.0;
            points.push((label.x - half_width, label.y - half_height));
            points.push((label.x + half_width, label.y + half_height));
        }
    }
    Bounds::from_points(points)
}

#[cfg(feature = "layout-elk")]
fn layout_er_diagram_elk_typed(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    settings: ErLayoutSettings,
    prepared_labels: Arc<ErPreparedLabels>,
    operation_seed: elk::ElkOperationSeed,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<ErDiagramLayout> {
    let mut elk_graph = er_elk_graph(
        model,
        effective_config,
        measurer,
        &settings,
        &prepared_labels,
        &mut Some(&mut *work_control),
    )?;
    let subgraph_by_id: HashMap<&str, &ErSubgraph> = model
        .subgraphs
        .iter()
        .map(|subgraph| (subgraph.id.as_str(), subgraph))
        .collect();
    let subgraph_ids: HashSet<&str> = subgraph_by_id.keys().copied().collect();
    let mut subgraph_title_metrics = HashMap::with_capacity(subgraph_by_id.len());
    for subgraph in model.subgraphs.iter() {
        let metrics = er_subgraph_title_metrics(
            subgraph,
            measurer,
            &settings.label_style,
            settings.relationship_html_labels,
        );
        subgraph_title_metrics.insert(
            subgraph.id.as_str(),
            (metrics.width.max(0.0), metrics.height.max(0.0)),
        );
    }
    let oriented = crate::elk_feedback_edges::orient_feedback_edges(
        &mut elk_graph,
        effective_config,
        &mut Some(&mut *work_control),
    )?;
    let mut elk_layout = elk::layout_with_operation_seed_and_work_control(
        oriented.graph(),
        operation_seed,
        work_control,
    )
    .map_err(|error| work_control.map_elk_error_with_context(error, "ER ELK"))?;
    oriented.restore(&mut elk_layout, &mut Some(&mut *work_control))?;
    let source_edge_by_id = elk_graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<HashMap<_, _>>();

    let frames = crate::elk_adapter::drawing_group_frames(
        &elk_graph,
        &elk_layout,
        |node| node.label.map_or(0.0, |label| label.width) + node.container.padding,
        &mut Some(&mut *work_control),
    )?;
    let paint_order =
        crate::elk_adapter::parent_first_order(&elk_graph.nodes, &mut Some(&mut *work_control))?;
    let paint_index_by_id: HashMap<&str, usize> = paint_order
        .iter()
        .enumerate()
        .map(|(index, &node_index)| (elk_graph.nodes[node_index].id.as_str(), index))
        .collect();
    let mut out_nodes = elk_layout
        .nodes
        .into_iter()
        .map(|node| {
            let is_cluster = subgraph_ids.contains(node.id.as_str());
            let frame = frames.get(node.id.as_str());
            LayoutNode {
                id: node.id,
                x: frame.map_or(node.x, |frame| frame.x),
                y: frame.map_or(node.y, |frame| frame.y),
                width: frame.map_or(node.width, |frame| frame.width),
                height: frame.map_or(node.height, |frame| frame.height),
                is_cluster,
                label_width: None,
                label_height: None,
            }
        })
        .collect::<Vec<_>>();

    let mut clusters = Vec::with_capacity(subgraph_by_id.len());
    for node in out_nodes.iter().filter(|node| node.is_cluster) {
        let Some(subgraph) = subgraph_by_id.get(node.id.as_str()).copied() else {
            continue;
        };
        let (title_width, title_height) = subgraph_title_metrics
            .get(node.id.as_str())
            .copied()
            .unwrap_or((0.0, 0.0));
        let padding = 8.0;
        let title_label = LayoutLabel {
            x: node.x,
            y: node.y - node.height / 2.0 + padding + title_height / 2.0,
            width: title_width,
            height: title_height,
        };
        clusters.push(crate::model::LayoutCluster {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            diff: (title_width - node.width) / 2.0 - padding / 2.0,
            offset_y: title_height - padding / 2.0,
            title: subgraph.title.clone(),
            title_label,
            requested_dir: subgraph.dir.clone(),
            effective_dir: subgraph
                .dir
                .clone()
                .unwrap_or_else(|| model.direction.clone()),
            padding,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        });
    }

    let mut out_edges = Vec::with_capacity(elk_layout.edges.len());
    for edge in elk_layout.edges {
        let Some(source) = source_edge_by_id.get(edge.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown ER edge {}", edge.id),
            });
        };
        let Some(index) = parse_er_rel_idx_from_edge_name(&edge.id) else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned malformed ER edge id {}", edge.id),
            });
        };
        let Some(relationship) = model.relationships.get(index) else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned out-of-range ER edge id {}", edge.id),
            });
        };
        let points = edge
            .points
            .into_iter()
            .map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        let label = source.label.and_then(|source_label| {
            edge.labels
                .first()
                .map(|label| LayoutLabel {
                    x: label.x + label.width / 2.0,
                    y: label.y + label.height / 2.0,
                    width: label.width,
                    height: label.height,
                })
                .or_else(|| {
                    calc_label_position(&points).map(|(x, y)| LayoutLabel {
                        x,
                        y,
                        width: source_label.width,
                        height: source_label.height,
                    })
                })
        });
        let rel_type = relationship.rel_spec.rel_type.as_str();
        out_edges.push(LayoutEdge {
            id: edge.id,
            from: source.source.clone(),
            to: source.target.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: er_marker_id(relationship.rel_spec.card_b.as_str(), "START"),
            end_marker: er_marker_id(relationship.rel_spec.card_a.as_str(), "END"),
            stroke_dasharray: (rel_type == "NON_IDENTIFYING").then(|| "8,8".to_string()),
        });
    }

    out_nodes.sort_by(|left, right| left.id.cmp(&right.id));
    out_edges.sort_by(|left, right| left.id.cmp(&right.id));
    // Mermaid paints groups by stable ancestor depth, then paints leaf nodes above them.
    clusters.sort_by_key(|cluster| paint_index_by_id.get(cluster.id.as_str()).copied());
    let bounds = er_layout_bounds(&out_nodes, &out_edges);
    Ok(ErDiagramLayout {
        nodes: out_nodes,
        render_edges: out_edges.clone(),
        swimlane: None,
        edges: out_edges,
        clusters,
        bounds,
        prepared_labels,
    })
}

#[cfg(feature = "layout-elk")]
fn er_elk_graph(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    prepared_labels: &ErPreparedLabels,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    let ErLayoutSettings {
        graph,
        label_style,
        attr_style: _,
        calculation_style: _,
        relationship_label_style: _,
        relationship_html_labels,
        entity_measurement: _,
    } = settings;

    let subgraph_ids: HashSet<&str> = model
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect();
    let entity_id_by_name: HashMap<&str, &str> = model
        .entities
        .iter()
        .map(|(name, entity)| (name.as_str(), entity.id.as_str()))
        .collect();
    let mut parent_by_member: HashMap<&str, &str> = HashMap::new();
    for subgraph in &model.subgraphs {
        for member in &subgraph.nodes {
            let member_id = if subgraph_ids.contains(member.as_str()) {
                member.as_str()
            } else if let Some(entity_id) = entity_id_by_name.get(member.as_str()) {
                entity_id
            } else {
                continue;
            };
            parent_by_member.insert(member_id, subgraph.id.as_str());
        }
    }

    let mut nodes = Vec::with_capacity(model.subgraphs.len() + model.entities.len());
    for subgraph in model.subgraphs.iter().rev() {
        let metrics =
            er_subgraph_title_metrics(subgraph, measurer, label_style, *relationship_html_labels);
        let has_children = subgraph.nodes.iter().any(|member| {
            subgraph_ids.contains(member.as_str())
                || entity_id_by_name.contains_key(member.as_str())
        });
        nodes.push(elk::Node {
            id: subgraph.id.clone(),
            kind: elk::NodeKind::Group,
            container: elk::ContainerNodeOptions {
                padding: 8.0,
                ..Default::default()
            },
            label_text: Some(subgraph.title.clone()),
            width: 0.0,
            height: 0.0,
            parent: parent_by_member
                .get(subgraph.id.as_str())
                .map(|parent| (*parent).to_string()),
            direction: subgraph.dir.as_deref().and_then(er_elk_direction),
            hierarchy_handling: Some(elk::HierarchyHandling::IncludeChildren),
            layer_constraint: None,
            port_alignment: None,
            label: has_children.then_some(elk::Label {
                width: metrics.width.max(0.0),
                height: metrics.height.max(0.0),
            }),
        });
    }

    let mut entities: Vec<&ErEntity> = model
        .entities
        .iter()
        .filter_map(|(name, entity)| (!subgraph_ids.contains(name.as_str())).then_some(entity))
        .collect();
    entities.sort_by(|left, right| {
        fn counter(id: &str) -> Option<usize> {
            id.rsplit_once('-')?.1.parse().ok()
        }
        (counter(&left.id), left.id.as_str()).cmp(&(counter(&right.id), right.id.as_str()))
    });

    let entity_nodes = entities
        .into_iter()
        .map(|entity| {
            let measure =
                prepared_labels
                    .entity(&entity.id)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("ER prepared labels are missing entity {}", entity.id),
                    })?;
            Ok(elk::Node {
                id: entity.id.clone(),
                kind: elk::NodeKind::Leaf,
                container: Default::default(),
                label_text: None,
                port_alignment: None,
                width: measure.width,
                height: measure.height,
                parent: parent_by_member
                    .get(entity.id.as_str())
                    .map(|parent| (*parent).to_string()),
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                label: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    nodes.extend(entity_nodes);

    if crate::config::config_bool(effective_config, &["elk", "keepEntryNodeOnTop"]).unwrap_or(false)
    {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut nodes,
            model.relationships.iter().map(|relationship| {
                (
                    relationship.entity_a.as_str(),
                    relationship.entity_b.as_str(),
                )
            }),
            work_control,
        )?;
    }

    let node_ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut edges = Vec::with_capacity(model.relationships.len());
    for (index, relationship) in model.relationships.iter().enumerate() {
        if !node_ids.contains(relationship.entity_a.as_str())
            || !node_ids.contains(relationship.entity_b.as_str())
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    relationship.entity_a, relationship.entity_b
                ),
            });
        }
        let (label_width, label_height) = prepared_labels
            .relationship(index)
            .map(ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        edges.push(elk::Edge {
            id: format!("er-rel-{index}"),
            source: relationship.entity_a.clone(),
            target: relationship.entity_b.clone(),
            label: (!relationship.role_a.is_empty()).then_some(elk::Label {
                width: label_width,
                height: label_height,
            }),
            minlen: 1,
            terminal_labels: Vec::new(),
            inside_self_loops_yo: false,
        });
    }

    Ok(elk::Graph {
        id: "root".to_string(),
        direction: match graph.rankdir {
            dugong::RankDir::LR => elk::Direction::Right,
            dugong::RankDir::RL => elk::Direction::Left,
            dugong::RankDir::BT => elk::Direction::Up,
            dugong::RankDir::TB => elk::Direction::Down,
        },
        nodes,
        edges,
        // Mermaid's ELK adapter sets `spacing.baseValue = 40`; the source-backed adapter owns
        // that exact option projection, so family-specific Dagre spacing does not leak into ELK.
        spacing: elk::Spacing::default(),
        options: er_elk_layout_options(effective_config),
    })
}

#[cfg(feature = "layout-elk")]
fn er_elk_direction(direction: &str) -> Option<elk::Direction> {
    match direction.trim().to_ascii_uppercase().as_str() {
        "LR" => Some(elk::Direction::Right),
        "RL" => Some(elk::Direction::Left),
        "BT" => Some(elk::Direction::Up),
        "TB" => Some(elk::Direction::Down),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::OperationWorkMeter;
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};

    #[cfg(feature = "layout-elk")]
    #[test]
    fn er_elk_keeps_the_source_default_nonzero_seed() {
        assert_eq!(
            super::er_elk_layout_options(&serde_json::Value::Null)
                .layered
                .random_seed,
            1
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn er_elk_zero_seed_adapter_graph_requires_an_operation_seed() {
        use std::num::NonZeroU64;

        let mut model = merman_model::families::er::ErDiagramModel {
            direction: "TB".to_string(),
            ..Default::default()
        };
        for id in ["CUSTOMER", "ORDER"] {
            model.entities.insert(
                id.to_string(),
                merman_model::families::er::ErEntity {
                    id: id.to_string(),
                    label: id.to_string(),
                    ..Default::default()
                },
            );
        }
        model
            .relationships
            .push(merman_model::families::er::ErRelationship {
                entity_a: "CUSTOMER".to_string(),
                role_a: "places".to_string(),
                entity_b: "ORDER".to_string(),
                ..Default::default()
            });

        let effective_config = serde_json::json!({ "layout": "elk" });
        let settings = super::ErConfigView::new(&effective_config)
            .layout_settings_with_font_family(&model.direction, None);
        let measurer = DeterministicTextMeasurer::default();
        let prepared_labels = super::prepare_er_labels(&model, &effective_config, &measurer);
        let mut graph = super::er_elk_graph(
            &model,
            &effective_config,
            &measurer,
            &settings,
            &prepared_labels,
            &mut None,
        )
        .expect("ER ELK adapter graph");
        graph.options.layered.random_seed = 0;

        assert!(super::elk::layout(&graph).is_err());

        let operation_seed = super::elk::ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x6572_2d73_6565_6421).expect("nonzero operation seed"),
        );
        let first = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("seeded ER layout");
        let replayed = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("replayed seeded ER layout");

        assert_eq!(first, replayed);
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn er_elk_cyclic_entry_follows_edge_order_in_root_and_nested_scopes() {
        for declared in [["B", "A", "C"], ["C", "B", "A"]] {
            for nested in [false, true] {
                let mut model = merman_model::families::er::ErDiagramModel {
                    direction: "TB".to_string(),
                    ..Default::default()
                };
                for (index, name) in declared.iter().enumerate() {
                    model.entities.insert(
                        (*name).to_string(),
                        super::ErEntity {
                            id: format!("entity-{name}-{index}"),
                            label: (*name).to_string(),
                            ..Default::default()
                        },
                    );
                }
                for (source, target) in [("A", "B"), ("B", "C"), ("C", "A")] {
                    model.relationships.push(super::ErRelationship {
                        entity_a: model.entities[source].id.clone(),
                        entity_b: model.entities[target].id.clone(),
                        ..Default::default()
                    });
                }
                if nested {
                    model.subgraphs.push(super::ErSubgraph {
                        id: "Scope".to_string(),
                        title: "Scope".to_string(),
                        nodes: declared.iter().map(|name| (*name).to_string()).collect(),
                        ..Default::default()
                    });
                }
                for enabled in [false, true] {
                    let config = serde_json::json!({ "layout": "elk", "elk": { "keepEntryNodeOnTop": enabled } });
                    let settings = super::ErConfigView::new(&config)
                        .layout_settings_with_font_family(&model.direction, None);
                    let graph = super::er_elk_graph(
                        &model,
                        &config,
                        &DeterministicTextMeasurer::default(),
                        &settings,
                        &super::prepare_er_labels(
                            &model,
                            &config,
                            &DeterministicTextMeasurer::default(),
                        ),
                        &mut None,
                    )
                    .expect("ER adapter graph");
                    let constrained: Vec<_> = graph
                        .nodes
                        .iter()
                        .filter(|node| {
                            node.layer_constraint == Some(super::elk::LayerConstraint::First)
                        })
                        .map(|node| node.id.as_str())
                        .collect();
                    assert_eq!(
                        constrained,
                        if enabled {
                            vec![model.entities["A"].id.as_str()]
                        } else {
                            vec![]
                        }
                    );
                }
            }
        }
    }

    struct ErProbeMeasurer;

    struct ErPrecisionMeasurer;

    impl TextMeasurer for ErProbeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 73.25,
                height: 22.5,
                line_count: 1,
            }
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_family.as_deref() == Some("sans-serif") {
                120.0
            } else {
                80.0
            }
        }

        fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            17.0
        }
    }

    impl TextMeasurer for ErPrecisionMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 73.123_456_789,
                height: 17.25,
                line_count: 1,
            }
        }
    }

    fn default_style() -> TextStyle {
        TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        }
    }

    fn measure_box_label(
        source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> TextMetrics {
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let mut label = super::ErBoxLabel::from_source(source);
        label
            .prepare_metrics(
                measurer,
                style,
                &super::theme::ErEntitySourceStyle::default(),
                i64::MAX,
                WrapMode::HtmlLike,
                style.font_size,
                Some(style.font_size),
                &meter,
            )
            .expect("prepare table label metrics")
    }

    #[test]
    fn er_svg_entity_measurement_uses_svg_mode_for_names_and_attributes() {
        use crate::environment::TextMeasurementOperation;
        use std::sync::Mutex;

        #[derive(Default)]
        struct ModeMeasurer {
            measured: Mutex<Vec<(TextMeasurementOperation, String)>>,
        }
        impl TextMeasurer for ModeMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                panic!("SVG terminals must select a literal measurement operation")
            }

            fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
                40.0
            }

            fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, _style: &TextStyle) -> f64 {
                assert!(text == "book <br> title", "literal first-row bbox: {text}");
                40.0
            }

            fn measure_mermaid_calculate_text_dimensions(
                &self,
                _text: &str,
                _style: &TextStyle,
            ) -> TextMetrics {
                TextMetrics {
                    width: 40.0,
                    height: 18.0,
                    line_count: 1,
                }
            }

            fn measure_wrapped(
                &self,
                _text: &str,
                _style: &TextStyle,
                _max_width_px: Option<f64>,
                _wrap_mode: WrapMode,
            ) -> TextMetrics {
                panic!("retained SVG word rows must not re-enter wrapped label parsing")
            }

            fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, _style: &TextStyle) -> f64 {
                assert!(!text.contains('\n'));
                self.measured
                    .lock()
                    .unwrap()
                    .push((TextMeasurementOperation::TspanBBoxHeight, text.to_owned()));
                18.0
            }

            fn measure_svg_create_text_bbox_y_offset_px(
                &self,
                text: &str,
                _style: &TextStyle,
            ) -> f64 {
                assert!(!text.contains('\n'));
                self.measured.lock().unwrap().push((
                    TextMeasurementOperation::CreateTextBBoxYOffset,
                    text.to_owned(),
                ));
                -4.0
            }
        }
        let config = serde_json::json!({ "htmlLabels": false });
        let settings = super::ErConfigView::new(&config).entity_measurement_settings();
        let style = default_style();
        // Source entities remain encoded until the retained SVG words are emitted.
        // addText flattens the first row for angle references without parsing another break.
        for (comment, comment_words, comment_lines) in [
            ("book &lt;br&gt; title", &["book <br> title"][..], 1),
            (
                "book &amp;lt;br&amp;gt; title",
                &["book", " &lt;br&gt;", " title"][..],
                1,
            ),
            ("book #lt;br#gt; title", &["book <br> title"][..], 1),
            ("book<br/>title", &["book", "title"][..], 2),
        ] {
            let entity = super::ErEntity {
                id: "entity-BOOK-0".to_string(),
                label: "BOOK".to_string(),
                attributes: vec![merman_model::families::er::ErAttribute {
                    ty: "string".to_string(),
                    name: "title".to_string(),
                    keys: vec!["PK".to_string()],
                    comment: comment.to_string(),
                }],
                ..Default::default()
            };
            let measurer = ModeMeasurer::default();
            let measured = super::measure_entity_box(&entity, &measurer, &style, &style, settings);
            assert_eq!(measured.label_height, 18.0, "{comment}");
            assert_eq!(measured.rows.len(), 1, "{comment}");
            let expected_height = 18.0
                + (comment_lines - 1) as f64 * style.font_size * 1.1
                + settings.entity_padding * 1.25;
            assert!(
                (measured.rows[0].height - expected_height).abs() < 1e-9,
                "{comment}: {} != {expected_height}",
                measured.rows[0].height
            );
            assert_eq!(measured.rows[0].cell_widths, [40.0; 4], "{comment}");
            let expected_words: Vec<_> = ["BOOK", "string", "title", "PK"]
                .into_iter()
                .chain(comment_words.iter().copied())
                .collect();
            let calls = measurer.measured.lock().unwrap();
            for operation in [
                TextMeasurementOperation::TspanBBoxHeight,
                TextMeasurementOperation::CreateTextBBoxYOffset,
            ] {
                let words: Vec<_> = calls
                    .iter()
                    .filter(|(actual, _)| *actual == operation)
                    .map(|(_, text)| text.as_str())
                    .collect();
                assert_eq!(words, expected_words, "{operation:?}: {comment}");
            }
        }
    }

    #[test]
    fn er_generic_html_metrics_measure_final_literal_line() {
        struct LiteralLineMeasurer<'a>(&'a str);

        impl TextMeasurer for LiteralLineMeasurer<'_> {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                panic!("generic HTML text must select literal line primitives")
            }

            fn measure_wrapped(
                &self,
                _text: &str,
                _style: &TextStyle,
                _max_width: Option<f64>,
                _wrap_mode: WrapMode,
            ) -> TextMetrics {
                panic!("generic HTML textContent must not re-enter label parsing")
            }

            fn measure_html_line_width_px(&self, text: &str, _style: &TextStyle) -> f64 {
                assert_eq!(text, self.0);
                73.123_456_789
            }

            fn measure_html_line_height_px(&self, text: &str, _style: &TextStyle) -> f64 {
                assert_eq!(text, self.0);
                17.25
            }
        }

        for (source, visible) in [
            ("A~br~B", "A<br>B"),
            ("*A~br~B*", "A<br>B"),
            ("A~br~B  C", "A<br>B C"),
            ("A~br~B\u{a0}C", "A<br>B\u{a0}C"),
            ("A~br~B&amp;lt;br&amp;gt;C", "A<br>B<br>C"),
            (
                "A~br~B&amp;amp;lt;br&amp;amp;gt;C",
                "A<br>B&amp;lt;br&amp;gt;C",
            ),
        ] {
            let label = super::ErBoxLabel::from_source(source);
            assert!(label.uses_generic_workaround(), "{source}");
            let actual = label
                .measure_html_text_content(
                    &LiteralLineMeasurer(visible),
                    &default_style(),
                    super::ErHtmlLabelLayout {
                        width_budget: 1000,
                        break_spaces: false,
                    },
                    &OperationWorkMeter::new(
                        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                    ),
                )
                .unwrap();
            assert_eq!(actual.width, 73.123_456_789, "{source}");
            assert_eq!(actual.height, 17.25, "{source}");
            assert_eq!(actual.line_count, 1, "{source}");
        }
    }

    #[test]
    fn er_svg_relationship_metrics_measure_retained_literal_words() {
        #[derive(Default)]
        struct TerminalMeasurer(std::cell::RefCell<Vec<String>>);

        impl TextMeasurer for TerminalMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                panic!("SVG relationships must select terminal word primitives")
            }

            fn measure_wrapped(
                &self,
                _text: &str,
                _style: &TextStyle,
                _max_width: Option<f64>,
                _wrap_mode: WrapMode,
            ) -> TextMetrics {
                panic!("SVG relationship words must not re-enter label parsing")
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                text.chars().count() as f64 * 5.0
            }

            fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
                let half = self.measure_svg_text_computed_length_px(text, style) / 2.0;
                (half, half)
            }

            fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, _style: &TextStyle) -> f64 {
                self.0.borrow_mut().push(text.to_owned());
                18.0
            }

            fn measure_svg_create_text_bbox_y_offset_px(
                &self,
                text: &str,
                _style: &TextStyle,
            ) -> f64 {
                assert!(!text.contains('\n'));
                -4.0
            }
        }

        for (source, words, width, height) in [
            ("A&lt;br&gt;B", vec!["A<br>B"], 30.0, 18.0),
            ("A&amp;lt;br&amp;gt;B", vec!["A&lt;br&gt;B"], 60.0, 18.0),
            ("A<br/>B", vec!["A", "B"], 5.0, 18.0 + 16.0 * 1.1),
        ] {
            let measurer = TerminalMeasurer::default();
            let prepared = super::ErPreparedRelationshipLabel::new(
                source,
                &measurer,
                &default_style(),
                false,
                true,
                default_style().font_size,
            );
            let svg = prepared.svg().expect("prepared SVG label");
            assert_eq!(
                svg.ink_bounds.max_x - svg.ink_bounds.min_x,
                width,
                "{source}"
            );
            assert!((svg.ink_bounds.max_y - svg.ink_bounds.min_y - height).abs() < 1e-9);
            assert_eq!(prepared.metrics.width, width + 4.0, "{source}");
            assert!(
                (prepared.metrics.height - height - 4.0).abs() < 1e-9,
                "{source}"
            );
            assert_eq!(prepared.metrics.line_count, words.len(), "{source}");
            assert_eq!(svg.ink_bounds.min_y, -4.0, "{source}");
            assert_eq!(*measurer.0.borrow(), words, "{source}");
        }
    }

    #[test]
    fn er_svg_relationship_reserves_padded_ink_bounds_separately_from_advance() {
        struct InkMeasurer;

        impl TextMeasurer for InkMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                panic!("SVG relationship bounds must use literal terminal primitives")
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                text.chars().count() as f64 * 10.0
            }

            fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
                let half = self.measure_svg_text_computed_length_px(text, style) / 2.0;
                (half + 1.0, half + 3.0)
            }

            fn measure_svg_tspan_text_bbox_height_px(
                &self,
                _text: &str,
                _style: &TextStyle,
            ) -> f64 {
                18.0
            }

            fn measure_svg_create_text_bbox_y_offset_px(
                &self,
                _text: &str,
                _style: &TextStyle,
            ) -> f64 {
                -4.0
            }
        }

        let prepared = super::ErPreparedRelationshipLabel::new(
            "owns",
            &InkMeasurer,
            &default_style(),
            false,
            true,
            default_style().font_size,
        );
        let svg = prepared.svg().expect("prepared SVG label");
        assert_eq!(
            svg.ink_bounds,
            crate::model::Bounds {
                min_x: -21.0,
                max_x: 23.0,
                min_y: -4.0,
                max_y: 14.0
            }
        );
        assert_eq!(
            svg.background_bounds,
            Some(crate::model::Bounds {
                min_x: -23.0,
                max_x: 25.0,
                min_y: -6.0,
                max_y: 16.0
            })
        );
        assert_eq!(prepared.dimensions(), (48.0, 22.0));
    }

    #[test]
    fn er_html_label_metrics_use_the_selected_html_measurer() {
        let metrics = measure_box_label("type~T~", &ErProbeMeasurer, &default_style());

        assert_eq!(metrics.width, 73.25);
        assert_eq!(metrics.height, 22.5);
    }

    #[test]
    fn er_inline_code_label_metrics_stay_on_the_html_measurement_path() {
        let metrics =
            measure_box_label("inline: `**not bold**`", &ErProbeMeasurer, &default_style());

        assert_eq!(metrics.width, 73.25);
        assert_eq!(metrics.height, 22.5);
    }

    #[test]
    fn er_inline_html_metrics_measure_visible_runs_instead_of_tag_source() {
        let metrics = measure_box_label(
            "short<br><strong>bold</strong>",
            &ErProbeMeasurer,
            &default_style(),
        );

        assert_eq!(metrics.width, 73.25);
        assert_eq!(metrics.height, 48.0);
        assert_eq!(metrics.line_count, 2);
    }

    #[test]
    fn er_raw_code_and_anchor_metrics_measure_the_rendered_dom() {
        let measurer = DeterministicTextMeasurer::default();
        let style = default_style();
        let source = "<a href='https://example.com'><code>Entity</code></a>";
        let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(source, true);
        let expected = crate::text::measure_html_with_inline_styles(
            &measurer,
            &fragment,
            &style,
            None,
            WrapMode::HtmlLike,
        );

        let actual = measure_box_label(source, &measurer, &style);
        let literal = measurer.measure_wrapped(source, &style, None, WrapMode::HtmlLike);

        assert_eq!(actual.width, expected.width);
        assert_eq!(actual.height, expected.height);
        assert_eq!(actual.line_count, expected.line_count);
        assert!(
            actual.width < literal.width,
            "actual={actual:?}, literal={literal:?}"
        );
    }

    #[test]
    fn er_plain_underscore_edge_label_preserves_host_precision() {
        let prepared = super::ErPreparedRelationshipLabel::new(
            "driver_license",
            &ErPrecisionMeasurer,
            &default_style(),
            true,
            true,
            default_style().font_size,
        );
        assert_eq!(prepared.dimensions(), (73.123_456_789, 17.25));
    }

    #[test]
    fn er_prepared_labels_defer_visible_style_parsing_until_theme_evidence_needs_it() {
        let mut model = merman_model::families::er::ErDiagramModel {
            direction: "TB".to_string(),
            ..Default::default()
        };
        model.entities.insert(
            "ENTITY".to_string(),
            merman_model::families::er::ErEntity {
                id: "entity-ENTITY-0".to_string(),
                label: "**Entity**".to_string(),
                ..Default::default()
            },
        );
        model
            .relationships
            .push(merman_model::families::er::ErRelationship {
                entity_a: "entity-ENTITY-0".to_string(),
                role_a: "**owns**".to_string(),
                entity_b: "entity-ENTITY-0".to_string(),
                ..Default::default()
            });
        let prepared = super::prepare_er_labels(
            &model,
            &serde_json::Value::Null,
            &DeterministicTextMeasurer::default(),
        );

        let entity = prepared
            .entity("entity-ENTITY-0")
            .expect("prepared entity label");
        let relationship = prepared
            .relationship(0)
            .expect("prepared relationship label");
        let super::ErRelationshipLabelTerminal::Markdown {
            visible_style_facts: relationship_facts,
            ..
        } = &relationship.terminal
        else {
            panic!("ordinary ER labels keep their Markdown terminal");
        };
        assert!(entity.label.visible_style_facts.get().is_none());
        assert!(relationship_facts.get().is_none());

        assert!(entity.label.visible_style_facts().has_visible_runs());
        assert!(relationship.visible_style_facts().has_visible_runs());
        assert!(entity.label.visible_style_facts.get().is_some());
        assert!(relationship_facts.get().is_some());
    }

    #[test]
    fn er_calculate_text_width_uses_shared_mermaid_family_selection() {
        let width = super::calculate_text_width_like_mermaid_px(
            &ErProbeMeasurer,
            &default_style(),
            "DRIVER",
        );

        assert_eq!(width, 80);
    }

    #[test]
    fn er_generic_workaround_covers_transformed_and_authored_angle_references() {
        let generic = super::ErBoxLabel::from_source("*string(99)~T~~~~~~*");
        assert!(generic.uses_generic_workaround());
        assert_eq!(generic.rendered_text(), "string(99)<T<<~>>>");
        assert_eq!(
            generic.markdown_input(),
            "*string(99)&lt;T&lt;&lt;~&gt;&gt;&gt;*"
        );

        let raw_html = super::ErBoxLabel::from_source("<code>type<T></code>");
        assert!(!raw_html.uses_generic_workaround());
        assert!(raw_html.xhtml_fragment().contains("<code>"));
        let encoded = super::ErBoxLabel::from_source("A&lt;T&gt;B");
        assert!(encoded.uses_generic_workaround());
        assert_eq!(encoded.rendered_text(), "A<T>B");
    }

    fn er_resource_model() -> merman_model::families::er::ErDiagramModel {
        let mut model = merman_model::families::er::ErDiagramModel::default();
        model.entities.insert(
            "A".into(),
            super::ErEntity {
                id: "A".into(),
                label: "AB".into(),
                css_styles: vec!["font-size:32px".into(), "font-style:italic".into()],
                ..Default::default()
            },
        );
        model
    }

    #[test]
    fn er_prepared_label_projection_rejects_a_one_byte_retained_budget() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        for html in [false, true] {
            let meter = std::sync::Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                    .unwrap(),
            ));
            let model = er_resource_model();
            let config = serde_json::json!({"htmlLabels": html});
            let settings = super::ErConfigView::new(&config)
                .layout_settings_with_font_family(&model.direction, None);
            let error = super::ErPreparedLabels::prepare(
                &model,
                &DeterministicTextMeasurer::default(),
                &settings,
                &meter,
            )
            .unwrap_err();
            let crate::Error::ResourceLimitExceeded(error) = error else {
                panic!("expected retained-byte rejection")
            };
            assert_eq!(error.limit, "max_prepared_text_retained_bytes");
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn er_prepared_projection_and_shared_source_lease_release_after_the_last_owner() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let model = er_resource_model();
        let config = serde_json::json!({"htmlLabels": false});
        let settings = super::ErConfigView::new(&config)
            .layout_settings_with_font_family(&model.direction, None);
        let prepared = std::sync::Arc::new(
            super::ErPreparedLabels::prepare(
                &model,
                &DeterministicTextMeasurer::default(),
                &settings,
                &meter,
            )
            .unwrap(),
        );
        let entity = prepared.entity("A").unwrap();
        let bytes = meter.prepared_text_retained_bytes();
        assert!(
            bytes
                >= entity.label.retained_buffer_bytes()
                    + entity.source_style.retained_declaration_bytes()
        );
        assert!(!entity.label.svg_markup().is_empty());
        let replay = std::sync::Arc::clone(&prepared);
        drop(prepared);
        assert_eq!(meter.prepared_text_retained_bytes(), bytes);
        drop(replay);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn er_later_entity_budget_rejection_releases_earlier_prepared_projection() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        let measuring = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut model = er_resource_model();
        let config = serde_json::json!({"htmlLabels": false});
        let settings = super::ErConfigView::new(&config)
            .layout_settings_with_font_family(&model.direction, None);
        let first = super::ErPreparedLabels::prepare(
            &model,
            &DeterministicTextMeasurer::default(),
            &settings,
            &measuring,
        )
        .unwrap();
        let first_bytes = measuring.prepared_text_retained_bytes();
        drop(first);
        assert_eq!(measuring.prepared_text_retained_bytes(), 0);
        model.entities.insert(
            "B".into(),
            super::ErEntity {
                id: "B".into(),
                label: "CD".into(),
                ..Default::default()
            },
        );
        let bounded = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, first_bytes)
                .unwrap(),
        ));
        assert!(
            super::ErPreparedLabels::prepare(
                &model,
                &DeterministicTextMeasurer::default(),
                &settings,
                &bounded
            )
            .is_err()
        );
        assert_eq!(bounded.prepared_text_retained_bytes_peak(), first_bytes);
        assert_eq!(bounded.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn er_cancelled_label_measurement_releases_earlier_prepared_projection() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
        let control = merman_model::OperationControl::new();
        struct CancellingLabel(merman_model::OperationControl);
        impl TextMeasurer for CancellingLabel {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                if text.trim() == "CD" {
                    self.0.cancel();
                }
                TextMetrics {
                    width: 20.0,
                    height: 16.0,
                    line_count: 1,
                }
            }
        }
        let meter = std::sync::Arc::new(OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        ));
        let mut model = er_resource_model();
        model.entities.insert(
            "B".into(),
            super::ErEntity {
                id: "B".into(),
                label: "CD".into(),
                ..Default::default()
            },
        );
        let config = serde_json::json!({"htmlLabels": false});
        let settings = super::ErConfigView::new(&config)
            .layout_settings_with_font_family(&model.direction, None);
        let error =
            super::ErPreparedLabels::prepare(&model, &CancellingLabel(control), &settings, &meter)
                .unwrap_err();
        assert!(matches!(error, crate::Error::Cancelled(_)));
        assert!(meter.prepared_text_retained_bytes_peak() > 0);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}
