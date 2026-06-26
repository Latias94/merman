use super::super::layout::CanvasCoord;
use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::terminal::char_display_width;
use crate::text::display_width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLabel {
    pub(super) text: String,
    pub(super) placement: RoutedLabelPlacement,
    pub(super) color: Option<AsciiRgb>,
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

    pub(in crate::graph) fn canvas_extent(self) -> (usize, usize) {
        (self.x + self.width, self.y + 1)
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

pub(crate) fn draw_routed_label(canvas: &mut Canvas, label: &EdgeLabel) {
    write_label_overlay(
        canvas,
        label.placement.x,
        label.placement.y,
        &label.text,
        label.color,
    );
}

pub(super) fn routed_label_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
) -> Option<RoutedLabelPlacement> {
    let width = display_width(text);
    if width == 0 {
        return None;
    }

    if start.y == end.y {
        let x = horizontal_label_x(start, end, width);
        return Some(RoutedLabelPlacement::new(x, start.y, width));
    }

    let x = start.x.saturating_sub(width / 2);
    let y = vertical_label_y(start, end);
    Some(RoutedLabelPlacement::new(x, y, width))
}

pub(super) fn routed_label_right_of_vertical_route_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
) -> Option<RoutedLabelPlacement> {
    if start.x != end.x {
        return None;
    }
    let width = display_width(text);
    if width == 0 {
        return None;
    }

    Some(RoutedLabelPlacement::new(
        start.x + 1,
        vertical_label_y(start, end),
        width,
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

fn write_label_overlay(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &str,
    color: Option<AsciiRgb>,
) {
    let mut offset = 0;
    for ch in label.chars() {
        if ch != ' ' {
            if let Some(color) = color {
                canvas.set_color(x + offset, y, ch, color);
            } else {
                canvas.set_role(x + offset, y, ch, AsciiColorRole::EdgeLabel);
            }
        }
        offset += char_display_width(ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
