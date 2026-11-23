use data_url::DataUrl;

pub(crate) fn inline_raster_url(value: &str) -> Option<DataUrl<'_>> {
    let url = DataUrl::process(value).ok()?;
    let mime = url.mime_type();
    let approved_mime = mime.matches("image", "png")
        || mime.matches("image", "jpeg")
        || mime.matches("image", "jpg")
        || mime.matches("image", "gif")
        || mime.matches("image", "webp");
    (approved_mime && has_valid_percent_encoding(value)).then_some(url)
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
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

fn has_valid_percent_encoding(data_url: &str) -> bool {
    let Some((_, encoded_body)) = data_url.split_once(',') else {
        return false;
    };
    let encoded_body = encoded_body
        .split_once('#')
        .map_or(encoded_body, |(body, _)| body);
    let mut bytes = encoded_body.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%'
            && (!bytes.next().is_some_and(|byte| byte.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|byte| byte.is_ascii_hexdigit()))
        {
            return false;
        }
    }
    true
}
