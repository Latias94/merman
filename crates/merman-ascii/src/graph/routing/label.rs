use super::super::layout::CanvasCoord;
use super::plan::RouteLabelAnchor;
use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::terminal::char_display_width;
use crate::text::display_width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLabel {
    pub(super) start: CanvasCoord,
    pub(super) end: CanvasCoord,
    pub(super) text: String,
    pub(super) anchor: RouteLabelAnchor,
    pub(super) color: Option<AsciiRgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoutedLabelPlacement {
    x: usize,
    y: usize,
    width: usize,
}

impl RoutedLabelPlacement {
    pub(super) fn canvas_extent(self) -> (usize, usize) {
        (self.x + self.width, self.y + 1)
    }
}

pub(crate) fn draw_routed_label(canvas: &mut Canvas, label: &EdgeLabel) {
    let Some(placement) = routed_label_placement(label.start, label.end, &label.text, label.anchor)
    else {
        return;
    };

    write_label_overlay(canvas, placement.x, placement.y, &label.text, label.color);
}

pub(super) fn routed_label_placement(
    start: CanvasCoord,
    end: CanvasCoord,
    text: &str,
    anchor: RouteLabelAnchor,
) -> Option<RoutedLabelPlacement> {
    let width = display_width(text);
    if width == 0 {
        return None;
    }

    if start.y == end.y {
        let x = horizontal_label_x(start, end, width);
        let y = match anchor {
            RouteLabelAnchor::Above => start.y.saturating_sub(1),
            RouteLabelAnchor::Below => start.y + 1,
            RouteLabelAnchor::Inline | RouteLabelAnchor::Left | RouteLabelAnchor::Right => start.y,
        };
        return Some(RoutedLabelPlacement { x, y, width });
    }

    let x = match anchor {
        RouteLabelAnchor::Left => start.x.saturating_sub(width + 1),
        RouteLabelAnchor::Right => start.x + 1,
        RouteLabelAnchor::Inline | RouteLabelAnchor::Above | RouteLabelAnchor::Below => {
            start.x.saturating_sub(width / 2)
        }
    };
    let y = vertical_label_y(start, end);
    Some(RoutedLabelPlacement { x, y, width })
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
    fn routed_label_anchor_offsets_horizontal_route_rows() {
        let start = CanvasCoord { x: 4, y: 5 };
        let end = CanvasCoord { x: 12, y: 5 };

        assert_eq!(
            routed_label_placement(start, end, "flow", RouteLabelAnchor::Inline),
            Some(RoutedLabelPlacement {
                x: 6,
                y: 5,
                width: 4,
            })
        );
        assert_eq!(
            routed_label_placement(start, end, "flow", RouteLabelAnchor::Above),
            Some(RoutedLabelPlacement {
                x: 6,
                y: 4,
                width: 4,
            })
        );
        assert_eq!(
            routed_label_placement(start, end, "flow", RouteLabelAnchor::Below),
            Some(RoutedLabelPlacement {
                x: 6,
                y: 6,
                width: 4,
            })
        );
    }

    #[test]
    fn routed_label_anchor_offsets_vertical_route_columns() {
        let start = CanvasCoord { x: 10, y: 1 };
        let end = CanvasCoord { x: 10, y: 7 };

        assert_eq!(
            routed_label_placement(start, end, "back", RouteLabelAnchor::Inline),
            Some(RoutedLabelPlacement {
                x: 8,
                y: 4,
                width: 4,
            })
        );
        assert_eq!(
            routed_label_placement(start, end, "back", RouteLabelAnchor::Left),
            Some(RoutedLabelPlacement {
                x: 5,
                y: 4,
                width: 4,
            })
        );
        assert_eq!(
            routed_label_placement(start, end, "back", RouteLabelAnchor::Right),
            Some(RoutedLabelPlacement {
                x: 11,
                y: 4,
                width: 4,
            })
        );
    }
}
