use super::LayeredRelationSummaryReason;
use super::{
    RelationGraphBox, RelationGraphLabel, RelationGraphLine, render_stacked_boxes_with_section,
};
use crate::color::AsciiColorRole;
use crate::options::AsciiRenderOptions;
use crate::text::display_width;

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
        Self {
            source: source.into(),
            connector: connector.into(),
            target: target.into(),
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
) -> String {
    let lines = relation_summary_lines(rows, reason, options);
    render_stacked_boxes_with_section(
        boxes,
        RelationGraphLine::with_role("relations:".to_string(), AsciiColorRole::MutedText),
        &lines,
        options,
    )
}

pub(crate) fn relation_summary_rows_lines<R>(
    relations: &[R],
    options: &AsciiRenderOptions,
    reason: Option<LayeredRelationSummaryReason>,
    build_row: impl FnMut(&R) -> crate::Result<RelationGraphSummaryRow>,
) -> crate::Result<Vec<RelationGraphLine>> {
    let rows = relations
        .iter()
        .map(build_row)
        .collect::<crate::Result<Vec<_>>>()?;

    Ok(relation_summary_lines(&rows, reason, options))
}

fn relation_summary_lines(
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
) -> Vec<RelationGraphLine> {
    if rows.is_empty() {
        return Vec::new();
    }

    let source_width = rows
        .iter()
        .map(|row| display_width(&row.source))
        .max()
        .unwrap_or(0);
    let connector_width = rows
        .iter()
        .map(|row| display_width(&row.connector))
        .max()
        .unwrap_or(0);
    let target_width = rows
        .iter()
        .map(|row| display_width(&row.target))
        .max()
        .unwrap_or(0);
    let label_prefix_width = source_width + connector_width + target_width + 5;

    let mut lines = Vec::new();
    if options.relation_summary_diagnostics
        && let Some(reason) = reason
    {
        lines.push(RelationGraphLine::with_role(
            format!("reason: {}", relation_summary_reason_text(reason)),
            AsciiColorRole::MutedText,
        ));
    }

    for row in rows {
        let mut line = String::new();
        line.push_str(&pad_right(&row.source, source_width));
        line.push(' ');
        line.push_str(&pad_right(&row.connector, connector_width));
        line.push(' ');
        line.push_str(&pad_right(&row.target, target_width));

        match row.label.as_ref() {
            Some(label) if !label.lines().is_empty() => {
                let label_lines = label.lines();
                line.push_str(" : ");
                line.push_str(&label_lines[0]);
                lines.push(RelationGraphLine::with_role(
                    line,
                    AsciiColorRole::EdgeLabel,
                ));
                for continuation in label_lines.iter().skip(1) {
                    lines.push(RelationGraphLine::with_role(
                        format!("{}{}", " ".repeat(label_prefix_width), continuation),
                        AsciiColorRole::EdgeLabel,
                    ));
                }
            }
            _ => lines.push(RelationGraphLine::with_role(
                line,
                AsciiColorRole::EdgeLabel,
            )),
        }
    }

    lines
}

fn relation_summary_reason_text(reason: LayeredRelationSummaryReason) -> String {
    match reason {
        LayeredRelationSummaryReason::Crossing => "crossing".to_string(),
        LayeredRelationSummaryReason::RouteCollision => "route_collision".to_string(),
        LayeredRelationSummaryReason::OverlayCollision => "overlay_collision".to_string(),
        LayeredRelationSummaryReason::GridBudget { actual, limit } => {
            format!("grid_budget actual={actual} limit={limit}")
        }
    }
}

fn pad_right(text: &str, width: usize) -> String {
    let text_width = display_width(text);
    let mut padded = String::from(text);
    padded.extend(std::iter::repeat_n(' ', width.saturating_sub(text_width)));
    padded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{AsciiColorTheme, AsciiRgb};
    use crate::{AsciiColorMode, AsciiRenderOptions};

    #[test]
    fn render_stacked_boxes_with_relation_summary_aligns_columns_and_wraps_labels() {
        let rows = vec![
            RelationGraphSummaryRow::new("Gateway", "-->", "Service")
                .with_label(RelationGraphLabel::new("receives<br>request").as_ref()),
            RelationGraphSummaryRow::new("Svc", "-->", "Repo"),
        ];

        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            None,
            &AsciiRenderOptions::ascii(),
        );

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
            RelationGraphSummaryRow::new("服务", "-->", "Repo")
                .with_label(RelationGraphLabel::new("处理🚀<br>完成").as_ref()),
            RelationGraphSummaryRow::new("API", "-->", "数据"),
        ];

        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            None,
            &AsciiRenderOptions::ascii(),
        );

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
        let rows = vec![
            RelationGraphSummaryRow::new("A", "-->", "B")
                .with_label(RelationGraphLabel::new("one<br>two").as_ref()),
        ];
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x222222))
            .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333));

        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            None,
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

        assert!(rendered.contains("<span style=\"color:#222222\">relations:</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">A --&gt; B : one</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">          two</span>"));
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_hides_diagnostics_by_default() {
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B")];

        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            Some(LayeredRelationSummaryReason::GridBudget {
                actual: 12,
                limit: 1,
            }),
            &AsciiRenderOptions::ascii(),
        );

        assert!(!rendered.contains("reason:"), "{rendered}");
    }

    #[test]
    fn render_stacked_boxes_with_relation_summary_can_show_diagnostics() {
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B")];

        let rendered = render_stacked_boxes_with_relation_summary(
            &[],
            &rows,
            Some(LayeredRelationSummaryReason::GridBudget {
                actual: 12,
                limit: 1,
            }),
            &AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true),
        );

        assert_eq!(
            rendered,
            "relations:\nreason: grid_budget actual=12 limit=1\nA --> B\n"
        );
    }
}
