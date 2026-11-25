use krilla::geom::Size;
use krilla::image::Image;
use krilla::surface::Surface;
use usvg::tiny_skia_path::Transform;

use crate::util::KrillaTransformExt;
use crate::ProcessContext;

/// Render a filter into a surface by rasterizing it with `resvg` and drawing
/// the image.
///
/// Returns `None` if converting the filter was unsuccessful.
pub(crate) fn render(
    group: &usvg::Group,
    surface: &mut Surface,
    process_context: &ProcessContext,
) -> Option<()> {
    let group_bbox = group.layer_bounding_box()?;
    let abs_layer_bbox = group.abs_layer_bounding_box()?;
    let group_transform = group.transform();
    let layer_bbox = group_bbox.transform(group_transform)?;

    let raster_scale = {
        // Find out what dimensions the SVG will actually have in user space units inside of the
        // PDF.
        // Note that this is not a 100% accurate, because the `cur_transform` method of surface will
        // only return the transform in the current content stream, so it's not accurate in case we
        // are currently in a XObject. But it's as good as it gets.
        let actual_transform = surface
            .cur_transform()
            .to_usvg()
            .pre_concat(group_transform);
        let actual_bbox = group_bbox.transform(actual_transform)?;
        // Calculate the necessary scale in the x/y direction, and take the maximum of that.
        let scale = {
            let (x_scale, y_scale) = (
                (actual_bbox.width() / layer_bbox.width()),
                (actual_bbox.height() / layer_bbox.height()),
            );
            x_scale.max(y_scale) * process_context.svg_settings.filter_scale
        };

        let max_scale = {
            // Let's try to avoid generating images that have more than 5000 pixels in either direction.
            const PIXEL_THRESHOLD: f32 = 5000.0;
            let (x_scale, y_scale) = (
                (PIXEL_THRESHOLD / layer_bbox.width()),
                (PIXEL_THRESHOLD / layer_bbox.height()),
            );
            // Take the minimum of that.
            x_scale.min(y_scale)
        };

        // Take whichever is smaller.
        scale.min(max_scale)
    };

    let pixmap_size = Size::from_wh(
        layer_bbox.width() * raster_scale,
        layer_bbox.height() * raster_scale,
    )?;
    let width = pixmap_size.width().round() as u32;
    let height = pixmap_size.height().round() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;

    let initial_transform = Transform::from_scale(raster_scale, raster_scale)
        .pre_concat(Transform::from_translate(-layer_bbox.x(), -layer_bbox.y()))
        // Cancel render_node's offset because this pixmap uses parent coordinates.
        .pre_concat(Transform::from_translate(
            abs_layer_bbox.x(),
            abs_layer_bbox.y(),
        ));

    resvg::render_node(
        &usvg::Node::Group(Box::new(group.clone())),
        initial_transform,
        &mut pixmap.as_mut(),
    )?;

    let demultiplied = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect::<Vec<_>>();

    let image = Image::from_rgba8(demultiplied, width, height);
    let size = Size::from_wh(layer_bbox.width(), layer_bbox.height())?;

    surface.push_transform(&krilla::geom::Transform::from_translate(
        layer_bbox.x(),
        layer_bbox.y(),
    ));
    surface.draw_image(image, size);
    surface.pop();

    Some(())
}
