mod executor;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod export;
#[cfg(feature = "icons")]
mod icons;
#[cfg(feature = "markdown")]
mod markdown_export;
mod plan;
mod svg_pipeline;

pub(crate) use executor::run_render;
#[cfg(feature = "icons")]
pub(crate) use icons::{resolve_local_icon_paths, validate_icon_source_count};
#[cfg(feature = "markdown")]
pub(crate) use plan::render_plan_for_batch;
pub(crate) use plan::{render_plan_for_mmdc, render_plan_for_native};

#[cfg(all(test, feature = "svg", feature = "png"))]
mod tests {
    use super::executor::RenderRequest;
    use super::plan::{RenderMode, RenderPlan};
    use super::svg_pipeline::svg_output_policy;
    #[cfg(feature = "ascii")]
    use crate::cli::TextOutputCliArgs;
    use crate::cli::{ParseCliArgs, RenderCliArgs, RenderFormat, RenderInputKind, SvgPipelineKind};
    #[cfg(feature = "pdf")]
    use crate::invocation::ResolvedPdfOptions;
    use crate::invocation::{ResolvedEmbeddedImageOptions, ResolvedRasterOptions};
    use crate::io::OutputTarget;
    use crate::resources::ResolvedResourcePolicy;
    use merman::svg::{HeadlessRenderer, RenderEnvironment};

    fn assert_root_background(svg: &str, expected: &str) {
        let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
        let style = document
            .root_element()
            .attribute("style")
            .expect("root style attribute");
        assert!(
            style.split(';').map(str::trim).any(|declaration| {
                declaration
                    .split_once(':')
                    .is_some_and(|(property, value)| {
                        property.trim() == "background-color" && value.trim() == expected
                    })
            }),
            "expected root background {expected:?}, got {style:?}"
        );
    }

    fn test_plan(format: RenderFormat) -> RenderPlan {
        RenderPlan {
            input: None,
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            input_kind: RenderInputKind::Mermaid,
            output: None::<OutputTarget>,
            format,
            parse: ParseCliArgs::default(),
            render: RenderCliArgs::default(),
            resources: ResolvedResourcePolicy::for_profile(
                merman::resources::CLI_DEFAULT_RESOURCE_PROFILE,
            ),
            publications: crate::output::PublicationGuards::for_test(),
            scale: 1.0,
            raster: ResolvedRasterOptions::default(),
            #[cfg(feature = "pdf")]
            pdf: ResolvedPdfOptions::default(),
            embedded_images: ResolvedEmbeddedImageOptions::default(),
            background: Some("#f8fafc".to_string()),
            css: Some(".node { fill: red; }".to_string()),
            svg_pipeline: None,
            #[cfg(feature = "icons")]
            icon_registry: None,
            #[cfg(feature = "markdown")]
            artefacts: None,
            #[cfg(feature = "parallel-markdown")]
            jobs: 1,
            #[cfg(feature = "pdf")]
            pdf_fit: true,
            quiet: true,
            warn_on_implicit_stdin: false,
            #[cfg(feature = "ascii")]
            text: TextOutputCliArgs::default(),
            mode: RenderMode::NativeSingle,
        }
    }

    #[test]
    fn diagram_raster_pipeline_uses_resvg_safe_before_cli_postprocessors() {
        let mut plan = test_plan(RenderFormat::Png);
        plan.svg_pipeline = Some(SvgPipelineKind::Readable);
        let request = RenderRequest::new(
            &plan,
            HeadlessRenderer::new().with_environment(RenderEnvironment::deterministic()),
            None,
        );
        let session = request.renderer.environment().begin_session().unwrap();
        let svg = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = request
            .postprocess_pipeline()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes bad"));
        assert!(!out.contains("animation: bad"));
        assert_root_background(&out, "#f8fafc");
        assert_eq!(
            out.matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }

    #[test]
    fn diagram_svg_pipeline_keeps_parity_base_before_cli_postprocessors() {
        let plan = test_plan(RenderFormat::Svg);
        let request = RenderRequest::new(
            &plan,
            HeadlessRenderer::new().with_environment(RenderEnvironment::deterministic()),
            None,
        );
        let session = request.renderer.environment().begin_session().unwrap();
        let svg = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = request
            .postprocess_pipeline()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("<foreignObject"));
        assert_root_background(&out, "#f8fafc");
        assert_eq!(
            out.matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }

    #[test]
    fn diagram_svg_pipeline_can_request_resvg_safe_before_cli_postprocessors() {
        let mut plan = test_plan(RenderFormat::Svg);
        plan.svg_pipeline = Some(SvgPipelineKind::ResvgSafe);
        let request = RenderRequest::new(
            &plan,
            HeadlessRenderer::new().with_environment(RenderEnvironment::deterministic()),
            None,
        );
        let session = request.renderer.environment().begin_session().unwrap();
        let svg = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = request
            .postprocess_pipeline()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(out.contains(r#"data-merman-foreignobject="fallback""#));
        assert_root_background(&out, "#f8fafc");
        assert_eq!(
            out.matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }

    #[test]
    fn raw_svg_raster_pipeline_sanitizes_before_cli_postprocessors() {
        let pipeline = svg_output_policy(
            SvgPipelineKind::ResvgSafe,
            Some("#f8fafc"),
            Some(".node { fill: red; }"),
        )
        .pipeline();
        let svg = r#"<svg id="raw" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let out = pipeline.process_to_string(svg, &session).unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes bad"));
        assert!(!out.contains("animation: bad"));
        assert_root_background(&out, "#f8fafc");
        assert!(out.contains(r#"data-merman-postprocess="scoped-css""#));
        let document = roxmltree::Document::parse(&out).expect("valid SVG XML");
        let scoped_css = document
            .descendants()
            .find(|node| {
                node.has_tag_name("style")
                    && node.attribute("data-merman-postprocess") == Some("scoped-css")
            })
            .and_then(|node| node.text())
            .expect("scoped CSS style element");
        assert!(scoped_css.contains("#raw .node"), "{scoped_css}");
        assert!(scoped_css.contains("fill: red"), "{scoped_css}");
    }
}
