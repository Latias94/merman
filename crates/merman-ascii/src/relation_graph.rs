use crate::canvas::Canvas;
use crate::text::display_width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    id: String,
    lines: Vec<String>,
    width: usize,
}

impl RelationGraphBox {
    pub(crate) fn new(id: String, lines: Vec<String>, width: usize) -> Self {
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
            canvas.write_text(x, y + row_index, line);
        }
    }
}

pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
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

pub(crate) fn marker_line(marker: char, center: usize) -> String {
    let mut line = String::new();
    line.extend(std::iter::repeat_n(' ', center));
    line.push(marker);
    line
}

pub(crate) fn centered_text_line(text: &str, center: usize) -> String {
    let mut line = String::new();
    let half_width = display_width(text) / 2;
    line.extend(std::iter::repeat_n(' ', center.saturating_sub(half_width)));
    line.push_str(text);
    line
}

fn render_box(relation_box: &RelationGraphBox) -> String {
    let mut rendered = relation_box.lines.join("\n");
    rendered.push('\n');
    rendered
}

fn align_box(relation_box: &RelationGraphBox, center: usize) -> Vec<String> {
    let left_padding = center.saturating_sub(relation_box.width / 2);
    let padding = " ".repeat(left_padding);
    relation_box
        .lines
        .iter()
        .map(|line| format!("{padding}{line}"))
        .collect()
}
