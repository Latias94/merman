pub(super) fn static_inline_pipeline(
    id_prefix: impl Into<String>,
) -> merman_render::svg::SvgPipeline {
    merman_render::svg::SvgPipeline::parity().with_static_inline_contract(id_prefix)
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
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml">Safe label</div></foreignObject></svg>"#;

        let control = merman::OperationControl::new();
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .with_resource_policy(resources().render_policy())
            .begin_session_with_control(control.clone())
            .unwrap();
        let output = static_inline_pipeline("rustdoc-test")
            .process_to_string(svg, &session)
            .unwrap();

        assert!(output.contains("Safe label"), "{output}");
        assert!(!output.contains("foreignObject"), "{output}");
        merman_render::svg::validate_static_inline_svg(&output, &session).unwrap();
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
            let error = static_inline_pipeline("rustdoc-test")
                .process_to_string(svg, &session)
                .unwrap_err();

            assert!(error.to_string().contains(expected), "{error}");
        }
    }
}
