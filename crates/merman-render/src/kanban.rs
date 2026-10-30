use crate::Result;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, KanbanDiagramLayout, KanbanItemLayout, KanbanSectionLayout};
use crate::resources::OperationWorkMeter;
#[cfg(test)]
use crate::resources::RenderResourcePolicy;
use crate::svg_security::{
    MermaidNavigationSecurity, SerializedMermaidNavigationHref, prepare_mermaid_navigation_href,
};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use merman_model::families::kanban::KanbanDiagramRenderModel;

pub(crate) const KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX: f64 = 25.0;
pub(crate) const KANBAN_SECTION_PADDING_PX: f64 = 10.0;
pub(crate) const KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX: f64 = 24.0;
pub(crate) const KANBAN_ITEM_LABEL_PADDING_Y_PX: f64 = 10.0;
const KANBAN_DETAIL_LABEL_MEASUREMENT_PASSES: usize = 2;

pub(crate) struct KanbanMarkdown<'a> {
    sanitize_config: &'a merman_model::presentation::PresentationConfig,
    auto_wrap: bool,
}

impl<'a> KanbanMarkdown<'a> {
    pub(crate) fn new(
        effective_config: &'a merman_model::presentation::PresentationConfig,
    ) -> Self {
        Self {
            sanitize_config: effective_config,
            auto_wrap: crate::config::config_bool(
                effective_config.as_value(),
                &["markdownAutoWrap"],
            )
            .unwrap_or(true),
        }
    }

    pub(crate) fn render(&self, raw: &str) -> String {
        let sanitized =
            merman_markup::sanitize::sanitize_text(raw, self.sanitize_config.as_config());
        crate::text::mermaid_markdown_to_xhtml_label_fragment(&sanitized, self.auto_wrap)
    }

    pub(crate) fn measure_html(
        &self,
        measurer: &dyn TextMeasurer,
        html: &str,
        style: &TextStyle,
        max_width: Option<f64>,
    ) -> TextMetrics {
        crate::text::measure_xhtml_label_fragment(
            measurer,
            html,
            style,
            max_width,
            WrapMode::HtmlLike,
        )
    }
}

mod config;
mod css_binding;
mod text_paint;
mod theme;

pub(crate) use text_paint::{KanbanTextPaintOutcome, KanbanTextPaintPlan};

pub(crate) use config::{KanbanConfigView, default_use_max_width};
pub(crate) use css_binding::KanbanCssBinding;
pub(crate) use theme::{
    KanbanTaskOccurrence, KanbanTaskTerminalDecision, KanbanTaskTheme, KanbanTaskThemeOutcome,
    KanbanTypographyPlan, KanbanTypographyTerminalFacts,
};

#[derive(Debug)]
pub(crate) struct KanbanPreparedArtifact {
    layout: KanbanDiagramLayout,
    adaptation: Option<crate::LayoutAdaptationObservation>,
    sections: Vec<KanbanPreparedMarkdownLabel>,
    items: Vec<KanbanPreparedItem>,
    task_theme: KanbanTaskTheme,
    text_paint: KanbanTextPaintPlan,
    css: KanbanCssBinding,
    _retained_text: Option<crate::resources::PreparedTextRetainedReservation>,
}

impl KanbanPreparedArtifact {
    pub(crate) fn with_retained_text_accounting(
        mut self,
        work_meter: &std::sync::Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let payloads = self
            .sections
            .iter()
            .map(|label| label.html.capacity())
            .chain(self.items.iter().flat_map(|item| {
                [
                    item.title.html.capacity(),
                    item.ticket.html.capacity(),
                    item.assigned.html.capacity(),
                    item.ticket_link
                        .as_ref()
                        .and_then(|link| link.href.as_ref())
                        .map_or(0, |href| href.as_serialized_str().len()),
                ]
            }));
        // Vec capacities include inline String, geometry, style-fact, and ticket-link slots.
        // The artifact owns the reservation and all prepared labels and serialized hrefs.
        // Hrefs use Box<str>, so their lengths cover the complete retained string allocations.
        let owner_bytes = std::mem::size_of::<Self>()
            .saturating_add(
                self.sections
                    .capacity()
                    .saturating_mul(std::mem::size_of::<KanbanPreparedMarkdownLabel>()),
            )
            .saturating_add(
                self.items
                    .capacity()
                    .saturating_mul(std::mem::size_of::<KanbanPreparedItem>()),
            );
        let bytes = payloads.fold(owner_bytes, usize::saturating_add);
        let work = OperationLayoutWorkControl::new(std::sync::Arc::clone(work_meter));
        self._retained_text = Some(work.reserve_prepared_text_retained_bytes(bytes)?);
        Ok(self)
    }

    pub(crate) const fn layout_adaptation(&self) -> Option<crate::LayoutAdaptationObservation> {
        self.adaptation
    }

    pub(crate) fn css(&self) -> &KanbanCssBinding {
        &self.css
    }

    pub(crate) fn layout(&self) -> &KanbanDiagramLayout {
        &self.layout
    }

    pub(crate) fn render_parts(
        &self,
    ) -> (
        &KanbanDiagramLayout,
        &[KanbanPreparedMarkdownLabel],
        &[KanbanPreparedItem],
    ) {
        (&self.layout, &self.sections, &self.items)
    }

    pub(crate) const fn task_theme(&self) -> &KanbanTaskTheme {
        &self.task_theme
    }

    pub(crate) const fn text_paint(&self) -> &KanbanTextPaintPlan {
        &self.text_paint
    }
}

