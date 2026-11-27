use merman_render_core::svg_safety::inline_raster_url;

pub(crate) fn intrinsic_raster_size(href: &str) -> Option<kurbo::Size> {
    let url = inline_raster_url(href)?;
    let (bytes, _) = url.decode_to_vec().ok()?;
    let mut reader = std::io::Cursor::new(bytes);
    let format = imagesize::reader_type(&mut reader).ok()?;
    let mime = url.mime_type();
    let matching_format = match format {
        imagesize::ImageType::Png => mime.matches("image", "png"),
        imagesize::ImageType::Jpeg => mime.matches("image", "jpeg") || mime.matches("image", "jpg"),
        imagesize::ImageType::Gif => mime.matches("image", "gif"),
        imagesize::ImageType::Webp => mime.matches("image", "webp"),
        // Export dependencies can enable additional formats on the same package.
        #[allow(unreachable_patterns)]
        _ => false,
    };
    if !matching_format {
        return None;
    }
    let size = format.reader_size(&mut reader).ok()?;
    (size.width > 0 && size.height > 0)
        .then_some(kurbo::Size::new(size.width as f64, size.height as f64))
}
