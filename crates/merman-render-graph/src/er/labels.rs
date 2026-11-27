//! Prepared ER entity, attribute, relationship and title labels.

#[cfg(test)]
use super::ErConfigView;
use super::{
    ErEntityMeasurementSettings, ErEntitySourceStyle, ErLayoutSettings,
    compile_er_entity_source_style, swimlane,
};
use merman_model::families::er::{ErEntity, ErSubgraph};
use merman_render_core::{
    Bounds, Error, Result,
    resources::{OperationWorkMeter, PreparedTextRetainedReservation},
    text::{
        self, MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, TextMeasurer, TextMetrics, TextStyle, WrapMode,
        html_wrap::HtmlWhiteSpace,
        inline_html::{MermaidMarkdownWordType, SvgTextEntityMode, svg_markdown_word_style},
        measure_mermaid_text_dimensions,
    },
    xml::escape_xml,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

// HTML rem resolves against the document root, rather than the SVG's inherited font size.
pub(super) const HTML_DOCUMENT_ROOT_FONT_SIZE_PX: f64 = 16.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct ErHtmlLabelLayout {
    pub(super) width_budget: i64,
    pub(super) break_spaces: bool,
}

#[derive(Debug)]
pub struct ErBoxLabel {
    markdown_input: String,
    rendered_text: String,
    xhtml_fragment: String,
    generic_workaround: bool,
    svg_markup: Option<String>,
    html_layout: Option<ErHtmlLabelLayout>,
}

impl ErBoxLabel {
    pub(super) fn from_source(source: &str) -> Self {
        Self::from_source_with_generic_types(source, true)
    }

    pub fn from_source_with_generic_types(source: &str, expand_generic_types: bool) -> Self {
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
        let xhtml_fragment = text::mermaid_markdown_to_xhtml_label_fragment(&markdown_input, true);
        let rendered_text = if generic_workaround {
            // addText restores only angle-bracket spellings through textContent after
            // createText. Other entities retain the depth left by the terminal decoder.
            text::mermaid_xhtml_label_text_content(&xhtml_fragment)
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
        }
    }

    pub(super) fn markdown_input(&self) -> &str {
        &self.markdown_input
    }

    pub(super) fn rendered_text(&self) -> &str {
        &self.rendered_text
    }

    pub fn xhtml_fragment(&self) -> &str {
        &self.xhtml_fragment
    }

    pub fn uses_generic_workaround(&self) -> bool {
        self.generic_workaround
    }

    pub(super) fn svg_text_source(&self) -> &str {
        if self.generic_workaround {
            &self.rendered_text
        } else {
            &self.markdown_input
        }
    }

    pub(super) fn svg_markup(&self) -> &str {
        self.svg_markup
            .as_deref()
            .expect("ER SVG labels are prepared before emission")
    }

    pub(super) fn html_layout(&self) -> Option<ErHtmlLabelLayout> {
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
        source_style: &ErEntitySourceStyle,
        width: i64,
        wrap_mode: WrapMode,
        root_font_size: f64,
        work_meter: &OperationWorkMeter,
    ) -> Result<TextMetrics> {
        use merman_model::OperationPhase;

        work_meter.checkpoint(OperationPhase::Layout)?;
        let text_style = source_style.text_style(probe_style, root_font_size);
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
            let max_width = Some(width.max(0) as f64);
            let nowrap = text::measure_xhtml_label_fragment_with_terminal_fonts(
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
            let mut metrics = text::measure_xhtml_label_fragment_with_terminal_fonts(
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
                self.xhtml_fragment = escape_xml(self.rendered_text());
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
        let lines = text::mermaid_markdown_to_wrapped_word_lines(
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
                    text.push_str(&SvgTextEntityMode::CreateTextSource.visible_word(word));
                    text
                })
                .replace("&lt;", "<")
                .replace("&gt;", ">")
        });
        let metrics = match flattened_first_row.as_deref() {
            Some(first_row) => {
                measure_er_flattened_svg_rows(measurer, &lines, first_row, &text_style, work_meter)?
            }
            None => text::measure_svg_markdown_word_lines(
                measurer,
                &lines,
                &text_style,
                SvgTextEntityMode::CreateTextSource,
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

    fn prepare_svg_markup(
        &self,
        lines: &[Vec<(String, MermaidMarkdownWordType)>],
        flattened_first_row: Option<&str>,
        style: &str,
        work_meter: &merman_render_core::resources::OperationWorkMeter,
    ) -> Result<String> {
        let mut markup = String::new();
        text::svg_label::write_svg_markdown_word_lines_with_style(
            &mut markup,
            lines,
            Some(style),
            SvgTextEntityMode::CreateTextSource,
        );
        work_meter.charge(markup.len())?;
        if let Some(text) = flattened_first_row {
            // Apply erBox's one post-createText textContent mutation to the exact shared
            // serialization. Untouched rows retain the shared writer's word attributes.
            let range = {
                let document =
                    roxmltree::Document::parse(&markup).map_err(|error| Error::InvalidModel {
                        message: format!("invalid ER createText projection: {error}"),
                    })?;
                document
                    .root_element()
                    .first_element_child()
                    .map(|row| row.range())
            };
            if let Some(range) = range {
                let row = &markup[range.clone()];
                if !row.ends_with("/>") {
                    let opening_end = range.start + row.find('>').expect("SVG row opening") + 1;
                    let closing_start = range.end - "</tspan>".len();
                    markup.replace_range(opening_end..closing_start, &escape_xml(text));
                }
            }
        }
        Ok(markup)
    }

    fn measure_html_text_content(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        layout: ErHtmlLabelLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<TextMetrics> {
        use text::html_wrap::{NormalizedHtmlText, plan_html_lines_with_measurer};
        let mut checkpoint = || {
            work_meter
                .checkpoint(merman_model::OperationPhase::Layout)
                .map_err(Error::from)
        };
        let mut normalized = NormalizedHtmlText::default();
        normalized.append(
            self.rendered_text(),
            if layout.break_spaces {
                HtmlWhiteSpace::BreakSpaces
            } else {
                HtmlWhiteSpace::Nowrap
            },
            &mut checkpoint,
        )?;
        let lines = plan_html_lines_with_measurer(
            &normalized,
            layout.break_spaces.then_some(layout.width_budget as f64),
            Some((measurer, style, Default::default())),
            &mut |text, _, _| {
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
}

fn measure_er_flattened_svg_rows(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<(String, MermaidMarkdownWordType)>],
    first_row: &str,
    style: &TextStyle,
    work_meter: &OperationWorkMeter,
) -> Result<TextMetrics> {
    let mut width = 0.0_f64;
    let mut ink: Option<(f64, f64)> = None;
    for (index, words) in lines.iter().enumerate() {
        work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
        let mut observe = |text: &str, face: &TextStyle| -> Result<()> {
            if text::trim_html_collapsible_ascii_whitespace(text).is_empty() {
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
            let measured = text::measure_svg_markdown_word_lines(
                measurer,
                std::slice::from_ref(words),
                style,
                SvgTextEntityMode::CreateTextSource,
            );
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            width = width.max(measured.width);
            for (word_index, (word, kind)) in words.iter().enumerate() {
                let visible = SvgTextEntityMode::CreateTextSource.visible_word(word);
                let text = if word_index == 0 {
                    visible.into_owned()
                } else {
                    format!(" {visible}")
                };
                observe(&text, &svg_markdown_word_style(style, *kind))?;
            }
        }
    }
    Ok(TextMetrics {
        width: text::round_to_1_64_px(width),
        height: ink.map_or(0.0, |(top, bottom)| (bottom - top).max(0.0)),
        line_count: lines.len(),
    })
}

#[derive(Debug)]
pub struct ErPreparedRelationshipLabel {
    metrics: TextMetrics,
    terminal: ErRelationshipLabelTerminal,
    _retained: Option<PreparedTextRetainedReservation>,
}

#[derive(Debug, Clone)]
pub(super) struct ErPreparedSvgRelationshipLabel {
    pub(super) word_lines: Vec<Vec<(String, MermaidMarkdownWordType)>>,
    pub(super) ink_bounds: Bounds,
    pub(super) background_bounds: Option<Bounds>,
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
            text::mermaid_markdown_to_wrapped_word_lines(
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
                let visible = SvgTextEntityMode::CreateTextSource.visible_word(word);
                let text = if word_index == 0 {
                    visible
                } else {
                    std::borrow::Cow::Owned(format!(" {visible}"))
                };
                let face = svg_markdown_word_style(style, *kind);
                let word_advance = measurer.measure_svg_text_computed_length_px(&text, &face);
                if !text::trim_html_collapsible_ascii_whitespace(&text).is_empty() {
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
                        .map(|(word, _)| SvgTextEntityMode::CreateTextSource.visible_word(word))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let face = svg_markdown_word_style(style, *kind);
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
            text::mermaid_markdown_to_xhtml_label_fragment(&source, markdown_auto_wrap);
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
            text::measure_xhtml_label_fragment_with_terminal_fonts(
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

    pub(super) fn xhtml_fragment(&self) -> &str {
        match &self.terminal {
            ErRelationshipLabelTerminal::Markdown { xhtml_fragment, .. } => xhtml_fragment,
            ErRelationshipLabelTerminal::Swimlane(label) => label.xhtml_fragment().unwrap_or(""),
        }
    }

    pub(super) fn svg(&self) -> Option<&ErPreparedSvgRelationshipLabel> {
        match &self.terminal {
            ErRelationshipLabelTerminal::Markdown { svg, .. } => svg.as_ref(),
            ErRelationshipLabelTerminal::Swimlane(_) => None,
        }
    }

    pub(super) fn html_is_constrained(&self) -> bool {
        // Min-content can exceed the budget for long words or NBSP-connected text.
        // Those labels still use createText's table/break-spaces wrapper.
        self.metrics.width >= MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX
    }

    pub(super) fn swimlane(&self) -> Option<&swimlane::ErSwimlaneRelationshipLabel> {
        match &self.terminal {
            ErRelationshipLabelTerminal::Swimlane(label) => Some(label),
            ErRelationshipLabelTerminal::Markdown { .. } => None,
        }
    }

    pub fn dimensions(&self) -> (f64, f64) {
        (self.metrics.width.max(0.0), self.metrics.height.max(0.0))
    }

    fn new_swimlane(
        text: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        html_labels: bool,
        config: &merman_model::MermaidConfig,
    ) -> Self {
        let label = swimlane::ErSwimlaneRelationshipLabel::prepare(
            text,
            measurer,
            style,
            html_labels,
            config,
        );
        Self {
            metrics: label.metrics(),
            terminal: ErRelationshipLabelTerminal::Swimlane(label),
            _retained: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct ErPreparedLabels {
    entity_measures: BTreeMap<String, ErEntityMeasure>,
    relationship_labels: Vec<ErPreparedRelationshipLabel>,
    // The theme plan prepares cluster labels after layout and uses this same retained ledger.
    pub(super) retained_work_meter: Option<Arc<OperationWorkMeter>>,
    _retained: Option<PreparedTextRetainedReservation>,
}

impl ErPreparedLabels {
    pub fn prepare_for_layout(
        model: &merman_model::families::er::ErDiagramModel,
        measurer: &dyn TextMeasurer,
        settings: &ErLayoutSettings,
        config: &Value,
        swimlane: bool,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let sanitize_config = swimlane.then(|| {
            merman_model::MermaidConfig::from_value(merman_model::json::clone_value_nonrecursive(
                config,
            ))
        });
        Self::prepare_with_provider(
            model,
            measurer,
            settings,
            merman_render_core::config::config_bool(config, &["markdownAutoWrap"]).unwrap_or(true),
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
            let source_style = compile_er_entity_source_style(entity, &model.classes);
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            let mut measure = measure_entity_box_with_source(
                entity,
                measurer,
                &settings.label_style,
                &settings.attr_style,
                &settings.calculation_style,
                settings.code_family.as_deref(),
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
                    settings.relationship_html_labels,
                    config,
                )
            } else {
                ErPreparedRelationshipLabel::new(
                    &relationship.role_a,
                    measurer,
                    &settings.relationship_label_style,
                    settings.relationship_html_labels,
                    markdown_auto_wrap,
                    HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
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
            retained_work_meter: Some(Arc::clone(work_meter)),
            _retained: Some(retained),
        })
    }

    pub fn entity(&self, entity_id: &str) -> Option<&ErEntityMeasure> {
        self.entity_measures.get(entity_id)
    }

    pub fn relationship(&self, index: usize) -> Option<&ErPreparedRelationshipLabel> {
        self.relationship_labels.get(index)
    }
}

#[cfg(test)]
fn test_layout_settings(config: &Value, direction: &str, swimlane: bool) -> ErLayoutSettings {
    let view = ErConfigView::new(config);
    view.layout_settings_with_native_style(
        direction,
        None,
        None,
        view.relationship_html_labels(swimlane),
    )
}

#[cfg(test)]
pub(super) fn prepare_er_labels(
    model: &merman_model::families::er::ErDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    swimlane: bool,
) -> Arc<ErPreparedLabels> {
    let settings = test_layout_settings(effective_config, &model.direction, swimlane);
    let work_meter = Arc::new(OperationWorkMeter::new(
        merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    Arc::new(
        ErPreparedLabels::prepare_for_layout(
            model,
            measurer,
            &settings,
            effective_config,
            swimlane,
            &work_meter,
        )
        .expect("prepare ER labels"),
    )
}

pub fn er_swimlane_title_metrics(
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
        WrapMode::HtmlLike => text::measure_xhtml_label_fragment_with_terminal_fonts(
            measurer,
            label.xhtml_fragment(),
            style,
            None,
            HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
        ),
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
            text::measure_markdown_with_inline_styles(
                measurer,
                label.svg_text_source(),
                style,
                None,
                wrap_mode,
            )
        }
    }
}

pub fn er_subgraph_title_metrics(
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
        let fragment = text::mermaid_markdown_to_xhtml_label_fragment(&subgraph.title, true);
        text::measure_xhtml_label_fragment_with_terminal_fonts(
            measurer,
            &fragment,
            style,
            None,
            HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
        )
    } else {
        text::measure_markdown_with_inline_styles(measurer, &subgraph.title, style, None, wrap_mode)
    }
}

fn calculate_text_width_like_mermaid_px(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
) -> i64 {
    measure_mermaid_text_dimensions(measurer, text, style).width
}

#[derive(Debug)]
pub(super) struct ErEntityMeasureRow {
    pub type_label: ErBoxLabel,
    pub name_label: ErBoxLabel,
    pub key_label: ErBoxLabel,
    pub comment_label: ErBoxLabel,
    pub height: f64,
    pub cell_widths: [f64; 4],
    pub cell_heights: [f64; 4],
}

#[derive(Debug)]
pub struct ErEntityMeasure {
    pub(super) source_style: ErEntitySourceStyle,
    pub width: f64,
    pub height: f64,
    pub(super) text_padding: f64,
    pub(super) label: ErBoxLabel,
    pub label_html_width: f64,
    pub label_height: f64,
    pub(super) unformatted_label_metrics: TextMetrics,
    pub(super) has_key: bool,
    pub(super) has_comment: bool,
    pub(super) type_col_w: f64,
    pub(super) name_col_w: f64,
    pub(super) key_col_w: f64,
    pub(super) comment_col_w: f64,
    pub(super) rows: Vec<ErEntityMeasureRow>,
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
pub(super) fn measure_entity_box(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    settings: ErEntityMeasurementSettings,
) -> ErEntityMeasure {
    let work_meter = OperationWorkMeter::new(
        merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    );
    measure_entity_box_with_source(
        entity,
        measurer,
        label_style,
        attr_style,
        label_style,
        None,
        settings,
        compile_er_entity_source_style(entity, &indexmap::IndexMap::new()),
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
    code_family: Option<&str>,
    settings: ErEntityMeasurementSettings,
    source_style: ErEntitySourceStyle,
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
    let root_font_size = if settings.label_wrap_mode == WrapMode::HtmlLike {
        HTML_DOCUMENT_ROOT_FONT_SIZE_PX
    } else {
        label_style.font_size
    };
    let label_probe_style = source_style.text_style(label_style, root_font_size);
    let attr_probe_style = source_style.text_style(attr_style, root_font_size);
    let native_type_style = code_family.map(|family| {
        source_style.text_style(
            &TextStyle {
                font_family: Some(family.to_owned()),
                ..attr_style.clone()
            },
            root_font_size,
        )
    });
    let type_style = native_type_style.as_ref().unwrap_or(&attr_probe_style);

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
            root_font_size,
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
        root_font_size,
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
        let cell_max_widths_px = [
            (
                &type_label,
                native_type_style.as_ref().unwrap_or(calculation_style),
            ),
            (&name_label, calculation_style),
            (&key_label, calculation_style),
            (&comment_label, calculation_style),
        ]
        .map(|(label, style)| {
            calculate_text_width_like_mermaid_px(measurer, style, label.markdown_input())
                .saturating_add(100)
        });
        let [type_m, name_m, keys_m, comment_m] = [
            (&mut type_label, cell_max_widths_px[0], type_style),
            (&mut name_label, cell_max_widths_px[1], &attr_probe_style),
            (&mut key_label, cell_max_widths_px[2], &attr_probe_style),
            (&mut comment_label, cell_max_widths_px[3], &attr_probe_style),
        ]
        .map(|(label, width, style)| {
            label.prepare_metrics(
                measurer,
                style,
                &source_style,
                width,
                settings.label_wrap_mode,
                root_font_size,
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

#[cfg(test)]
mod tests {
    use super::text;
    use merman_model::families::er::{ErDiagramModel, ErRelationship};
    use merman_render_core::Bounds;
    use merman_render_core::resources::OperationWorkMeter;
    use serde_json::json;
    use text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};

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
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let mut label = super::ErBoxLabel::from_source(source);
        label
            .prepare_metrics(
                measurer,
                style,
                &super::ErEntitySourceStyle::default(),
                i64::MAX,
                WrapMode::HtmlLike,
                style.font_size,
                &meter,
            )
            .expect("prepare table label metrics")
    }

    #[test]
    fn er_svg_entity_measurement_uses_svg_mode_for_names_and_attributes() {
        use std::sync::Mutex;
        use text::measurement::TextMeasurementOperation;

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
                        merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
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
            Bounds {
                min_x: -21.0,
                max_x: 23.0,
                min_y: -4.0,
                max_y: 14.0
            }
        );
        assert_eq!(
            svg.background_bounds,
            Some(Bounds {
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
        let fragment = text::mermaid_markdown_to_xhtml_label_fragment(source, true);
        let expected = text::inline_html::measure_html_with_inline_styles(
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
        use merman_render_core::resources::{
            OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
        };
        for html in [false, true] {
            let meter = std::sync::Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                    .unwrap(),
            ));
            let model = er_resource_model();
            let config = serde_json::json!({"htmlLabels": html});
            let settings = super::test_layout_settings(&config, &model.direction, false);
            let error = super::ErPreparedLabels::prepare(
                &model,
                &DeterministicTextMeasurer::default(),
                &settings,
                &meter,
            )
            .unwrap_err();
            let merman_render_core::Error::ResourceLimitExceeded(error) = error else {
                panic!("expected retained-byte rejection")
            };
            assert_eq!(error.limit, "max_prepared_text_retained_bytes");
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn er_prepared_projection_and_shared_source_lease_release_after_the_last_owner() {
        use merman_render_core::resources::{OperationWorkMeter, RenderResourcePolicy};
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let model = er_resource_model();
        let config = serde_json::json!({"htmlLabels": false});
        let settings = super::test_layout_settings(&config, &model.direction, false);
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
        use merman_render_core::resources::{
            OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
        };
        let measuring = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut model = er_resource_model();
        let config = serde_json::json!({"htmlLabels": false});
        let settings = super::test_layout_settings(&config, &model.direction, false);
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
        use merman_render_core::resources::{OperationWorkMeter, RenderResourcePolicy};
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
        let settings = super::test_layout_settings(&config, &model.direction, false);
        let error =
            super::ErPreparedLabels::prepare(&model, &CancellingLabel(control), &settings, &meter)
                .unwrap_err();
        assert!(matches!(error, merman_render_core::Error::Cancelled(_)));
        assert!(meter.prepared_text_retained_bytes_peak() > 0);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn swimlane_plain_terminal_owns_literal_text_and_metrics() {
        let measurer = DeterministicTextMeasurer::default();
        let model = ErDiagramModel {
            direction: "TB".into(),
            relationships: vec![ErRelationship {
                entity_a: "A".into(),
                entity_b: "B".into(),
                role_a: "**owns**".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        for html in [true, false] {
            let config = json!({"layout": "swimlane", "htmlLabels": html,
                "flowchart": {"wrappingWidth": 80}});
            let labels = super::prepare_er_labels(&model, &config, &measurer, true);
            let relationship = labels.relationship(0).unwrap();
            let terminal = relationship
                .swimlane()
                .expect("provider-owned plain terminal");
            assert_eq!(
                relationship.dimensions(),
                (terminal.metrics().width, terminal.metrics().height)
            );
            if html {
                let fragment = terminal.xhtml_fragment().unwrap();
                assert!(fragment.contains("**owns**") && !fragment.contains("<strong>"));
            } else {
                assert_eq!(
                    terminal.svg_lines().unwrap(),
                    &[vec!["**owns**".to_string()]]
                );
            }
        }
    }
}
