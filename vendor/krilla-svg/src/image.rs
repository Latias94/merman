use krilla::geom::{Rect, Size};
use krilla::image::Image;
use krilla::paint::FillRule;
use krilla::surface::Surface;
use usvg::ImageKind;

use crate::util::RectExt;
use crate::{render_tree, ProcessContext};

/// Render an image into a surface.
///
/// Returns `None` if the image could not be rendered.
pub(crate) fn render(
    image: &usvg::Image,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) -> Option<()> {
    if !image.is_visible() {
        return Some(());
    }

    let size = Size::from_wh(image.size().width(), image.size().height()).unwrap();

    match image.kind() {
        ImageKind::JPEG(d) => {
            let image = Image::from_jpeg(d.clone().into(), false).ok()?;
            surface.draw_image(image, size);
        }
        ImageKind::PNG(d) => {
            let image = Image::from_png(d.clone().into(), false).ok()?;
            surface.draw_image(image, size);
        }
        ImageKind::GIF(d) => {
            let image = Image::from_gif(d.clone().into(), false).ok()?;
            surface.draw_image(image, size);
        }
        ImageKind::WEBP(d) => {
            let image = Image::from_webp(d.clone().into(), false).ok()?;
            surface.draw_image(image, size);
        }
        ImageKind::SVG(t) => {
            let clip_path =
                Rect::from_xywh(0.0, 0.0, t.size().width(), t.size().height())?.to_clip_path();
            surface.push_clip_path(&clip_path, &FillRule::NonZero);
            render_tree(t, process_context.svg_settings, surface);
            surface.pop();
        }
    }

    Some(())
}
