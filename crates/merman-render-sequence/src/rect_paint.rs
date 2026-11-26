use merman_render_core::Bounds;

pub fn include_rect_stroke_bounds(
    total: &mut Option<Bounds>,
    [left, top, width, height]: [f64; 4],
    stroke_width: f64,
) {
    let bounds = Bounds {
        min_x: left - stroke_width / 2.0,
        min_y: top - stroke_width / 2.0,
        max_x: left + width + stroke_width / 2.0,
        max_y: top + height + stroke_width / 2.0,
    };
    if let Some(total) = total {
        total.min_x = total.min_x.min(bounds.min_x);
        total.min_y = total.min_y.min(bounds.min_y);
        total.max_x = total.max_x.max(bounds.max_x);
        total.max_y = total.max_y.max(bounds.max_y);
    } else {
        *total = Some(bounds);
    }
}
