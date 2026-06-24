use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::options::AsciiRenderOptions;
use crate::text::{StyledLine, display_width, split_label_lines};
mod layered;

pub(crate) use self::layered::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphLine {
    text: String,
    line: StyledLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    id: String,
    lines: Vec<RelationGraphLine>,
    width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphLabel {
    lines: Vec<String>,
    width: usize,
}

impl RelationGraphLabel {
    pub(crate) fn new(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lines = split_label_lines(trimmed);
        let width = lines
            .iter()
            .map(|line| display_width(line))
            .max()
            .unwrap_or_default();

        Some(Self { lines, width })
    }

    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn half_width(&self) -> usize {
        self.width / 2
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RelationStackPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
}

impl<'a> RelationStackPlan<'a> {
    pub(crate) fn from_centered_rows(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        extra_half_widths: &[usize],
        build_rows: impl FnOnce(usize) -> Vec<RelationGraphLine>,
    ) -> Self {
        let center = vertical_center(top, bottom, extra_half_widths);
        let relation_lines = build_rows(center);
        Self {
            top,
            bottom,
            center,
            relation_lines,
        }
    }

    pub(crate) fn render_with_options(&self, options: &AsciiRenderOptions) -> String {
        render_vertical_stack_with_options(
            self.top,
            self.bottom,
            self.center,
            self.relation_lines.clone(),
            options,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RelationParallelPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    lane_left: usize,
    lane_gap: usize,
    lane_widths: Vec<usize>,
    lanes: Vec<Vec<RelationGraphLine>>,
}

impl<'a> RelationParallelPlan<'a> {
    pub(crate) fn new(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        lanes: Vec<Vec<RelationGraphLine>>,
        lane_gap: usize,
    ) -> Self {
        let lane_widths = lanes
            .iter()
            .map(|lane| {
                lane.iter()
                    .map(|line| display_width(line.text()))
                    .max()
                    .unwrap_or(1)
                    .max(1)
            })
            .collect::<Vec<_>>();
        let lanes_width = lane_widths.iter().sum::<usize>()
            + lane_gap.saturating_mul(lane_widths.len().saturating_sub(1));
        let lane_center = lanes_width / 2;
        let center = (top.width / 2).max(bottom.width / 2).max(lane_center);
        let lane_left = center.saturating_sub(lane_center);

        Self {
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            lane_widths,
            lanes,
        }
    }

    pub(crate) fn render_with_options(&self, options: &AsciiRenderOptions) -> String {
        if options.color_mode == AsciiColorMode::Plain {
            let mut relation_lines = Vec::new();
            let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
            for row_index in 0..row_count {
                let mut line = String::new();
                line.extend(std::iter::repeat_n(' ', self.lane_left));
                for (lane_index, lane) in self.lanes.iter().enumerate() {
                    if lane_index > 0 {
                        line.extend(std::iter::repeat_n(' ', self.lane_gap));
                    }
                    let text = lane.get(row_index).map(|line| line.text()).unwrap_or("");
                    line.push_str(&centered_cell(text, self.lane_widths[lane_index]));
                }
                while line.ends_with(' ') {
                    line.pop();
                }
                relation_lines.push(line);
            }

            return render_vertical_stack(self.top, self.bottom, self.center, relation_lines);
        }

        let mut relation_lines = Vec::new();
        let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
        for row_index in 0..row_count {
            let mut parts = Vec::new();
            parts.push(RelationGraphLine::plain(" ".repeat(self.lane_left)));
            for (lane_index, lane) in self.lanes.iter().enumerate() {
                if lane_index > 0 {
                    parts.push(RelationGraphLine::plain(" ".repeat(self.lane_gap)));
                }
                let cell = lane
                    .get(row_index)
                    .cloned()
                    .unwrap_or_else(|| RelationGraphLine::plain(String::new()));
                parts.push(centered_cell_line(&cell, self.lane_widths[lane_index]));
            }
            relation_lines.push(concat_relation_lines(parts));
        }

        render_vertical_stack_with_options(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            options,
        )
    }

    #[allow(dead_code)]
    fn render_plain(&self) -> String {
        let mut relation_lines = Vec::new();
        let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
        for row_index in 0..row_count {
            let mut line = String::new();
            line.extend(std::iter::repeat_n(' ', self.lane_left));
            for (lane_index, lane) in self.lanes.iter().enumerate() {
                if lane_index > 0 {
                    line.extend(std::iter::repeat_n(' ', self.lane_gap));
                }
                let text = lane.get(row_index).map(|line| line.text()).unwrap_or("");
                line.push_str(&centered_cell(text, self.lane_widths[lane_index]));
            }
            while line.ends_with(' ') {
                line.pop();
            }
            relation_lines.push(line);
        }

        render_vertical_stack(self.top, self.bottom, self.center, relation_lines)
    }
}

impl RelationGraphLine {
    pub(crate) fn new(text: String, roles: Vec<Option<AsciiColorRole>>) -> Self {
        let line = StyledLine::text_with_roles(&text, roles);
        Self { text, line }
    }

    pub(crate) fn plain(text: String) -> Self {
        let line = StyledLine::plain_text(&text);
        Self { text, line }
    }

    pub(crate) fn with_role(text: String, role: AsciiColorRole) -> Self {
        let line = StyledLine::role_text(&text, role);
        Self { text, line }
    }

    pub(crate) fn box_border(
        left: char,
        right: char,
        horizontal: char,
        content_width: usize,
        role: AsciiColorRole,
    ) -> Self {
        let mut line = StyledLine::new();
        line.push_role_char(left, role);
        line.push_role_repeat(horizontal, content_width, role);
        line.push_role_char(right, role);
        Self::from_styled(line)
    }

    pub(crate) fn box_content(
        text: &str,
        content_width: usize,
        padding: usize,
        vertical: char,
        border_role: AsciiColorRole,
        text_role: AsciiColorRole,
    ) -> Self {
        let text_width = display_width(text);
        let trailing = content_width.saturating_sub(padding + text_width);

        let mut line = StyledLine::new();
        line.push_role_char(vertical, border_role);
        line.push_spaces(padding);
        line.push_role_text(text, text_role);
        line.push_spaces(trailing);
        line.push_role_char(vertical, border_role);
        Self::from_styled(line)
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) {
        self.line.write_to_at(canvas, x, y);
    }

    fn from_styled(line: StyledLine) -> Self {
        let text = line.text();
        Self { text, line }
    }
}

impl RelationGraphBox {
    #[allow(dead_code)]
    pub(crate) fn new(id: String, lines: Vec<String>, width: usize) -> Self {
        Self {
            id,
            lines: lines.into_iter().map(RelationGraphLine::plain).collect(),
            width,
        }
    }

    pub(crate) fn new_with_lines(id: String, lines: Vec<RelationGraphLine>, width: usize) -> Self {
        Self { id, lines, width }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) {
        for (row_index, line) in self.lines.iter().enumerate() {
            line.draw_at(canvas, x, y + row_index);
        }
    }
}

pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

pub(crate) fn render_stacked_boxes_with_options(
    boxes: &[RelationGraphBox],
    options: &AsciiRenderOptions,
) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return render_stacked_boxes(boxes);
    }

    let mut lines = Vec::new();
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::plain(String::new()));
        }
        lines.extend(relation_box.lines.iter().cloned());
    }

    render_lines_with_options(&lines, options)
}

