use data_url::DataUrl;

pub fn inline_raster_url(value: &str) -> Option<DataUrl<'_>> {
    let url = DataUrl::process(value).ok()?;
    let mime = url.mime_type();
    let approved_mime = mime.matches("image", "png")
        || mime.matches("image", "jpeg")
        || mime.matches("image", "jpg")
        || mime.matches("image", "gif")
        || mime.matches("image", "webp");
    (approved_mime && has_valid_percent_encoding(value)).then_some(url)
}

pub fn is_data_url(value: &str) -> bool {
    DataUrl::process(value).is_ok()
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