#[derive(Debug)]
pub(crate) struct KanbanPreparedMarkdownLabel {
    pub(crate) html: String,
    pub(crate) geometry: KanbanPreparedLabelGeometry,
    pub(crate) visible_style_facts: crate::text::VisibleTextStyleFacts,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct KanbanPreparedLabelGeometry {
    pub(crate) content_height: f64,
    pub(crate) foreign_object_width: f64,
    pub(crate) wrapped: bool,
}

impl KanbanPreparedLabelGeometry {
    pub(crate) fn empty() -> Self {
        Self {
            content_height: 0.0,
            foreign_object_width: 0.0,
            wrapped: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct KanbanPreparedItem {
    pub(crate) title: KanbanPreparedMarkdownLabel,
    pub(crate) ticket: KanbanPreparedMetadataLabel,
    pub(crate) assigned: KanbanPreparedMetadataLabel,
    pub(crate) ticket_link: Option<KanbanPreparedTicketLink>,
}

#[derive(Debug)]
pub(crate) struct KanbanPreparedMetadataLabel {
    pub(crate) html: String,
    pub(crate) geometry: KanbanPreparedLabelGeometry,
    pub(crate) visible_style_facts: crate::text::VisibleTextStyleFacts,
}

#[derive(Debug)]
pub(crate) struct KanbanPreparedTicketLink {
    pub(crate) href: Option<SerializedMermaidNavigationHref>,
}

fn replace_first_like_javascript(source: &str, search: &str, replacement: &str) -> String {
    let Some(match_start) = source.find(search) else {
        return source.to_string();
    };
    let match_end = match_start + search.len();
    let prefix = &source[..match_start];
    let suffix = &source[match_end..];
    let mut result = String::with_capacity(source.len() + replacement.len());
    result.push_str(prefix);

    let mut cursor = 0usize;
    while let Some(relative_dollar) = replacement[cursor..].find('$') {
        let dollar = cursor + relative_dollar;
        result.push_str(&replacement[cursor..dollar]);
        match replacement.as_bytes().get(dollar + 1).copied() {
            Some(b'$') => result.push('$'),
            Some(b'&') => result.push_str(search),
            Some(b'`') => result.push_str(prefix),
            Some(b'\'') => result.push_str(suffix),
            _ => {
                result.push('$');
                cursor = dollar + 1;
                continue;
            }
        }
        cursor = dollar + 2;
    }
    result.push_str(&replacement[cursor..]);
    result.push_str(suffix);
    result
}

fn prepare_kanban_ticket_link(
    ticket_base_url: Option<&str>,
    ticket: Option<&str>,
    effective_config: &merman_model::presentation::PresentationConfig,
) -> Option<KanbanPreparedTicketLink> {
    let ticket = ticket.filter(|ticket| !ticket.is_empty())?;
    let ticket_url = replace_first_like_javascript(ticket_base_url?, "#TICKET#", ticket);
    let href = prepare_mermaid_navigation_href(
        &ticket_url,
        MermaidNavigationSecurity::from_security_level_loose(
            effective_config.get_str("securityLevel") == Some("loose"),
        ),
    );
    Some(KanbanPreparedTicketLink { href })
}

fn prepare_kanban_markdown_label(
    markdown: &KanbanMarkdown<'_>,
    measurer: &dyn TextMeasurer,
    raw: &str,
    style: &TextStyle,
    max_width: f64,
    capture_style_facts: bool,
) -> KanbanPreparedMarkdownLabel {
    let html = markdown.render(raw);
    let raw_metrics = markdown.measure_html(measurer, &html, style, None);
    let wrapped = max_width > 0.0 && raw_metrics.width > max_width;
    let metrics = if wrapped {
        markdown.measure_html(measurer, &html, style, Some(max_width))
    } else {
        raw_metrics
    };

    KanbanPreparedMarkdownLabel {
        visible_style_facts: if capture_style_facts {
            crate::text::VisibleTextStyleFacts::from_xhtml_fragment(&html)
        } else {
            crate::text::VisibleTextStyleFacts::not_requested()
        },
        html,
        geometry: KanbanPreparedLabelGeometry {
            content_height: metrics.height,
            foreign_object_width: if wrapped {
                max_width.max(0.0)
            } else {
                metrics.width.max(0.0)
            },
            wrapped,
        },
    }
}

fn prepare_kanban_title_label(
    markdown: &KanbanMarkdown<'_>,
    measurer: &dyn TextMeasurer,
    raw: &str,
    style: &TextStyle,
    max_width: f64,
    capture_style_facts: bool,
) -> KanbanPreparedMarkdownLabel {
    let mut prepared = prepare_kanban_markdown_label(
        markdown,
        measurer,
        raw,
        style,
        max_width,
        capture_style_facts,
    );
    if raw.is_empty() {
        prepared.geometry.foreign_object_width = 0.0;
    }
    prepared
}

fn prepare_kanban_metadata_label(
    markdown: &KanbanMarkdown<'_>,
    measurer: &dyn TextMeasurer,
    text: Option<&str>,
    style: &TextStyle,
    max_width: f64,
    capture_style_facts: bool,
) -> KanbanPreparedMetadataLabel {
    // Metadata takes the non-Markdown HTML path. Keep its original source on the public model;
    // only this private terminal owns HTML parsing, sanitization, and XHTML serialization.
    let html = text
        .filter(|text| !text.is_empty())
        .map_or_else(String::new, |text| {
            merman_markup::sanitize::sanitize_xhtml(
                &format!(
                    "<p>{}</p>",
                    text.replace("\\n", "<br/>").replace('\n', "<br/>")
                ),
                markdown.sanitize_config.as_config(),
            )
        });
    let geometry = if html.is_empty() {
        KanbanPreparedLabelGeometry::empty()
    } else {
        let raw_metrics = markdown.measure_html(measurer, &html, style, None);
        let wrapped = max_width > 0.0 && raw_metrics.width > max_width;
        let metrics = if wrapped {
            markdown.measure_html(measurer, &html, style, Some(max_width))
        } else {
            raw_metrics
        };
        KanbanPreparedLabelGeometry {
            content_height: metrics.height,
            foreign_object_width: if wrapped {
                max_width.max(0.0)
            } else {
                metrics.width.max(0.0)
            },
            wrapped,
        }
    };
    KanbanPreparedMetadataLabel {
        geometry,
        visible_style_facts: if capture_style_facts {
            crate::text::VisibleTextStyleFacts::from_xhtml_fragment(&html)
        } else {
            crate::text::VisibleTextStyleFacts::not_requested()
        },
        html,
    }
}

fn kanban_layout_work_units(model: &KanbanDiagramRenderModel) -> usize {
    let sections = model.nodes.iter().filter(|node| node.is_group).count();
    let items = model
        .nodes
        .iter()
        .filter(|node| node.parent_id.is_some())
        .count();
    let non_empty_detail_labels = model
        .nodes
        .iter()
        .filter(|node| node.parent_id.is_some())
        .fold(0usize, |count, node| {
            count
                .saturating_add(usize::from(
                    node.ticket.as_deref().is_some_and(|text| !text.is_empty()),
                ))
                .saturating_add(usize::from(
                    node.assigned
                        .as_deref()
                        .is_some_and(|text| !text.is_empty()),
                ))
        });
    let detail_measurement_work =
        non_empty_detail_labels.saturating_mul(KANBAN_DETAIL_LABEL_MEASUREMENT_PASSES);

    model
        .nodes
        .len()
        .saturating_mul(2)
        .saturating_add(sections.saturating_mul(2))
        .saturating_add(items.saturating_mul(3))
        .saturating_add(detail_measurement_work)
}

#[cfg(test)]
pub(crate) fn layout_kanban_diagram_typed(
    model: &KanbanDiagramRenderModel,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<KanbanDiagramLayout> {
    let effective_config = crate::test_support::presentation_config(effective_config.clone());
    let work_meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
    prepare_kanban_diagram_typed_with_work_meter(
        model,
        &effective_config,
        None,
        measurer,
        &work_meter,
        &crate::LayoutOptions::default(),
    )
    .map(|prepared| prepared.layout)
}

/// Prepares a Kanban model under the cumulative work meter owned by the render operation.
pub(crate) fn prepare_kanban_diagram_typed_with_work_meter(
    model: &KanbanDiagramRenderModel,
    effective_config: &merman_model::presentation::PresentationConfig,
    theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
    measurer: &dyn TextMeasurer,
    work_meter: &OperationWorkMeter,
    options: &crate::LayoutOptions,
) -> Result<KanbanPreparedArtifact> {
    work_meter
        .policy()
        .check_model_complexity(merman_model::resources::model_complexity_from_kanban(model))?;
    work_meter.charge(kanban_layout_work_units(model))?;
    let typography = KanbanTypographyPlan::resolve(theme, effective_config);
    let capture_style_facts = typography.typography_requested()
        || theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    crate::diagram_theme::FamilyThemeMechanism::RuleFacet {
                        target: crate::diagram_theme::ThemeTarget::TaskLabel
                            | crate::diagram_theme::ThemeTarget::Text,
                        ..
                    } | crate::diagram_theme::FamilyThemeMechanism::OrdinalPalette {
                        target: crate::diagram_theme::ThemeTarget::TaskLabel
                            | crate::diagram_theme::ThemeTarget::Text
                    } | crate::diagram_theme::FamilyThemeMechanism::EffectBinding {
                        target: crate::diagram_theme::ThemeTarget::TaskLabel
                            | crate::diagram_theme::ThemeTarget::Text,
                        ..
                    }
                )
            })
        });
    let config_view = KanbanConfigView::new(effective_config.as_value());
    let cfg = config_view.layout_settings_with_resolved_typography(
        typography.font_family_css(),
        typography.font_size_px(),
    );
    let ticket_base_url = config_view.ticket_base_url();
    let section_width = cfg.section_width;
    let viewbox_padding = cfg.viewbox_padding;
    let padding = KANBAN_SECTION_PADDING_PX;
    let section_rect_y = -(section_width * 3.0) / 2.0;