pub(crate) fn render_stacked_boxes_with_section(
    boxes: &[RelationGraphBox],
    section_title: &str,
    section_lines: &[String],
    options: &AsciiRenderOptions,
) -> String {
    if options.color_mode != AsciiColorMode::Plain {
        let title_line =
            RelationGraphLine::with_role(section_title.to_string(), AsciiColorRole::MutedText);
        let section_lines = section_lines
            .iter()
            .map(|line| RelationGraphLine::with_role(line.clone(), AsciiColorRole::EdgeLabel))
            .collect::<Vec<_>>();
        return render_stacked_boxes_with_section_lines(boxes, title_line, &section_lines, options);
    }

    let mut rendered = render_stacked_boxes_with_options(boxes, options);
    if section_lines.is_empty() {
        return rendered;
    }

    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered.push_str(section_title);
    rendered.push('\n');
    for line in section_lines {
        rendered.push_str(line);
        rendered.push('\n');
    }
    rendered
}

fn render_stacked_boxes_with_section_lines(
    boxes: &[RelationGraphBox],
    section_title: RelationGraphLine,
    section_lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::plain(String::new()));
        }
        lines.extend(relation_box.lines.iter().cloned());
    }

    if !section_lines.is_empty() {
        if !lines.is_empty() {
            lines.push(RelationGraphLine::plain(String::new()));
        }
        lines.push(section_title);
        lines.extend(section_lines.iter().cloned());
    }

    render_lines_with_options(&lines, options)
}

