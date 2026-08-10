use super::super::layout::CanvasCoord;
use crate::canvas::{Canvas, CanvasColor};
use crate::error::Result;
use crate::options::TerminalWidthProfile;
use crate::safe_text::SafeLine;
use crate::text::{display_width_with_profile, normalize_optional_text, split_label_lines};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLabel {
    pub(super) text: RoutedLabelText,
    pub(super) placement: RoutedLabelPlacement,
    pub(super) color: CanvasColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::graph) struct RoutedLabelText {
    lines: Vec<String>,
    width: usize,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph) struct RoutedLabelPlacement {
    x: usize,
    y: usize,
    width: usize,
}

impl RoutedLabelPlacement {
    pub(in crate::graph) fn new(x: usize, y: usize, width: usize) -> Self {
        Self { x, y, width }
    }

    #[cfg(test)]
    pub(in crate::graph) fn canvas_extent(self) -> (usize, usize) {
        self.canvas_extent_for_lines(1)
    }

    pub(in crate::graph) fn canvas_extent_for_lines(self, line_count: usize) -> (usize, usize) {
        (self.x + self.width, self.y + line_count.max(1))
    }

    pub(in crate::graph) fn x(self) -> usize {
        self.x
    }

    pub(in crate::graph) fn y(self) -> usize {
        self.y
    }

    pub(in crate::graph) fn width(self) -> usize {
        self.width
    }

    pub(in crate::graph) fn with_position(self, x: usize, y: usize) -> Self {
        Self { x, y, ..self }
    }
}

impl RoutedLabelText {
    #[cfg(test)]
    pub(super) fn new(raw: &str) -> Option<Self> {
        Self::new_with_profile(raw, TerminalWidthProfile::Unicode)
    }

