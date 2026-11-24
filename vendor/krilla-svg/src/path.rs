use krilla::surface::Surface;
use usvg::{Fill, PaintOrder, Stroke};

use crate::util::{convert_fill, convert_stroke, PathExt};
use crate::ProcessContext;

/// Render a path into a surface.
pub(crate) fn render(
    path: &usvg::Path,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    if !path.is_visible() {
        return;
    }

    match path.paint_order() {
        PaintOrder::FillAndStroke => {
            draw_path(path, path.fill(), path.stroke(), surface, process_context);
        }
        PaintOrder::StrokeAndFill => {
            draw_path(path, None, path.stroke(), surface, process_context);
            draw_path(path, path.fill(), None, surface, process_context);
        }
    }
}

/// Render a filled and/or stroked path into a surface.
pub(crate) fn draw_path(
    path: &usvg::Path,
    fill: Option<&Fill>,
    stroke: Option<&Stroke>,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    let fill = fill.map(|f| {
        convert_fill(
            f,
            surface.stream_builder(),
            process_context,
            usvg::Transform::identity(),
        )
    });

    let stroke = stroke.map(|s| {
        convert_stroke(
            s,
            surface.stream_builder(),
            process_context,
            usvg::Transform::identity(),
        )
    });

    // Otherwise krilla will fill with black by default.
    if fill.is_some() || stroke.is_some() {
        surface.set_fill(fill);
        surface.set_stroke(stroke);

        surface.draw_path(&path.to_krilla());
    }
}
