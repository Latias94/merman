use crate::error::CliError;
use crate::markdown::MarkdownFenceLocation;
use std::path::Path;

pub(super) fn prepare_static_svg(
    svg: &str,
    source_path: &Path,
    location: MarkdownFenceLocation,
    session: &merman_render::environment::RenderSession,
    control: &merman::OperationControl,
) -> Result<String, CliError> {
    crate::operation::checkpoint(control, merman::OperationPhase::Emit)?;
    merman_render::svg::validate_static_inline_svg_admission(svg, session)
        .map_err(|error| svg_content_error(source_path, location, error))?;
    let output = merman_render::svg::SvgPipeline::parity()
        .with_postprocessor(merman_render::svg::ForeignObjectFallbackPostprocessor)
        .with_postprocessor(merman_render::svg::SanitizeCssPostprocessor)
        .with_postprocessor(merman_render::svg::SanitizeSvgAttributesPostprocessor)
        .process_to_string(svg, session)
        .map_err(|error| svg_content_error(source_path, location, error))?;
    crate::operation::checkpoint(control, merman::OperationPhase::Emit)?;
    Ok(output)
}

pub(super) fn validate_static_svg(
    svg: &str,
    source_path: &Path,
    location: MarkdownFenceLocation,
    session: &merman_render::environment::RenderSession,
) -> Result<(), CliError> {
    merman_render::svg::validate_static_inline_svg(svg, session)
        .map_err(|error| svg_content_error(source_path, location, error))
}

pub(super) fn svg_content_error(
    source_path: &Path,
    location: MarkdownFenceLocation,
    error: merman_render::Error,
) -> CliError {
    match error {
        merman_render::Error::Cancelled(cancelled) => {
            CliError::Render(merman::RenderError::Cancelled(cancelled))
        }
        merman_render::Error::ResourceLimitExceeded(resource) => {
            CliError::Render(merman::RenderError::ResourceLimitExceeded(
                merman::render::ResourceLimitExceeded::from(resource),
            ))
        }
        error => content_error(source_path, location, error),
    }
}

fn content_error(
    source_path: &Path,
    location: MarkdownFenceLocation,
    error: impl std::fmt::Display,
) -> CliError {
    CliError::rustdoc_content(
        source_path,
        location.line,
        location.column,
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ResolvedResourcePolicy;

    fn resources() -> ResolvedResourcePolicy {
        ResolvedResourcePolicy::for_profile(merman::resources::CLI_DEFAULT_RESOURCE_PROFILE)
    }

    #[test]
    fn rustdoc_pipeline_converts_safe_foreign_objects_before_strict_validation() {
        let source = Path::new("docs.md");
        let location = MarkdownFenceLocation { line: 4, column: 2 };
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml">Safe label</div></foreignObject></svg>"#;

        let control = merman::OperationControl::new();
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .with_resource_policy(resources().render_policy())
            .begin_session_with_control(control.clone())
            .unwrap();
        let output = prepare_static_svg(svg, source, location, &session, &control).unwrap();

        assert!(output.contains("Safe label"), "{output}");
        assert!(!output.contains("foreignObject"), "{output}");
        validate_static_svg(&output, source, location, &session).unwrap();
    }

    #[test]
    fn rustdoc_pipeline_rejects_unsafe_renderer_output_before_lossy_sanitizers() {
        let cases = [
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>"#,
                "forbidden <script>",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"><g onclick="run()"/></svg>"#,
                "event attribute",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.test/a.png"/></svg>"#,
                "non-local resource",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"><style>@import url(https://example.test/a.css);</style></svg>"#,
                "forbidden CSS @import",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"><path background="url(https://tracker.test/a.png)"/></svg>"#,
                "non-local CSS URL",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"><path background-image="url(https://tracker.test/a.png)"/></svg>"#,
                "non-local CSS URL",
            ),
        ];

        for (svg, expected) in cases {
            let control = merman::OperationControl::new();
            let session = merman_render::environment::RenderEnvironment::deterministic()
                .with_resource_policy(resources().render_policy())
                .begin_session_with_control(control.clone())
                .unwrap();
            let error = prepare_static_svg(
                svg,
                Path::new("docs.md"),
                MarkdownFenceLocation { line: 7, column: 3 },
                &session,
                &control,
            )
            .unwrap_err();

            assert!(error.to_string().contains(expected), "{error}");
            assert!(error.to_string().contains("line 7, column 3"), "{error}");
        }
    }
}