pub(crate) fn find_box<'a>(
    boxes: &'a [RelationGraphBox],
    id: &str,
) -> Option<&'a RelationGraphBox> {
    boxes.iter().find(|relation_box| relation_box.id == id)
}

pub(crate) fn vertical_center(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    extra_half_widths: &[usize],
) -> usize {
    extra_half_widths
        .iter()
        .copied()
        .fold((top.width / 2).max(bottom.width / 2), usize::max)
}

pub(crate) fn render_vertical_stack(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<String>,
) -> String {
    let mut lines = Vec::new();
    lines.extend(align_box(top, center));
    lines.extend(relation_lines);
    lines.extend(align_box(bottom, center));

    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

pub(crate) fn render_vertical_stack_with_options(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return render_vertical_stack(
            top,
            bottom,
            center,
            relation_lines
                .into_iter()
                .map(|line| line.text().to_string())
                .collect(),
        );
    }

    let mut lines = Vec::new();
    lines.extend(align_box_lines(top, center));
    lines.extend(relation_lines);
    lines.extend(align_box_lines(bottom, center));

    render_lines_with_options(&lines, options)
}

fn render_box(relation_box: &RelationGraphBox) -> String {
    let mut rendered = relation_box
        .lines
        .iter()
        .map(RelationGraphLine::text)
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    rendered
}

fn render_lines_with_options(lines: &[RelationGraphLine], options: &AsciiRenderOptions) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let width = lines.iter().map(line_char_width).max().unwrap_or(0);
    if width == 0 {
        return "\n".repeat(lines.len());
    }

    let mut canvas = Canvas::new(width, lines.len());
    for (y, line) in lines.iter().enumerate() {
        line.draw_at(&mut canvas, 0, y);
    }

    canvas.finish_trimmed_with_options(options)
}

fn line_char_width(line: &RelationGraphLine) -> usize {
    display_width(line.text())
}

fn centered_cell(text: &str, width: usize) -> String {
    let text_width = display_width(text);
    let left_padding = width.saturating_sub(text_width) / 2;
    let right_padding = width.saturating_sub(text_width + left_padding);
    let mut cell = String::new();
    cell.extend(std::iter::repeat_n(' ', left_padding));
    cell.push_str(text);
    cell.extend(std::iter::repeat_n(' ', right_padding));
    cell
}

fn centered_cell_line(line: &RelationGraphLine, width: usize) -> RelationGraphLine {
    let text_width = display_width(line.text());
    let left_padding = width.saturating_sub(text_width) / 2;
    let right_padding = width.saturating_sub(text_width + left_padding);
    padded_line(line, left_padding, right_padding)
}

fn align_box(relation_box: &RelationGraphBox, center: usize) -> Vec<String> {
    let left_padding = center.saturating_sub(relation_box.width / 2);
    let padding = " ".repeat(left_padding);
    relation_box
        .lines
        .iter()
        .map(|line| format!("{padding}{}", line.text()))
        .collect()
}

fn align_box_lines(relation_box: &RelationGraphBox, center: usize) -> Vec<RelationGraphLine> {
    let left_padding = center.saturating_sub(relation_box.width / 2);
    relation_box
        .lines
        .iter()
        .map(|line| padded_line(line, left_padding, 0))
        .collect()
}

fn padded_line(line: &RelationGraphLine, left: usize, right: usize) -> RelationGraphLine {
    let mut padded = StyledLine::blank(left);
    padded.push_line(&line.line);
    padded.push_spaces(right);
    RelationGraphLine::from_styled(padded)
}