    let legend_style = cfg.text_style;
    let markdown = KanbanMarkdown::new(effective_config);

    let mut section_inputs = Vec::new();
    let mut node_index = 0;
    while node_index < model.nodes.len() {
        if !model.nodes[node_index].is_group {
            node_index += 1;
            continue;
        }
        let item_start = node_index + 1;
        let mut item_end = item_start;
        while item_end < model.nodes.len() && !model.nodes[item_end].is_group {
            item_end += 1;
        }
        section_inputs.push((&model.nodes[node_index], item_start..item_end));
        node_index = item_end;
    }
    let item_capacity = section_inputs
        .iter()
        .map(|(_, items)| {
            model.nodes[items.start..items.end]
                .iter()
                .filter(|node| node.parent_id.is_some())
                .count()
        })
        .sum();
    let item_width = (section_width - 1.5 * padding).max(1.0);
    let item_inner_max_width = (item_width - padding).max(0.0);
    let mut task_occurrences = Vec::with_capacity(if theme.is_some() { item_capacity } else { 0 });
    let mut prepared_task_labels = Vec::with_capacity(item_capacity);
    for (_, items) in &section_inputs {
        for node in model.nodes[items.start..items.end]
            .iter()
            .filter(|node| node.parent_id.is_some())
        {
            let prepared_title = prepare_kanban_title_label(
                &markdown,
                measurer,
                &node.label,
                &legend_style,
                item_inner_max_width,
                capture_style_facts,
            );
            let prepared_ticket = prepare_kanban_metadata_label(
                &markdown,
                measurer,
                node.ticket.as_deref(),
                &legend_style,
                item_inner_max_width,
                capture_style_facts,
            );
            let prepared_assigned = prepare_kanban_metadata_label(
                &markdown,
                measurer,
                node.assigned.as_deref(),
                &legend_style,
                item_inner_max_width,
                capture_style_facts,
            );
            if theme.is_some() {
                let label_facts = [
                    &prepared_title.visible_style_facts,
                    &prepared_ticket.visible_style_facts,
                    &prepared_assigned.visible_style_facts,
                ];
                task_occurrences.push(KanbanTaskOccurrence::new(
                    label_facts.iter().any(|facts| facts.has_visible_runs()),
                    label_facts
                        .iter()
                        .any(|facts| facts.parse_valid() && facts.inherited_color_run_count() > 0),
                ));
            }
            prepared_task_labels.push((prepared_title, prepared_ticket, prepared_assigned));
        }
    }
    if theme.is_some() {
        debug_assert_eq!(task_occurrences.len(), item_capacity);
    }
    debug_assert_eq!(prepared_task_labels.len(), item_capacity);
    let mut task_theme = match theme {
        Some(theme) => KanbanTaskTheme::resolve_with_typography(
            theme,
            task_occurrences,
            effective_config,
            typography,
            work_meter,
        )?,
        None => KanbanTaskTheme::baseline(item_capacity, typography),
    };
    let mut max_label_height = KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX;
    let mut sections: Vec<KanbanSectionLayout> = Vec::with_capacity(section_inputs.len());
    let mut items: Vec<KanbanItemLayout> = Vec::with_capacity(item_capacity);
    let mut prepared_sections = Vec::with_capacity(section_inputs.len());
    let mut prepared_items = Vec::with_capacity(item_capacity);
    let mut prepared_task_labels = prepared_task_labels.into_iter();

