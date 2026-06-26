use super::super::layout::CanvasCoord;
use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::terminal::char_display_width;
use crate::text::display_width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLabel {
    pub(super) start: CanvasCoord,
    pub(super) end: CanvasCoord,
    pub(super) text: String,
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
    let Some(placement) = routed_label_placement(label.start, label.end, &label.text) else {
        return;
    };

    write_label_overlay(canvas, placement.x, placement.y, &label.text, label.color);
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
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let middle_x = min_x + (max_x - min_x) / 2;
        let x = middle_x.saturating_sub(width / 2);
        return Some(RoutedLabelPlacement {
            x,
            y: start.y,
            width,
        });
    }

    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);
    let middle_y = min_y + (max_y - min_y) / 2;
    let x = start.x.saturating_sub(width / 2);
    Some(RoutedLabelPlacement {
        x,
        y: middle_y,
        width,
    })
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
