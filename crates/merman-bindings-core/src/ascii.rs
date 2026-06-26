use crate::common::{
    BindingError, BindingStatus, binding_fixed_local_offset_minutes, binding_fixed_today,
    binding_site_config, no_diagram_error, parse_options, source_text,
};

pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let renderer = build_ascii_renderer(&options)?;

    render_ascii_with_renderer(&renderer, source)
}

#[derive(Clone)]
pub(crate) struct CachedAsciiEngine {
    renderer: merman::ascii::HeadlessAsciiRenderer,
}

impl CachedAsciiEngine {
    pub(crate) fn new(options: &crate::common::BindingOptions) -> Result<Self, BindingError> {
        Ok(Self {
            renderer: build_ascii_renderer(options)?,
        })
    }

    pub(crate) fn render_ascii(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        render_ascii_with_renderer(&self.renderer, source)
    }
}

fn build_ascii_renderer(
    options: &crate::common::BindingOptions,
) -> Result<merman::ascii::HeadlessAsciiRenderer, BindingError> {
    let parse = if options
        .analysis
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
        .with_fixed_today(binding_fixed_today(options)?)
        .with_fixed_local_offset_minutes(binding_fixed_local_offset_minutes(options)?)
        .with_parse_options(parse)
        .with_ascii_options(merman::ascii::AsciiRenderOptions::unicode());
    if let Some(site_config) = binding_site_config(options)? {
        renderer = renderer.with_site_config(site_config);
    }

    Ok(renderer)
}

fn render_ascii_with_renderer(
    renderer: &merman::ascii::HeadlessAsciiRenderer,
    source: &str,
) -> Result<Vec<u8>, BindingError> {
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

    #[test]
    fn shared_parse_options_are_stored_under_analysis_options() {
        let options =
            crate::common::parse_options(br#"{ "parse": { "suppress_errors": true } }"#).unwrap();

        assert_eq!(
            options
                .analysis
                .parse
                .as_ref()
                .and_then(|parse| parse.suppress_errors),
            Some(true)
        );
    }

    #[test]
    fn render_ascii_rejects_invalid_fixed_time_options() {
        let err = render_ascii(
            b"flowchart TD\nA[Hello]",
            br#"{ "fixed_today": "2026/02/15" }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("fixed_today"), "{err:?}");
    }
}
