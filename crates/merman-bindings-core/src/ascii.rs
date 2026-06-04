use crate::common::{
    BindingError, BindingStatus, binding_site_config, no_diagram_error, parse_options, source_text,
};

pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;

    let parse = if options
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        merman::ParseOptions::lenient()
    } else {
        merman::ParseOptions::strict()
    };

    let mut renderer = merman::ascii::HeadlessAsciiRenderer::new()
        .with_parse_options(parse)
        .with_ascii_options(merman::ascii::AsciiRenderOptions::unicode());
    if let Some(site_config) = binding_site_config(&options)? {
        renderer = renderer.with_site_config(site_config);
    }

    let rendered = renderer
        .render_ascii_sync(source)
        .map_err(classify_ascii_error)?
        .ok_or_else(no_diagram_error)?;

    Ok(rendered.into_bytes())
}

fn classify_ascii_error(err: merman::ascii::HeadlessAsciiError) -> BindingError {
    match err {
        merman::ascii::HeadlessAsciiError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::ascii::HeadlessAsciiError::Ascii(err) => match err {
            merman::ascii::AsciiError::InvalidOption { .. } => {
                BindingError::new(BindingStatus::InvalidArgument, err.to_string())
            }
            merman::ascii::AsciiError::UnsupportedDiagram { .. }
            | merman::ascii::AsciiError::UnsupportedFeature { .. } => {
                BindingError::new(BindingStatus::UnsupportedFormat, err.to_string())
            }
            merman::ascii::AsciiError::RenderLimitExceeded { .. } => {
                BindingError::new(BindingStatus::RenderError, err.to_string())
            }
            _ => BindingError::new(BindingStatus::RenderError, err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_ascii_returns_unicode_text() {
        let text =
            String::from_utf8(render_ascii(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap())
                .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}
