use super::LayeredRelationSummaryReason;
use super::{RelationGraphBox, RelationGraphLabel, RelationGraphLine, stacked_box_extent};
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{LogicalExtent, ResourceContext};
use crate::safe_text::SafeLine;
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphSummaryRow {
    source: String,
    connector: String,
    target: String,
    label: Option<RelationGraphLabel>,
}

impl RelationGraphSummaryRow {
    pub(crate) fn new(
        source: impl Into<String>,
        connector: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        let source = source.into();
        let connector = connector.into();
        let target = target.into();
        Self {
            source: SafeLine::new(&source).as_str().to_owned(),
            connector: SafeLine::new(&connector).as_str().to_owned(),
            target: SafeLine::new(&target).as_str().to_owned(),
            label: None,
        }
    }

    pub(crate) fn with_label(mut self, label: Option<&RelationGraphLabel>) -> Self {
        self.label = label.cloned();
        self
    }
}

pub(crate) fn render_stacked_boxes_with_relation_summary(
    boxes: &[RelationGraphBox],
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> crate::Result<String> {
    let base_extent = stacked_box_extent(boxes, resources)?;
    let lines = super::render_relation_document_with_summary(
        base_extent,
        rows,
        reason,
        options,
        resources,
        |resources| super::stacked_box_lines(boxes, options.terminal_width_profile, resources),
    )?;
    super::render_lines_with_options(&lines, options, resources)
}

pub(crate) fn relation_summary_extent(
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> crate::Result<LogicalExtent> {
    Ok(measure_relation_summary(rows, reason, options, resources)?.extent)
}

pub(crate) fn relation_summary_lines_for_rows(
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> crate::Result<Vec<RelationGraphLine>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let measurement = measure_relation_summary(rows, reason, options, resources)?;
    resources.charge_layout_work(measurement.extent.cells())?;

    let source_width = measurement.source_width;
    let connector_width = measurement.connector_width;
    let target_width = measurement.target_width;
    let label_prefix_width = measurement.label_prefix_width;
    let diagnostic = measurement
        .diagnostic_reason
        .map(|reason| format!("reason: {}", relation_summary_reason_text(reason)));
    let height = measurement.extent.height();

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| super::layout_allocation_failed())?;
    if let Some(diagnostic) = diagnostic.as_deref() {
        lines.push(RelationGraphLine::try_with_role(
            diagnostic,
            AsciiColorRole::MutedText,
            options.terminal_width_profile,
            resources,
        )?);
    }

    for row in rows {
        let mut line = String::new();
        line.push_str(&pad_right(
            &row.source,
            source_width,
            options.terminal_width_profile,
        ));
        line.push(' ');
        line.push_str(&pad_right(
            &row.connector,
            connector_width,
            options.terminal_width_profile,
        ));
        line.push(' ');
        line.push_str(&pad_right(
            &row.target,
            target_width,
            options.terminal_width_profile,
        ));

        match row.label.as_ref() {
            Some(label) if !label.lines().is_empty() => {
                let label_lines = label.lines();
                line.push_str(" : ");
                line.push_str(&label_lines[0]);
                lines.push(RelationGraphLine::try_with_role(
                    &line,
                    AsciiColorRole::EdgeLabel,
                    options.terminal_width_profile,
                    resources,
                )?);
                for continuation in label_lines.iter().skip(1) {
                    let continuation =
                        format!("{}{}", " ".repeat(label_prefix_width), continuation);
                    lines.push(RelationGraphLine::try_with_role(
                        &continuation,
                        AsciiColorRole::EdgeLabel,
                        options.terminal_width_profile,
                        resources,
                    )?);
                }
            }
            _ => lines.push(RelationGraphLine::try_with_role(
                &line,
                AsciiColorRole::EdgeLabel,
                options.terminal_width_profile,
                resources,
            )?),
        }
    }

    Ok(lines)
}

#[derive(Debug, Clone, Copy)]
struct RelationSummaryMeasurement {
    extent: LogicalExtent,
    source_width: usize,
    connector_width: usize,
    target_width: usize,
    label_prefix_width: usize,
    diagnostic_reason: Option<LayeredRelationSummaryReason>,
}

fn measure_relation_summary(
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> crate::Result<RelationSummaryMeasurement> {
    let source_width = rows
        .iter()
        .map(|row| display_width_with_profile(&row.source, options.terminal_width_profile))
        .max()
        .unwrap_or(0);
    let connector_width = rows
        .iter()
        .map(|row| display_width_with_profile(&row.connector, options.terminal_width_profile))
        .max()
        .unwrap_or(0);
    let target_width = rows
        .iter()
        .map(|row| display_width_with_profile(&row.target, options.terminal_width_profile))
        .max()
        .unwrap_or(0);
    let base_width = resources.checked_grid_add(
        resources.checked_grid_add(source_width, connector_width)?,
        target_width,
    )?;
    let label_prefix_width = resources.checked_grid_add(base_width, 5)?;
    let diagnostic_reason = options
        .relation_summary_diagnostics
        .then_some(reason)
        .flatten();
    let diagnostic_width = if let Some(reason) = diagnostic_reason {
        resources.checked_grid_add(
            display_width_with_profile("reason: ", options.terminal_width_profile),
            display_width_with_profile(
                relation_summary_reason_text(reason),
                options.terminal_width_profile,
            ),
        )?
    } else {
        0
    };
    let mut height = usize::from(diagnostic_reason.is_some());
    let mut width = diagnostic_width;
    for row in rows {
        height = resources.checked_grid_add(height, 1)?;
        let row_width = match row.label.as_ref() {
            Some(label) if !label.lines().is_empty() => {
                height =
                    resources.checked_grid_add(height, label.lines().len().saturating_sub(1))?;
                resources.checked_grid_add(label_prefix_width, label.width())?
            }
            _ => resources.checked_grid_add(base_width, 2)?,
        };
        width = width.max(row_width);
    }
    let extent = resources.grid_extent(width, height)?;
    Ok(RelationSummaryMeasurement {
        extent,
        source_width,
        connector_width,
        target_width,
        label_prefix_width,
        diagnostic_reason,
    })
}

fn relation_summary_reason_text(reason: LayeredRelationSummaryReason) -> &'static str {
    match reason {
        LayeredRelationSummaryReason::Crossing => "crossing",
        LayeredRelationSummaryReason::RouteCollision => "route_collision",
        LayeredRelationSummaryReason::OverlayCollision => "overlay_collision",
    }
}

fn pad_right(text: &str, width: usize, width_profile: TerminalWidthProfile) -> String {
    let text_width = display_width_with_profile(text, width_profile);
    let mut padded = String::from(text);
    debug_assert!(width >= text_width);
    padded.extend(std::iter::repeat_n(' ', width - text_width));
    padded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{AsciiColorTheme, AsciiRgb};
    use crate::{AsciiColorMode, AsciiRenderOptions};

    fn test_resources(options: &AsciiRenderOptions) -> ResourceContext {
        ResourceContext::new(options.resources)
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_aligns_columns_and_wraps_labels() {
        let rows = vec![
            RelationGraphSummaryRow::new("Gateway", "-->", "Service").with_label(
                RelationGraphLabel::new("receives<br>request", TerminalWidthProfile::Unicode)
                    .as_ref(),
            ),
            RelationGraphSummaryRow::new("Svc", "-->", "Repo"),
        ];

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let rendered =
            render_stacked_boxes_with_relation_summary(&[], &rows, None, &options, &mut resources)
                .expect("summary should render");

        assert_eq!(
            rendered,
            format!(
                concat!(
                    "relations:\n",
                    "Gateway --> Service : receives\n",
                    "{}request\n",
                    "Svc     --> Repo\n",
                ),
                " ".repeat(22),
            )
        );
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_aligns_wide_text_by_display_width() {
        let rows = vec![
            RelationGraphSummaryRow::new("服务", "-->", "Repo").with_label(
                RelationGraphLabel::new("处理🚀<br>完成", TerminalWidthProfile::Unicode).as_ref(),
            ),
            RelationGraphSummaryRow::new("API", "-->", "数据"),
        ];

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let rendered =
            render_stacked_boxes_with_relation_summary(&[], &rows, None, &options, &mut resources)
                .expect("wide summary should render");

        assert_eq!(
            rendered,
            format!(
                concat!(
                    "relations:\n",
                    "服务 --> Repo : 处理🚀\n",
                    "{}完成\n",
                    "API  --> 数据\n",
                ),
                " ".repeat(16),
            )
        );
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_colors_title_and_rows() {
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B").with_label(
            RelationGraphLabel::new("one<br>two", TerminalWidthProfile::Unicode).as_ref(),
        )];
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x222222))
            .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333));

        let options = AsciiRenderOptions::ascii()
            .with_color_mode(AsciiColorMode::Html)
            .with_color_theme(theme);
        let mut resources = test_resources(&options);
        let rendered =
            render_stacked_boxes_with_relation_summary(&[], &rows, None, &options, &mut resources)
                .expect("colored summary should render");

        assert!(rendered.contains("<span style=\"color:#222222\">relations:</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">A --&gt; B : one</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">          two</span>"));
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_hides_diagnostics_by_default() {
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B")];

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            Some(LayeredRelationSummaryReason::Crossing),
            &options,
            &mut resources,
        )
        .expect("summary should render");

        assert!(!rendered.contains("reason:"), "{rendered}");
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_can_show_diagnostics() {
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B")];

        let options = AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true);
        let mut resources = test_resources(&options);
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            Some(LayeredRelationSummaryReason::RouteCollision),
            &options,
            &mut resources,
        )
        .expect("diagnostic summary should render");

        assert_eq!(rendered, "relations:\nreason: route_collision\nA --> B\n");
    }
}