    for (i, (section, _)) in section_inputs.iter().enumerate() {
        let index = (i + 1) as i64;
        let center_x = section_width * (index as f64) + ((index - 1) as f64 * padding) / 2.0;
        let center_y = 0.0;

        let prepared_label = prepare_kanban_markdown_label(
            &markdown,
            measurer,
            &section.label,
            &legend_style,
            section_width,
            capture_style_facts,
        );
        let label_height = prepared_label
            .geometry
            .content_height
            .max(KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX);
        max_label_height = max_label_height.max(label_height);

        sections.push(KanbanSectionLayout {
            id: section.id.clone(),
            label: section.label.clone(),
            index,
            center_x,
            center_y,
            width: section_width,
            rect_y: section_rect_y,
            rect_height: (section_width * 3.0).max(1.0),
            rx: 5.0,
            ry: 5.0,
            label_width: prepared_label.geometry.foreign_object_width,
            label_height,
        });
        prepared_sections.push(prepared_label);
    }

    let mut section_item_ranges = (options.adaptation == crate::LayoutAdaptation::FitWidth)
        .then(|| Vec::with_capacity(sections.len()));
    for (section, (_, item_range)) in sections.iter_mut().zip(&section_inputs) {
        let item_start = items.len();
        let top = section_rect_y + max_label_height;
        let mut y = top;

        for item in model.nodes[item_range.start..item_range.end]
            .iter()
            .filter(|node| node.parent_id.is_some())
        {
            let width = item_width;

            // Mermaid's kanban items are rendered via `kanbanItem.ts`, which uses HTML labels for
            // the title and applies `max-width` clamping when the content needs wrapping. Mirror
            // that behavior so item heights match the upstream bbox-based layout.
            let (prepared_title, prepared_ticket, prepared_assigned) = prepared_task_labels
                .next()
                .expect("Kanban task label preparation must match visible item order");

            let detail_height = prepared_ticket
                .geometry
                .content_height
                .max(prepared_assigned.geometry.content_height);
            let height = prepared_title.geometry.content_height
                + 2.0 * KANBAN_ITEM_LABEL_PADDING_Y_PX
                + detail_height / 2.0;

            let center_x = section.center_x;
            let center_y = y + height / 2.0;

            let item_index = items.len();
            let radius = task_theme
                .radius_px(item_index)
                .unwrap_or(theme::MERMAID_TASK_RADIUS_PX);
            items.push(KanbanItemLayout {
                id: item.id.clone(),
                label: item.label.clone(),
                parent_id: section.id.clone(),
                center_x,
                center_y,
                width,
                height: height.max(1.0),
                rx: radius,
                ry: radius,
                ticket: item.ticket.clone(),
                assigned: item.assigned.clone(),
                priority: item.priority.clone(),
                icon: item.icon.clone(),
            });
            prepared_items.push(KanbanPreparedItem {
                title: prepared_title,
                ticket: prepared_ticket,
                assigned: prepared_assigned,
                ticket_link: prepare_kanban_ticket_link(
                    ticket_base_url.as_deref(),
                    item.ticket.as_deref(),
                    effective_config,
                ),
            });

            y = center_y + height / 2.0 + padding / 2.0;
        }

        let height = (y - top + 3.0 * padding).max(50.0)
            + (max_label_height - KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX);
        section.rect_height = height.max(1.0);
        if let Some(ranges) = &mut section_item_ranges {
            ranges.push(item_start..items.len());
        }
    }
    debug_assert!(prepared_task_labels.next().is_none());