    pub(super) fn new_with_profile(raw: &str, width_profile: TerminalWidthProfile) -> Option<Self> {
        let normalized = normalize_optional_text(Some(raw))?;
        let lines = split_label_lines(&normalized);
        let width = lines
            .iter()
            .map(|line| display_width_with_profile(line, width_profile))
            .max()
            .unwrap_or_default();
        if width == 0 {
            return None;
        }

        Some(Self {
            lines,
            width,
            width_profile,
        })
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(super) fn width(&self) -> usize {
        self.width
    }

    fn line_width(&self, line: &str) -> usize {
        display_width_with_profile(line, self.width_profile)
    }

    pub(super) fn line_count(&self) -> usize {
        self.lines.len()
    }
}

pub(crate) fn draw_routed_label(canvas: &mut Canvas, label: &EdgeLabel) -> Result<()> {
    for (line_index, line) in label.text.lines().iter().enumerate() {
        let line_width = label.text.line_width(line);
        let x = label
            .placement
            .x
            .saturating_add(label.text.width().saturating_sub(line_width) / 2);
        write_label_overlay(
            canvas,
            x,
            label.placement.y + line_index,
            line,
            label.text.width_profile,
            label.color,
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn routed_label_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
) -> Option<RoutedLabelPlacement> {
    let text = RoutedLabelText::new(text)?;
    routed_label_placement_for_text(start, end, &text)
}

pub(super) fn routed_label_placement_for_text(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &RoutedLabelText,
) -> Option<RoutedLabelPlacement> {
    if start.y == end.y {
        let x = horizontal_label_x(start, end, text.width());
        let y = label_block_y(start.y, text.line_count());
        return Some(RoutedLabelPlacement::new(x, y, text.width()));
    }

    let x = start.x.saturating_sub(text.width() / 2);
    let y = label_block_y(vertical_label_y(start, end), text.line_count());
    Some(RoutedLabelPlacement::new(x, y, text.width()))
}

#[cfg(test)]
pub(super) fn routed_label_right_of_vertical_route_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
) -> Option<RoutedLabelPlacement> {
    let text = RoutedLabelText::new(text)?;
    routed_label_right_of_vertical_route_placement_for_text(start, end, &text)
}

pub(super) fn routed_label_right_of_vertical_route_placement_for_text(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &RoutedLabelText,
) -> Option<RoutedLabelPlacement> {
    if start.x != end.x {
        return None;
    }

    Some(RoutedLabelPlacement::new(
        start.x + 1,
        label_block_y(vertical_label_y(start, end), text.line_count()),
        text.width(),
    ))
}

fn horizontal_label_x(start: CanvasCoord, end: CanvasCoord, width: usize) -> usize {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let middle_x = min_x + (max_x - min_x) / 2;
    middle_x.saturating_sub(width / 2)
}

fn vertical_label_y(start: CanvasCoord, end: CanvasCoord) -> usize {
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);
    min_y + (max_y - min_y) / 2
}

fn label_block_y(center_y: usize, line_count: usize) -> usize {
    center_y.saturating_sub(line_count.saturating_sub(1) / 2)
}

fn write_label_overlay(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &str,
    width_profile: TerminalWidthProfile,
    color: CanvasColor,
) -> Result<()> {
    let mut offset = 0;
    let label = SafeLine::new(label);
    for grapheme in label.graphemes(width_profile) {
        if grapheme.text() != " " {
            match color {
                CanvasColor::Role(role) => {
                    canvas.write_text_role(x + offset, y, grapheme.text(), role)?
                }
                CanvasColor::Direct(color) => {
                    canvas.write_text_color(x + offset, y, grapheme.text(), color)?
                }
            }
        }
        offset += grapheme.width();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::TerminalWidthProfile;
    use crate::terminal::TerminalCellText;

    #[test]
    fn routed_label_placement_centers_horizontal_route_labels() {
        let start = CanvasCoord { x: 4, y: 5 };
        let end = CanvasCoord { x: 12, y: 5 };

        assert_eq!(
            routed_label_placement(start, end, "flow"),
            Some(RoutedLabelPlacement::new(6, 5, 4))
        );
    }

    #[test]
    fn routed_label_placement_centers_vertical_route_labels() {
        let start = CanvasCoord { x: 10, y: 1 };
        let end = CanvasCoord { x: 10, y: 7 };

        assert_eq!(
            routed_label_placement(start, end, "back"),
            Some(RoutedLabelPlacement::new(8, 4, 4))
        );
    }

    #[test]
    fn routed_label_placement_accounts_for_multiline_labels() {
        let start = CanvasCoord { x: 4, y: 5 };
        let end = CanvasCoord { x: 12, y: 5 };

        assert_eq!(
            routed_label_placement(start, end, "north<br>south"),
            Some(RoutedLabelPlacement::new(6, 5, 5))
        );
        assert_eq!(
            routed_label_right_of_vertical_route_placement(start, end, "north<br>south"),
            None
        );
    }

    #[test]
    fn routed_label_right_of_vertical_route_requires_vertical_route() {
        let start = CanvasCoord { x: 10, y: 1 };
        let end = CanvasCoord { x: 10, y: 7 };

        assert_eq!(
            routed_label_right_of_vertical_route_placement(start, end, "back"),
            Some(RoutedLabelPlacement::new(11, 4, 4))
        );
        assert_eq!(
            routed_label_right_of_vertical_route_placement(
                CanvasCoord { x: 1, y: 1 },
                CanvasCoord { x: 4, y: 1 },
                "bad",
            ),
            None
        );
    }

    #[test]
    fn routed_label_overlay_preserves_complete_grapheme_cells() {
        let text = RoutedLabelText::new_with_profile(
            "e\u{301} \u{1f469}\u{200d}\u{1f4bb} \u{1f1fa}\u{1f1f8}",
            TerminalWidthProfile::Unicode,
        )
        .expect("label should exist");
        let mut canvas = Canvas::with_width_profile(7, 1, TerminalWidthProfile::Unicode);
        for x in 0..7 {
            canvas.set(x, 0, '-');
        }

        draw_routed_label(
            &mut canvas,
            &EdgeLabel {
                placement: RoutedLabelPlacement::new(0, 0, text.width()),
                text,
                color: CanvasColor::Role(crate::color::AsciiColorRole::EdgeLabel),
            },
        )
        .expect("test routed label should fit the unbounded resource policy");

        assert_eq!(
            canvas.get_text(0, 0),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
        assert_eq!(
            canvas.get_text(2, 0),
            Some(TerminalCellText::Grapheme("\u{1f469}\u{200d}\u{1f4bb}"))
        );
        assert_eq!(
            canvas.get_text(5, 0),
            Some(TerminalCellText::Grapheme("\u{1f1fa}\u{1f1f8}"))
        );
    }

    #[test]
    fn routed_label_text_uses_selected_ambiguous_width_profile() {
        let unicode = RoutedLabelText::new_with_profile("A·B", TerminalWidthProfile::Unicode)
            .expect("Unicode label should exist");
        let cjk = RoutedLabelText::new_with_profile("A·B", TerminalWidthProfile::Cjk)
            .expect("CJK label should exist");

        assert_eq!(unicode.width(), 3);
        assert_eq!(cjk.width(), 4);
    }
}
