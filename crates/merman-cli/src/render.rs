mod executor;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod export;
#[cfg(feature = "network-icons")]
mod icons;
#[cfg(feature = "analysis")]
mod markdown_export;
mod plan;
mod svg_pipeline;

pub(crate) use executor::run_render;
pub(crate) use plan::{render_plan_for_mmdc, render_plan_for_subcommand};

#[cfg(all(
    test,
    feature = "svg",
    feature = "network-icons",
    feature = "parallel-markdown",
    feature = "shell-completions"
))]
mod tests {
    use super::executor::RenderRequest;
    use super::export::{EmbeddedImageCliOptions, PdfCliOptions, RasterCliOptions};
    use super::plan::{RenderMode, RenderPlan};
    use super::svg_pipeline::svg_output_policy;
    use crate::cli::{
        ParseCliArgs, RenderCliArgs, RenderFormat, SvgPipelineKind, TextOutputCliArgs,
    };
    use crate::io::OutputTarget;
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
            output: None::<OutputTarget>,
            format,
            parse: ParseCliArgs::default(),
            render: RenderCliArgs::default(),
            scale: 1.0,
            raster: RasterCliOptions::default(),
            pdf: PdfCliOptions::default(),
            embedded_images: EmbeddedImageCliOptions::default(),
            background: Some("#f8fafc".to_string()),
            css: Some(".node { fill: red; }".to_string()),
            svg_pipeline: None,
            icon_registry: None,
            artefacts: None,
            jobs: 1,
            pdf_fit: true,
            quiet: true,
            text: TextOutputCliArgs::default(),
            mode: RenderMode::Subcommand,
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
