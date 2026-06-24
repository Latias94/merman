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
    options: &AsciiRenderOptions,
) -> String {
    let lines = relation_summary_lines(rows);
    render_stacked_boxes_with_section(
        boxes,
        RelationGraphLine::with_role("relations:".to_string(), AsciiColorRole::MutedText),
        &lines,
        options,
    )
}

fn relation_summary_lines(rows: &[RelationGraphSummaryRow]) -> Vec<RelationGraphLine> {
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

        let rendered =
            render_stacked_boxes_with_relation_summary(&[], &rows, &AsciiRenderOptions::ascii());

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
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

        assert!(rendered.contains("<span style=\"color:#222222\">relations:</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">A --&gt; B : one</span>"));
        assert!(rendered.contains("<span style=\"color:#333333\">          two</span>"));
    }
}
