use super::LayeredRelationSummaryReason;
use super::RelationGraphLine;
#[cfg(test)]
use super::{RelationGraphBox, RelationGraphLabel, RelationGraphLabelPlan, stacked_box_extent};
use crate::color::AsciiColorRole;
use crate::options::AsciiRenderOptions;
#[cfg(test)]
use crate::options::TerminalWidthProfile;
use crate::resource::{LogicalExtent, ResourceContext};
use crate::safe_text::DeferredTextLine;
#[cfg(test)]
use crate::safe_text::{ComposedTextPlan, DeferredTextRegistry};
use crate::text::{StyledLine, display_width_with_profile};
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphSummaryRow {
    source: DeferredTextLine,
    connector: DeferredTextLine,
    target: DeferredTextLine,
    label: Rc<Vec<DeferredTextLine>>,
}

impl RelationGraphSummaryRow {
    pub(crate) const fn new(
        source: DeferredTextLine,
        connector: DeferredTextLine,
        target: DeferredTextLine,
        label: Rc<Vec<DeferredTextLine>>,
    ) -> Self {
        Self {
            source,
            connector,
            target,
            label,
        }
    }
}

#[cfg(test)]
pub(crate) fn test_summary_row<'a>(
    source: &'a str,
    connector: &'a str,
    target: &'a str,
    label: Option<&super::RelationGraphLabel>,
    width_profile: TerminalWidthProfile,
    deferred: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> crate::Result<RelationGraphSummaryRow> {
    let source = deferred.try_register(
        ComposedTextPlan::try_new(resources, 1, |push| push(source))?,
        width_profile,
        resources,
    )?;
    let connector = deferred.try_register(
        ComposedTextPlan::try_new(resources, 1, |push| push(connector))?,
        width_profile,
        resources,
    )?;
    let target = deferred.try_register(
        ComposedTextPlan::try_new(resources, 1, |push| push(target))?,
        width_profile,
        resources,
    )?;
    let label = label
        .map(RelationGraphLabel::shared_lines)
        .unwrap_or_else(|| Rc::new(Vec::new()));
    Ok(RelationGraphSummaryRow::new(
        source, connector, target, label,
    ))
}