    if let Some(ranges) = section_item_ranges {
        let gap = padding / 2.0;
        let inner_width = (options.container_width - 2.0 * viewbox_padding).max(0.0);
        let capacity = (((inner_width + gap) / (section_width + gap)).floor() as usize)
            .max(1)
            .min(sections.len().max(1));
        let mut row_offset = 0.0;
        let mut row_height: f64 = 0.0;
        for (index, (section, item_range)) in sections.iter_mut().zip(ranges).enumerate() {
            work_meter.charge(1)?;
            let column = index % capacity;
            if column == 0 && index > 0 {
                row_offset += row_height + padding;
                row_height = 0.0;
            }
            row_height = row_height.max(section.rect_height);
            if index >= capacity {
                let x = section_width * (column + 1) as f64 + column as f64 * gap;
                let dx = x - section.center_x;
                section.center_x = x;
                section.center_y += row_offset;
                section.rect_y += row_offset;
                for item in &mut items[item_range] {
                    work_meter.charge(1)?;
                    item.center_x += dx;
                    item.center_y += row_offset;
                }
            }
        }
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for s in &sections {
        let left = s.center_x - s.width / 2.0;
        let right = left + s.width;
        let top = s.rect_y;
        let bottom = s.rect_y + s.rect_height;
        min_x = min_x.min(left);
        min_y = min_y.min(top);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }
    for n in &items {
        let left = n.center_x - n.width / 2.0;
        let right = n.center_x + n.width / 2.0;
        let top = n.center_y - n.height / 2.0;
        let bottom = n.center_y + n.height / 2.0;
        min_x = min_x.min(left);
        min_y = min_y.min(top);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }

    let bounds = if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()
    {
        Some(Bounds {
            min_x: min_x - viewbox_padding,
            min_y: min_y - viewbox_padding,
            max_x: max_x + viewbox_padding,
            max_y: max_y + viewbox_padding,
        })
    } else {
        None
    };

    let adaptation = if options.adaptation == crate::LayoutAdaptation::FitWidth {
        let actual_width = match &bounds {
            Some(bounds) => bounds.max_x - bounds.min_x,
            None if sections.is_empty() => 100.0,
            None => f64::NAN,
        };
        if !actual_width.is_finite() {
            return Err(crate::Error::InvalidModel {
                message: "fit-width Kanban bounds exceed finite layout coordinates".to_owned(),
            });
        }
        Some(crate::LayoutAdaptationObservation::fit_width(
            options.container_width,
            actual_width.max(1.0),
        ))
    } else {
        None
    };
    let layout = KanbanDiagramLayout {
        bounds,
        section_width,
        padding,
        max_label_height,
        viewbox_padding,
        use_max_width: cfg.use_max_width,
        sections,
        items,
    };
    let text_paint = KanbanTextPaintPlan::resolve(
        theme,
        effective_config,
        &prepared_sections,
        &prepared_items,
        &task_theme,
        work_meter,
    )?;
    let css = KanbanCssBinding::resolve(
        effective_config.as_value(),
        &task_theme,
        text_paint.fill_css(),
    )?;
    task_theme.bind_terminal_paints(
        effective_config
            .get_bool("darkMode")
            .or_else(|| effective_config.get_bool("themeVariables.darkMode"))
            .unwrap_or(false),
    )?;
    Ok(KanbanPreparedArtifact {
        layout,
        adaptation,
        sections: prepared_sections,
        items: prepared_items,
        task_theme,
        text_paint,
        css,
        _retained_text: None,
    })
}

#[cfg(test)]
pub(crate) fn prepare_kanban_artifact_from_layout_for_test(
    layout: &KanbanDiagramLayout,
    effective_config: &merman_model::presentation::PresentationConfig,
    measurer: &dyn TextMeasurer,
) -> Result<KanbanPreparedArtifact> {
    let config_view = KanbanConfigView::new(effective_config.as_value());
    let typography = KanbanTypographyPlan::resolve(None, effective_config);
    let settings = config_view.layout_settings_with_resolved_typography(
        typography.font_family_css(),
        typography.font_size_px(),
    );
    let ticket_base_url = config_view.ticket_base_url();
    let markdown = KanbanMarkdown::new(effective_config);
    let sections: Vec<KanbanPreparedMarkdownLabel> = layout
        .sections
        .iter()
        .map(|section| {
            prepare_kanban_markdown_label(
                &markdown,
                measurer,
                &section.label,
                &settings.text_style,
                section.width,
                false,
            )
        })
        .collect();
    let items: Vec<KanbanPreparedItem> = layout
        .items
        .iter()
        .map(|item| KanbanPreparedItem {
            title: prepare_kanban_title_label(
                &markdown,
                measurer,
                &item.label,
                &settings.text_style,
                (item.width - KANBAN_SECTION_PADDING_PX).max(0.0),
                false,
            ),
            ticket: prepare_kanban_metadata_label(
                &markdown,
                measurer,
                item.ticket.as_deref(),
                &settings.text_style,
                (item.width - KANBAN_SECTION_PADDING_PX).max(0.0),
                false,
            ),
            assigned: prepare_kanban_metadata_label(
                &markdown,
                measurer,
                item.assigned.as_deref(),
                &settings.text_style,
                (item.width - KANBAN_SECTION_PADDING_PX).max(0.0),
                false,
            ),
            ticket_link: prepare_kanban_ticket_link(
                ticket_base_url.as_deref(),
                item.ticket.as_deref(),
                effective_config,
            ),
        })
        .collect();
    let task_theme = KanbanTaskTheme::baseline(layout.items.len(), typography);
    let css = KanbanCssBinding::resolve(effective_config.as_value(), &task_theme, None)?;
    Ok(KanbanPreparedArtifact {
        layout: layout.clone(),
        adaptation: None,
        sections,
        items,
        task_theme,
        text_paint: KanbanTextPaintPlan::baseline(),
        css,
        _retained_text: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        kanban_layout_work_units, layout_kanban_diagram_typed,
        prepare_kanban_diagram_typed_with_work_meter, replace_first_like_javascript,
    };
    use crate::Error;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use merman_model::families::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
    use serde_json::json;
    use std::cell::Cell;

    fn section(id: &str, label: &str) -> KanbanRenderNode {
        let mut node = KanbanRenderNode::new(id, label);
        node.is_group = true;
        node
    }

    fn item(id: &str, label: &str, parent_id: &str) -> KanbanRenderNode {
        let mut node = KanbanRenderNode::new(id, label);
        node.parent_id = Some(parent_id.to_string());
        node
    }

    fn item_with_details(
        id: &str,
        label: &str,
        parent_id: &str,
        ticket: Option<&str>,
        assigned: Option<&str>,
    ) -> KanbanRenderNode {
        let mut node = item(id, label, parent_id);
        node.ticket = ticket.map(str::to_owned);
        node.assigned = assigned.map(str::to_owned);
        node
    }

    #[derive(Default)]
    struct CountingTextMeasurer {
        calls: Cell<usize>,
        inner: DeterministicTextMeasurer,
    }

    impl TextMeasurer for CountingTextMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.calls.set(self.calls.get().saturating_add(1));
            self.inner.measure(text, style)
        }
    }

