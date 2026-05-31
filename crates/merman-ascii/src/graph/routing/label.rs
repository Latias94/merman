use super::super::layout::CanvasCoord;
use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::text::display_width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLabel {
    pub(super) start: CanvasCoord,
    pub(super) end: CanvasCoord,
    pub(super) text: String,
    pub(super) color: Option<AsciiRgb>,
}

pub(crate) fn draw_routed_label(canvas: &mut Canvas, label: &EdgeLabel) {
    if label.start.y == label.end.y {
        draw_label_on_horizontal_line(
            canvas,
            label.start.x,
            label.end.x,
            label.start.y,
            Some(&label.text),
            label.color,
        );
    } else {
        draw_label_on_vertical_line(
            canvas,
            label.start.x,
            label.start.y,
            label.end.y,
            Some(&label.text),
            label.color,
        );
    }
}

pub(super) fn push_label_on_vertical_line(
    labels: &mut Vec<EdgeLabel>,
    x: usize,
    start_y: usize,
    end_y: usize,
    label: Option<&str>,
) {
    push_label(
        labels,
        CanvasCoord { x, y: start_y },
        CanvasCoord { x, y: end_y },
        label,
    );
}

fn push_label(
    labels: &mut Vec<EdgeLabel>,
    start: CanvasCoord,
    end: CanvasCoord,
    label: Option<&str>,
) {
    let Some(label) = label else {
        return;
    };
    if label.is_empty() {
        return;
    }
    labels.push(EdgeLabel {
        start,
        end,
        text: label.to_string(),
        color: None,
    });
}

fn draw_label_on_horizontal_line(
    canvas: &mut Canvas,
    start_x: usize,
    end_x: usize,
    y: usize,
    label: Option<&str>,
    color: Option<AsciiRgb>,
) {
    let Some(label) = label else {
        return;
    };
    if label.is_empty() {
        return;
    }
    let min_x = start_x.min(end_x);
    let max_x = start_x.max(end_x);
    let middle_x = min_x + (max_x - min_x) / 2;
    let x = middle_x.saturating_sub(display_width(label) / 2);
    write_label_overlay(canvas, x, y, label, color);
}

fn draw_label_on_vertical_line(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    label: Option<&str>,
    color: Option<AsciiRgb>,
) {
    let Some(label) = label else {
        return;
    };
    if label.is_empty() {
        return;
    }
    let min_y = start_y.min(end_y);
    let max_y = start_y.max(end_y);
    let middle_y = min_y + (max_y - min_y) / 2;
    let x = x.saturating_sub(display_width(label) / 2);
    write_label_overlay(canvas, x, middle_y, label, color);
}

fn write_label_overlay(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &str,
    color: Option<AsciiRgb>,
) {
    for (offset, ch) in label.chars().enumerate() {
        if ch != ' ' {
            if let Some(color) = color {
                canvas.set_color(x + offset, y, ch, color);
            } else {
                canvas.set_role(x + offset, y, ch, AsciiColorRole::EdgeLabel);
            }
        }
    }
}
