use super::TreemapDiagramLayout;
use merman_render_core::Bounds;

pub fn treemap_view_box(
    layout: &TreemapDiagramLayout,
    measured_title: Option<(f64, f64)>,
) -> (f64, f64, f64, f64) {
    let mut bounds = Bounds {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };

    for section in layout.sections.iter().filter(|section| section.depth != 0) {
        include_rect(&mut bounds, section.x0, section.y0, section.x1, section.y1);
    }
    for leaf in &layout.leaves {
        include_rect(&mut bounds, leaf.x0, leaf.y0, leaf.x1, leaf.y1);
    }

    let title_height = layout.title_height;
    if title_height > 0.0 && bounds.min_y.is_finite() && bounds.max_y.is_finite() {
        bounds.min_y += title_height;
        bounds.max_y += title_height;
    }
    if let Some((width, height)) = measured_title {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            let center_x = layout.width / 2.0;
            let center_y = title_height / 2.0;
            include_rect(
                &mut bounds,
                center_x - (width / 2.0),
                center_y - (height / 2.0),
                center_x + (width / 2.0),
                center_y + (height / 2.0),
            );
        } else {
            bounds.min_y = bounds.min_y.min(0.0);
            bounds.max_y = bounds.max_y.max(title_height);
        }
    }

    let padding = layout.diagram_padding;
    if bounds.min_x.is_finite()
        && bounds.min_y.is_finite()
        && bounds.max_x.is_finite()
        && bounds.max_y.is_finite()
    {
        (
            bounds.min_x - padding,
            bounds.min_y - padding,
            (bounds.max_x - bounds.min_x) + padding * 2.0,
            (bounds.max_y - bounds.min_y) + padding * 2.0,
        )
    } else {
        (-padding, -padding, padding * 2.0, padding * 2.0)
    }
}

fn include_rect(bounds: &mut Bounds, x0: f64, y0: f64, x1: f64, y1: f64) {
    let width = x1 - x0;
    let height = y1 - y0;
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        return;
    }
    bounds.min_x = bounds.min_x.min(x0);
    bounds.min_y = bounds.min_y.min(y0);
    bounds.max_x = bounds.max_x.max(x1);
    bounds.max_y = bounds.max_y.max(y1);
}