    #[test]
    fn kanban_geometry_constants_match_mermaid() {
        assert_eq!(super::KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX, 25.0);
        assert_eq!(super::KANBAN_SECTION_PADDING_PX, 10.0);
        assert_eq!(super::KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX, 24.0);
        assert_eq!(super::KANBAN_ITEM_LABEL_PADDING_Y_PX, 10.0);
    }

    #[test]
    fn kanban_prepared_text_owner_is_charged_and_released_without_html_payloads() {
        use std::sync::Arc;

        let prepare = |meter: &Arc<OperationWorkMeter>| {
            prepare_kanban_diagram_typed_with_work_meter(
                &KanbanDiagramRenderModel { nodes: Vec::new() },
                &merman_model::presentation::PresentationConfig::default(),
                None,
                &DeterministicTextMeasurer::default(),
                meter,
                &crate::LayoutOptions::default(),
            )
            .unwrap()
            .with_retained_text_accounting(meter)
        };
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let artifact = prepare(&meter).unwrap();
        assert!(meter.prepared_text_retained_bytes() > 0);
        drop(artifact);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);

        let limited = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                .unwrap(),
        ));
        let error = prepare(&limited).unwrap_err();
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected prepared-text owner rejection");
        };
        assert_eq!(
            error.limit,
            ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
        );
        assert_eq!(limited.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn kanban_ticket_hrefs_count_every_retained_url_and_release_on_rejection() {
        use std::sync::Arc;

        let model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item_with_details("first", "Task", "todo", Some("MC-1"), None),
                item_with_details("second", "Task", "todo", Some("MC-2"), None),
                item_with_details("third", "Task", "todo", Some("MC-3"), None),
            ],
        };
        let short_url = "https://example.invalid/#TICKET#?owner=a&team=b";
        let long_url = format!(
            "https://example.invalid/{}/#TICKET#?owner=a&team=b",
            "a".repeat(8192)
        );
        for security_level in ["strict", "loose"] {
            let prepare = |ticket_base_url: &str, policy| {
                let config = crate::test_support::presentation_config(json!({
                    "securityLevel": security_level,
                    "kanban": { "ticketBaseUrl": ticket_base_url }
                }));
                let meter = Arc::new(OperationWorkMeter::new(policy));
                let artifact = prepare_kanban_diagram_typed_with_work_meter(
                    &model,
                    &config,
                    None,
                    &DeterministicTextMeasurer::default(),
                    &meter,
                    &crate::LayoutOptions::default(),
                )
                .and_then(|artifact| artifact.with_retained_text_accounting(&meter));
                (artifact, meter)
            };
            let policy = RenderResourcePolicy::unbounded_for_trusted_input();
            let (short, short_meter) = prepare(short_url, policy);
            let short = short.unwrap();
            let short_bytes = short_meter.prepared_text_retained_bytes();
            let (long, long_meter) = prepare(&long_url, policy);
            let long = long.unwrap();
            let long_bytes = long_meter.prepared_text_retained_bytes();
            assert_eq!(long.items.len(), 3);
            assert_eq!(
                long_bytes - short_bytes,
                long.items.len() * (long_url.len() - short_url.len()),
                "each card retains its own serialized href: {security_level}"
            );
            for (index, item) in long.items.iter().enumerate() {
                let href = item.ticket_link.as_ref().unwrap().href.as_ref().unwrap();
                assert_eq!(
                    href.as_serialized_str(),
                    long_url
                        .replacen("#TICKET#", &format!("MC-{}", index + 1), 1)
                        .replace('&', "&amp;"),
                    "prepared href remains serialized exactly once: {security_level}"
                );
            }
            drop(short);
            drop(long);
            assert_eq!(short_meter.prepared_text_retained_bytes(), 0);
            assert_eq!(long_meter.prepared_text_retained_bytes(), 0);

            let (exact, exact_meter) = prepare(
                &long_url,
                policy
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, long_bytes)
                    .unwrap(),
            );
            let exact = exact.expect("the exact retained URL budget must admit all cards");
            assert_eq!(exact_meter.prepared_text_retained_bytes(), long_bytes);
            drop(exact);
            assert_eq!(exact_meter.prepared_text_retained_bytes(), 0);

            let (rejected, rejected_meter) = prepare(
                &long_url,
                policy
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, short_bytes)
                    .unwrap(),
            );
            let Error::ResourceLimitExceeded(error) = rejected.unwrap_err() else {
                panic!("long prepared hrefs must exceed the short URL budget");
            };
            assert_eq!(error.limit, "max_prepared_text_retained_bytes");
            assert_eq!(error.actual, long_bytes);
            assert_eq!(rejected_meter.prepared_text_retained_bytes(), 0);
            assert_eq!(rejected_meter.prepared_text_retained_bytes_peak(), 0);
        }
    }

    #[test]
    fn kanban_retained_admission_observes_cancellation_before_the_budget() {
        use std::sync::Arc;

        let control = merman_model::OperationControl::new();
        let meter = Arc::new(OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                .unwrap(),
            control.clone(),
        ));
        let existing = meter.reserve_prepared_text_retained_bytes(1).unwrap();
        let model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item_with_details("task", "Task", "todo", Some("MC-1"), None),
            ],
        };
        let config = crate::test_support::presentation_config(json!({
            "kanban": { "ticketBaseUrl": "https://example.invalid/#TICKET#" }
        }));
        let artifact = prepare_kanban_diagram_typed_with_work_meter(
            &model,
            &config,
            None,
            &DeterministicTextMeasurer::default(),
            &meter,
            &crate::LayoutOptions::default(),
        )
        .unwrap();
        control.cancel();
        let Error::Cancelled(error) = artifact.with_retained_text_accounting(&meter).unwrap_err()
        else {
            panic!("cancellation must win over the insufficient retained budget");
        };
        assert_eq!(error.phase, merman_model::OperationPhase::Layout);
        assert_eq!(error.reason, merman_model::CancelReason::Requested);
        assert_eq!(meter.prepared_text_retained_bytes(), 1);
        assert_eq!(meter.prepared_text_retained_bytes_peak(), 1);
        drop(existing);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn no_consumer_kanban_preparation_omits_fact_collection_but_measures_details() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "**Todo**"),
                item_with_details("task", "Task", "todo", Some("MC-2038"), Some("Alice")),
            ],
        };
        let measurer = CountingTextMeasurer::default();
        let (prepared, collections) = crate::text::VisibleTextStyleFacts::count_collections(|| {
            prepare_kanban_diagram_typed_with_work_meter(
                &model,
                &merman_model::presentation::PresentationConfig::default(),
                None,
                &measurer,
                &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
                &crate::LayoutOptions::default(),
            )
            .unwrap()
        });
        assert!(measurer.calls.get() >= 4);
        assert!(prepared.items[0].ticket.geometry.content_height > 0.0);
        assert!(prepared.items[0].assigned.geometry.content_height > 0.0);
        assert_eq!(collections, 0);
    }

    #[test]
    fn unthemed_preparation_preserves_visible_item_order_and_configured_typography() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![
                item("orphan", "Before any section", "todo"),
                section("todo", "Todo"),
                KanbanRenderNode::new("detached", "No parent"),
                item("first", "First task", "todo"),
                section("done", "Done"),
                item("second", "Second task", "done"),
            ],
        };
        let config = crate::test_support::presentation_config(json!({
            "fontFamily": "RootFallback",
            "themeVariables": {
                "fontFamily": "Configured, monospace",
                "fontSize": "22px"
            }
        }));
        let prepared = prepare_kanban_diagram_typed_with_work_meter(
            &model,
            &config,
            None,
            &CountingTextMeasurer::default(),
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            &crate::LayoutOptions::default(),
        )
        .unwrap();

        let ids: Vec<_> = prepared
            .layout
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids, ["first", "second"]);
        assert_eq!(prepared.task_theme().item_count(), 2);
        assert_eq!(prepared.items.len(), 2);
        assert!(
            prepared
                .layout
                .items
                .iter()
                .all(|item| item.rx == 5.0 && item.ry == 5.0)
        );
        assert_eq!(prepared.task_theme().radius_px(2), None);
        assert_eq!(
            prepared.task_theme().font_family_css(),
            "Configured,monospace"
        );
        assert_eq!(prepared.task_theme().typography_font_size_px(), 22.0);
        assert!(
            prepared
                .task_theme()
                .resolved_typography_for_css()
                .is_none()
        );
    }

    #[test]
    fn kanban_layout_work_units_charge_each_non_empty_detail_label() {
        let cases = [
            ("no details", None, None, 9),
            ("ticket only", Some("MC-2038"), None, 11),
            ("assigned only", None, Some("Alice"), 11),
            ("ticket and assigned", Some("MC-2038"), Some("Alice"), 13),
            ("empty details", Some(""), Some(""), 9),
        ];

        for (name, ticket, assigned, expected) in cases {
            let model = KanbanDiagramRenderModel {
                nodes: vec![
                    section("todo", "Todo"),
                    item_with_details("task-1", "Task", "todo", ticket, assigned),
                ],
            };

            assert_eq!(kanban_layout_work_units(&model), expected, "{name}");
        }
    }

    #[test]
    fn kanban_layout_work_budget_is_charged_before_detail_measurement() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item_with_details("task-1", "Task", "todo", Some("MC-2038"), Some("Alice")),
            ],
        };
        let expected_work = kanban_layout_work_units(&model);
        let measurer = CountingTextMeasurer::default();
        let narrow_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work - 1)
            .unwrap();
        let narrow_meter = OperationWorkMeter::new(narrow_policy);
        let error = prepare_kanban_diagram_typed_with_work_meter(
            &model,
            &crate::test_support::presentation_config(json!({})),
            None,
            &measurer,
            &narrow_meter,
            &crate::LayoutOptions::default(),
        )
        .unwrap_err();

        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured layout work rejection");
        };
        assert_eq!(error.actual, expected_work);
        assert_eq!(error.max, expected_work - 1);
        assert_eq!(narrow_meter.used(), 0);
        assert_eq!(measurer.calls.get(), 0);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work)
            .unwrap();
        let exact_meter = OperationWorkMeter::new(exact_policy);
        let exact_measurer = CountingTextMeasurer::default();
        prepare_kanban_diagram_typed_with_work_meter(
            &model,
            &crate::test_support::presentation_config(json!({})),
            None,
            &exact_measurer,
            &exact_meter,
            &crate::LayoutOptions::default(),
        )
        .unwrap();

        assert_eq!(exact_meter.used(), expected_work);
        assert!(exact_measurer.calls.get() > 0);
    }

    #[test]
    fn kanban_ticket_placeholder_replacement_matches_javascript_string_replace() {
        assert_eq!(
            replace_first_like_javascript("pre#TICKET#post#TICKET#", "#TICKET#", "MC-2038"),
            "preMC-2038post#TICKET#"
        );
        assert_eq!(
            replace_first_like_javascript("left#TICKET#right", "#TICKET#", "$$"),
            "left$right"
        );
        assert_eq!(
            replace_first_like_javascript("left#TICKET#right", "#TICKET#", "$&"),
            "left#TICKET#right"
        );
        assert_eq!(
            replace_first_like_javascript("left#TICKET#right", "#TICKET#", "$`"),
            "leftleftright"
        );
        assert_eq!(
            replace_first_like_javascript("left#TICKET#right", "#TICKET#", "$'"),
            "leftrightright"
        );
        assert_eq!(
            replace_first_like_javascript("https://example.test/tickets", "#TICKET#", "$&"),
            "https://example.test/tickets"
        );
    }

    #[test]
    fn kanban_layout_uses_mermaid_padding() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                section("doing", "Doing"),
                item("task-1", "Task", "todo"),
            ],
        };
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };

        let layout = layout_kanban_diagram_typed(&model, &json!({}), &measurer).unwrap();

        assert_eq!(layout.padding, super::KANBAN_SECTION_PADDING_PX);
        assert!(layout.use_max_width);
        assert_eq!(
            layout.items[0].width,
            layout.section_width - 1.5 * super::KANBAN_SECTION_PADDING_PX
        );
    }

    #[test]
    fn kanban_layout_measures_rendered_markdown_instead_of_source_markers() {
        let markdown_model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item("task-1", "*aaaa aaaa aaaaaaa*", "todo"),
                item("task-2", "Next", "todo"),
            ],
        };
        let plain_model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item("task-1", "aaaa aaaa aaaaaaa", "todo"),
                item("task-2", "Next", "todo"),
            ],
        };
        let measurer = DeterministicTextMeasurer::default();

        let markdown_layout =
            layout_kanban_diagram_typed(&markdown_model, &json!({}), &measurer).unwrap();
        let plain_layout =
            layout_kanban_diagram_typed(&plain_model, &json!({}), &measurer).unwrap();

        assert_eq!(
            markdown_layout.items[0].height,
            plain_layout.items[0].height
        );
        assert_eq!(
            markdown_layout.items[1].center_y,
            plain_layout.items[1].center_y
        );
        assert_eq!(
            markdown_layout.sections[0].rect_height,
            plain_layout.sections[0].rect_height
        );
        let markdown_bounds = markdown_layout.bounds.as_ref().unwrap();
        let plain_bounds = plain_layout.bounds.as_ref().unwrap();
        assert_eq!(markdown_bounds.min_x, plain_bounds.min_x);
        assert_eq!(markdown_bounds.min_y, plain_bounds.min_y);
        assert_eq!(markdown_bounds.max_x, plain_bounds.max_x);
        assert_eq!(markdown_bounds.max_y, plain_bounds.max_y);
    }

    #[test]
    fn kanban_layout_uses_mermaid_mindmap_viewport_config_precedence() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![section("todo", "Todo")],
        };
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };

        let layout = layout_kanban_diagram_typed(
            &model,
            &json!({
                "mindmap": {
                    "padding": 3,
                    "useMaxWidth": false
                },
                "kanban": {
                    "padding": 12,
                    "useMaxWidth": true
                }
            }),
            &measurer,
        )
        .unwrap();

        assert_eq!(layout.viewbox_padding, 3.0);
        assert!(!layout.use_max_width);
    }
}