#[cfg(test)]
pub(crate) fn render_stacked_boxes_with_relation_summary(
    boxes: &[RelationGraphBox],
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
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
    super::render_lines_with_deferred_options(&lines, options, resources, deferred)
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
    let height = measurement.extent.height();

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| super::layout_allocation_failed())?;
    if let Some(reason) = measurement.diagnostic_reason {
        let mut diagnostic = StyledLine::with_resources(options.terminal_width_profile, resources);
        diagnostic.try_push_role_text("reason: ", AsciiColorRole::Diagnostic)?;
        diagnostic.try_push_role_text(
            relation_summary_reason_text(reason),
            AsciiColorRole::Diagnostic,
        )?;
        lines.push(RelationGraphLine::from_styled(diagnostic));
    }

    for row in rows {
        let mut line = StyledLine::with_resources(options.terminal_width_profile, resources);
        line.try_push_deferred_text(&row.source, AsciiColorRole::EdgeLabel)?;
        line.try_push_role_repeat(
            ' ',
            source_width - row.source.width(),
            AsciiColorRole::EdgeLabel,
        )?;
        line.try_push_role_repeat(' ', 1, AsciiColorRole::EdgeLabel)?;
        line.try_push_deferred_text(&row.connector, AsciiColorRole::EdgeLabel)?;
        line.try_push_role_repeat(
            ' ',
            connector_width - row.connector.width(),
            AsciiColorRole::EdgeLabel,
        )?;
        line.try_push_role_repeat(' ', 1, AsciiColorRole::EdgeLabel)?;
        line.try_push_deferred_text(&row.target, AsciiColorRole::EdgeLabel)?;
        line.try_push_role_repeat(
            ' ',
            target_width - row.target.width(),
            AsciiColorRole::EdgeLabel,
        )?;

        if let Some(first) = row.label.first() {
            line.try_push_role_text(" : ", AsciiColorRole::EdgeLabel)?;
            line.try_push_deferred_text(first, AsciiColorRole::EdgeLabel)?;
        }
        lines.push(RelationGraphLine::from_styled(line));

        for continuation in row.label.iter().skip(1) {
            let mut line = StyledLine::with_resources(options.terminal_width_profile, resources);
            line.try_push_role_repeat(' ', label_prefix_width, AsciiColorRole::EdgeLabel)?;
            line.try_push_deferred_text(continuation, AsciiColorRole::EdgeLabel)?;
            lines.push(RelationGraphLine::from_styled(line));
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
    let source_width = rows.iter().map(|row| row.source.width()).max().unwrap_or(0);
    let connector_width = rows
        .iter()
        .map(|row| row.connector.width())
        .max()
        .unwrap_or(0);
    let target_width = rows.iter().map(|row| row.target.width()).max().unwrap_or(0);
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
        let row_width = if row.label.is_empty() {
            resources.checked_grid_add(base_width, 2)?
        } else {
            height = resources.checked_grid_add(height, row.label.len().saturating_sub(1))?;
            let label_width = row
                .label
                .iter()
                .map(DeferredTextLine::width)
                .max()
                .unwrap_or(0);
            resources.checked_grid_add(label_prefix_width, label_width)?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{AsciiColorTheme, AsciiRgb};
    use crate::{AsciiColorMode, AsciiRenderOptions, AsciiResourcePolicy};

    fn test_resources(policy: AsciiResourcePolicy) -> ResourceContext {
        ResourceContext::new(policy)
    }

    fn rows_with_label<'a>(
        definitions: &[(&'a str, &'a str, &'a str, Option<&RelationGraphLabel>)],
        options: &AsciiRenderOptions,
        deferred: &mut DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Vec<RelationGraphSummaryRow> {
        definitions
            .iter()
            .map(|(source, connector, target, label)| {
                test_summary_row(
                    source,
                    connector,
                    target,
                    *label,
                    options.terminal_width_profile,
                    deferred,
                    resources,
                )
                .expect("test summary row should plan")
            })
            .collect()
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_aligns_columns_and_wraps_labels() {
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let mut deferred = DeferredTextRegistry::new();
        let label_plan = RelationGraphLabelPlan::try_new(
            "receives<br>request",
            TerminalWidthProfile::Unicode,
            &deferred,
            &resources,
        )
        .expect("label should plan");
        let label = label_plan
            .map(|plan| plan.materialize(&mut deferred, &resources))
            .transpose()
            .expect("label should materialize");
        let rows = rows_with_label(
            &[
                ("Gateway", "-->", "Service", label.as_ref()),
                ("Svc", "-->", "Repo", None),
            ],
            &options,
            &mut deferred,
            &resources,
        );
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            None,
            &options,
            &mut resources,
            &deferred,
        )
        .expect("summary should render");

        assert_eq!(
            rendered,
            format!(
                concat!(
                    "relations:\n",
                    "Gateway --> Service : receives\n",
                    "{}request\n",
                    "{}authored(bytes=19)=\"receives<br>request\"\n",
                    "Svc     --> Repo\n",
                ),
                " ".repeat(22),
                " ".repeat(22),
            )
        );
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_aligns_wide_text_by_display_width() {
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let mut deferred = DeferredTextRegistry::new();
        let label_plan = RelationGraphLabelPlan::try_new(
            "处理🚀<br>完成",
            TerminalWidthProfile::Unicode,
            &deferred,
            &resources,
        )
        .expect("label should plan");
        let label = label_plan
            .map(|plan| plan.materialize(&mut deferred, &resources))
            .transpose()
            .expect("label should materialize");
        let rows = rows_with_label(
            &[
                ("服务", "-->", "Repo", label.as_ref()),
                ("API", "-->", "数据", None),
            ],
            &options,
            &mut deferred,
            &resources,
        );
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            None,
            &options,
            &mut resources,
            &deferred,
        )
        .expect("wide summary should render");

        assert_eq!(
            rendered,
            format!(
                concat!(
                    "relations:\n",
                    "服务 --> Repo : 处理🚀\n",
                    "{}完成\n",
                    "{}authored(bytes=20)=\"处理🚀<br>完成\"\n",
                    "API  --> 数据\n",
                ),
                " ".repeat(16),
                " ".repeat(16),
            )
        );
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_colors_title_and_rows() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Section, AsciiRgb::from_hex24(0x222222))
            .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333));

        let options = AsciiRenderOptions::ascii()
            .with_color_mode(AsciiColorMode::Html)
            .with_color_theme(theme);
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let mut deferred = DeferredTextRegistry::new();
        let label_plan = RelationGraphLabelPlan::try_new(
            "one<br>two",
            TerminalWidthProfile::Unicode,
            &deferred,
            &resources,
        )
        .expect("label should plan");
        let label = label_plan
            .map(|plan| plan.materialize(&mut deferred, &resources))
            .transpose()
            .expect("label should materialize");
        let rows = rows_with_label(
            &[("A", "-->", "B", label.as_ref())],
            &options,
            &mut deferred,
            &resources,
        );
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            None,
            &options,
            &mut resources,
            &deferred,
        )
        .expect("colored summary should render");

        assert!(rendered.contains("<span style=\"color:#222222\">relations:</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">A --&gt; B : one</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">          two</span>"));
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_hides_diagnostics_by_default() {
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let mut deferred = DeferredTextRegistry::new();
        let rows = rows_with_label(
            &[("A", "-->", "B", None)],
            &options,
            &mut deferred,
            &resources,
        );
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            Some(LayeredRelationSummaryReason::Crossing),
            &options,
            &mut resources,
            &deferred,
        )
        .expect("summary should render");

        assert!(!rendered.contains("reason:"), "{rendered}");
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_can_show_diagnostics() {
        let options = AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true);
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let mut deferred = DeferredTextRegistry::new();
        let rows = rows_with_label(
            &[("A", "-->", "B", None)],
            &options,
            &mut deferred,
            &resources,
        );
        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            Some(LayeredRelationSummaryReason::RouteCollision),
            &options,
            &mut resources,
            &deferred,
        )
        .expect("diagnostic summary should render");

        assert_eq!(rendered, "relations:\nreason: route_collision\nA --> B\n");
    }
}