fn concat_relation_lines(parts: Vec<RelationGraphLine>) -> RelationGraphLine {
    let mut line = StyledLine::new();
    for part in parts {
        line.push_line(&part.line);
    }
    RelationGraphLine::from_styled(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;
    use crate::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb};

    #[test]
    fn render_stacked_boxes_preserves_plain_text() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string(), "|".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string(), "|".to_string()], 1),
        ];

        assert_eq!(render_stacked_boxes(&boxes), "A\n|\n\nB\n|\n");
    }

    #[test]
    fn render_stacked_boxes_with_section_appends_summary() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let section_lines = vec!["A --> B".to_string(), "B --> A".to_string()];

        assert_eq!(
            render_stacked_boxes_with_section(
                &boxes,
                "relations:",
                &section_lines,
                &AsciiRenderOptions::ascii(),
            ),
            "A\n\nB\n\nrelations:\nA --> B\nB --> A\n"
        );
    }

    #[test]
    fn render_stacked_boxes_with_section_colors_title_and_summary_lines() {
        let boxes = vec![RelationGraphBox::new_with_lines(
            "a".to_string(),
            vec![RelationGraphLine::with_role(
                "A".to_string(),
                AsciiColorRole::Text,
            )],
            1,
        )];
        let section_lines = vec!["A --> B".to_string()];
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x111111))
            .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x222222))
            .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333));

        let rendered = render_stacked_boxes_with_section(
            &boxes,
            "relations:",
            &section_lines,
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

        assert_eq!(
            rendered,
            concat!(
                "<span style=\"color:#111111\">A</span>\n",
                "\n",
                "<span style=\"color:#222222\">relations:</span>\n",
                "<span style=\"color:#333333\">A --&gt; B</span>\n",
            )
        );
    }

    #[test]
    fn relation_graph_box_draws_role_lines_to_trimmed_canvas() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let line = RelationGraphLine::with_role("AB".to_string(), AsciiColorRole::Text);
        let relation_box = RelationGraphBox::new_with_lines("box".to_string(), vec![line], 2);
        let mut canvas = Canvas::new(4, 1);
        relation_box.draw_at(&mut canvas, 0, 0);

        let output = canvas.finish_trimmed_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m\n");
    }

    #[test]
    fn relation_graph_box_content_line_preserves_border_and_text_roles() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x111111))
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x222222));
        let line = RelationGraphLine::box_content(
            "A",
            3,
            1,
            '|',
            AsciiColorRole::NodeBorder,
            AsciiColorRole::Text,
        );
        let mut canvas = Canvas::new(5, 1);

        line.draw_at(&mut canvas, 0, 0);

        assert_eq!(line.text(), "| A |");
        assert_eq!(
            canvas.finish_trimmed_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Html)
                    .with_color_theme(theme),
            ),
            "<span style=\"color:#111111\">|</span> <span style=\"color:#222222\">A</span> <span style=\"color:#111111\">|</span>\n"
        );
    }

    #[test]
    fn relation_line_chars_merge_crossing_relation_lines_to_junction() {
        let chars = RelationLineChars::new(['-', '|', '.', ':'], '+');
        let mut canvas = Canvas::new(1, 1);
        canvas.set_role(0, 0, '-', AsciiColorRole::EdgeLine);

        put_relation_char(&mut canvas, 0, 0, '|', chars);

        assert_eq!(canvas.get(0, 0), Some('+'));
        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::Junction))
        );
    }

    #[test]
    fn parallel_relation_lane_offsets_group_by_endpoint_pair() {
        let offsets =
            parallel_relation_lane_offsets([("A", "B"), ("A", "B"), ("A", "C"), ("A", "B")]);

        assert_eq!(offsets, vec![-6, 0, 0, 6]);
    }

    #[test]
    fn parallel_relation_lane_offsets_group_reverse_endpoint_pairs() {
        let offsets = parallel_relation_lane_offsets([("A", "B"), ("B", "A"), ("A", "B")]);

        assert_eq!(offsets, vec![-6, 0, 6]);
    }

    #[test]
    fn relation_graph_label_splits_breaks_and_tracks_line_count() {
        let label = RelationGraphLabel::new("north<br>south").expect("label should be present");

        assert_eq!(label.lines(), ["north", "south"]);
        assert_eq!(label.half_width(), 2);
        assert_eq!(label.line_count(), 2);
    }

    #[test]
    fn write_centered_relation_label_draws_each_line() {
        let label = RelationGraphLabel::new("A<br>B").expect("label should be present");
        let mut canvas = Canvas::new(3, 3);

        write_centered_relation_label(&mut canvas, 1, 1, &label, AsciiColorRole::EdgeLabel);

        assert_eq!(canvas.get(1, 1), Some('A'));
        assert_eq!(canvas.get(1, 2), Some('B'));
        assert_eq!(
            canvas.get_color(1, 1),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel))
        );
    }

    #[test]
    fn layered_relation_gap_grows_with_label_line_count() {
        let boxes = vec![
            RelationGraphBox::new("top".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("bottom".to_string(), vec!["B".to_string()], 1),
        ];
        let no_label_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 0)];
        let one_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 1)];
        let two_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 2)];

        let no_label_plan = plan_layered_relation_boxes(&boxes, &no_label_edges, 1)
            .expect("unlabeled layered relation should plan");
        let one_line_plan = plan_layered_relation_boxes(&boxes, &one_line_edges, 1)
            .expect("single-line labeled relation should plan");
        let two_line_plan = plan_layered_relation_boxes(&boxes, &two_line_edges, 1)
            .expect("multiline labeled relation should plan");

        assert_eq!(no_label_plan.height(), 5);
        assert_eq!(one_line_plan.height(), 6);
        assert_eq!(two_line_plan.height(), 7);
    }

    #[test]
    fn layered_relation_plan_reserves_width_for_reverse_spanning_edges() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("b", "c", 0, 0),
            LayeredRelationEdge::new("c", "a", 0, 0),
        ];

        let plan =
            plan_layered_relation_boxes(&boxes, &edges, 1).expect("cyclic plan should render");

        assert_eq!(plan.width(), 7);
    }

    #[test]
    fn layered_relation_plan_reserves_width_for_reverse_parallel_lanes() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("b", "a", 0, 0),
        ];

        let plan = plan_layered_relation_boxes(&boxes, &edges, 1)
            .expect("bidirectional plan should render");

        assert_eq!(plan.width(), 7);
    }

    #[test]
    fn layered_relation_route_plan_draws_route_and_overlays() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox {
                id: "top",
                relation_box: &top_box,
                x: 0,
                y: 0,
            },
            PlacedRelationGraphBox {
                id: "bottom",
                relation_box: &bottom_box,
                x: 0,
                y: 4,
            },
        ];
        let geometry = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0),
        ));
        let route = LayeredRelationRoutePlan::new(
            geometry.clone(),
            '|',
            '-',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
            vec![
                RelationOverlay::text(
                    geometry.from_x(),
                    geometry.source_marker_y(),
                    "T".to_string(),
                    AsciiColorRole::EdgeArrow,
                ),
                RelationOverlay::text(
                    (geometry.from_x() + geometry.to_x()) / 2,
                    geometry.route_y().saturating_sub(1),
                    "L".to_string(),
                    AsciiColorRole::EdgeLabel,
                ),
                RelationOverlay::text(
                    geometry.to_x(),
                    geometry.target_marker_y(),
                    "B".to_string(),
                    AsciiColorRole::EdgeArrow,
                ),
            ],
        );
        let mut canvas = Canvas::new(3, 5);

        route.draw_at(&mut canvas);

        assert_eq!(canvas.get(1, 1), Some('T'));
        assert_eq!(canvas.get(1, 2), Some('L'));
        assert_eq!(canvas.get(1, 3), Some('B'));
        assert_eq!(
            canvas.get_color(1, 1),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeArrow))
        );
        assert_eq!(
            canvas.get_color(1, 2),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel))
        );
    }

    #[test]
    fn layered_relation_route_label_y_follows_source_to_target_direction() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox {
                id: "top",
                relation_box: &top_box,
                x: 0,
                y: 0,
            },
            PlacedRelationGraphBox {
                id: "bottom",
                relation_box: &bottom_box,
                x: 0,
                y: 10,
            },
        ];

        let downward = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0),
        ));
        let upward = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[1],
            &placed[0],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0),
        ));

        assert_eq!(downward.label_y_after_source(), 2);
        assert_eq!(upward.label_y_after_source(), 8);
    }

    #[test]
    fn layered_relation_route_plan_avoids_intermediate_boxes() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let middle_box =
            RelationGraphBox::new("middle".to_string(), vec!["MMMMMMM".to_string()], 7);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox {
                id: "top",
                relation_box: &top_box,
                x: 0,
                y: 0,
            },
            PlacedRelationGraphBox {
                id: "middle",
                relation_box: &middle_box,
                x: 0,
                y: 4,
            },
            PlacedRelationGraphBox {
                id: "bottom",
                relation_box: &bottom_box,
                x: 0,
                y: 10,
            },
        ];

        let geometry = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[2],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0),
        ));

        assert_eq!(geometry.from_x(), 7);
        assert_eq!(geometry.to_x(), 7);
        assert_eq!(geometry.route_y(), 9);
    }
}
