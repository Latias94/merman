mod executor;
mod icons;
mod markdown_export;
mod plan;
mod raster;
mod svg_pipeline;

pub(crate) use executor::run_render;
pub(crate) use plan::{render_plan_for_mmdc, render_plan_for_subcommand};

#[cfg(test)]
mod tests {
    use super::executor::RenderRequest;
    use super::plan::{RenderMode, RenderPlan};
    use super::raster::RasterCliOptions;
    use super::svg_pipeline::svg_postprocess_pipeline;
    use crate::cli::{
        ParseCliArgs, RenderCliArgs, RenderFormat, SvgPipelineKind, TextOutputCliArgs,
    };
    use crate::io::OutputTarget;
    use merman::render::SvgPipeline;
    use merman::{Engine, ParseOptions};

    fn test_plan(format: RenderFormat) -> RenderPlan {
        RenderPlan {
            input: None,
            output: None::<OutputTarget>,
            format,
            parse: ParseCliArgs::default(),
            render: RenderCliArgs::default(),
            scale: 1.0,
            raster: RasterCliOptions::default(),
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
        let engine = Engine::new();
        let request = RenderRequest {
            plan: &plan,
            engine: &engine,
            parse_options: ParseOptions::default(),
            math_renderer: None,
        };
        let svg = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = request
            .postprocess_pipeline()
            .process_to_string(svg)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes bad"));
        assert!(!out.contains("animation: bad"));
        assert!(out.contains(r#"style="background-color: #f8fafc;""#));
        assert_eq!(
            out.matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }

    #[test]
    fn diagram_svg_pipeline_keeps_parity_base_before_cli_postprocessors() {
        let plan = test_plan(RenderFormat::Svg);
        let engine = Engine::new();
        let request = RenderRequest {
            plan: &plan,
            engine: &engine,
            parse_options: ParseOptions::default(),
            math_renderer: None,
        };
        let svg = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = request
            .postprocess_pipeline()
            .process_to_string(svg)
            .unwrap();

        assert!(out.contains("<foreignObject"));
        assert!(out.contains(r#"style="background-color: #f8fafc;""#));
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
        let engine = Engine::new();
        let request = RenderRequest {
            plan: &plan,
            engine: &engine,
            parse_options: ParseOptions::default(),
            math_renderer: None,
        };
        let svg = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = request
            .postprocess_pipeline()
            .process_to_string(svg)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(out.contains(r#"data-merman-foreignobject="fallback""#));
        assert!(out.contains(r#"style="background-color: #f8fafc;""#));
        assert_eq!(
            out.matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }

    #[test]
    fn raw_svg_raster_pipeline_sanitizes_before_cli_postprocessors() {
        let pipeline = svg_postprocess_pipeline(
            SvgPipeline::resvg_safe(),
            Some("#f8fafc"),
            Some(".node { fill: red; }"),
        );
        let svg = r#"<svg id="raw" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = pipeline.process_to_string(svg).unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes bad"));
        assert!(!out.contains("animation: bad"));
        assert!(out.contains(r#"style="background-color: #f8fafc;""#));
        assert!(out.contains(r#"data-merman-postprocess="scoped-css""#));
        assert!(out.contains("#raw .node { fill: red; }"));
    }
}
